use serde::{Deserialize, Serialize};
use tracing::{debug, error, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub max_tokens: usize,
    pub temperature: f32,
}

#[derive(Debug, Deserialize)]
pub struct ChatResponse {
    pub choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
pub struct Choice {
    pub message: ChatMessage,
}

pub struct AIClient {
    client: reqwest::Client,
    api_url: String,
    model: String,
    api_key: String,
    max_tokens: usize,
    temperature: f32,
}

impl AIClient {
    pub fn new(api_url: &str, model: &str, api_key: &str, max_tokens: usize, temperature: f32) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
            api_url: api_url.to_string(),
            model: model.to_string(),
            api_key: api_key.to_string(),
            max_tokens,
            temperature,
        }
    }

    pub async fn chat(&self, messages: &[ChatMessage]) -> Result<String, Box<dyn std::error::Error>> {
        let request = ChatRequest {
            model: self.model.clone(),
            messages: messages.to_vec(),
            max_tokens: self.max_tokens,
            temperature: self.temperature,
        };

        debug!("Sending AI request to {} with {} messages", self.api_url, messages.len());

        let response = self
            .client
            .post(&self.api_url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await;

        match response {
            Ok(resp) => {
                if resp.status().is_success() {
                    let chat_response: ChatResponse = resp.json().await?;
                    if let Some(choice) = chat_response.choices.first() {
                        Ok(choice.message.content.clone())
                    } else {
                        warn!("AI response has no choices");
                        Err("No response from AI".into())
                    }
                } else {
                    let status = resp.status().as_u16();
                    let body = resp.text().await.unwrap_or_default();
                    error!("AI API error: {} - {}", status, body);
                    Err(format!("API error {}: {}", status, body).into())
                }
            }
            Err(e) => {
                error!("AI request failed: {}", e);
                Err(format!("Request failed: {}", e).into())
            }
        }
    }
}
