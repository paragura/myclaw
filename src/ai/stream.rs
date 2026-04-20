use serde::{Deserialize, Serialize};
use tracing::{error, warn};
use futures::TryStreamExt;

use super::client::ChatMessage;

/// A streaming chunk from the AI response.
/// Inspired by Codex's ResponseItem enum — each chunk is a different type of item.
#[derive(Debug, Clone)]
pub enum StreamItem {
    /// Intermediate thinking/reasoning steps (shown as Discord messages)
    Reasoning {
        content: String,
        /// Whether this is the final thinking summary
        done: bool,
    },
    /// The actual answer/content
    Content(String),
    /// Error during streaming
    Error(String),
}

#[derive(Debug, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub max_tokens: usize,
    pub temperature: f32,
    pub stream: bool,
}

#[derive(Debug, Deserialize)]
pub struct StreamChunk {
    pub choices: Vec<StreamChoice>,
}

#[derive(Debug, Deserialize)]
pub struct StreamChoice {
    pub delta: StreamDelta,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct StreamDelta {
    pub role: Option<String>,
    pub content: Option<String>,
    pub reasoning_content: Option<String>,
}

pub struct StreamClient {
    client: reqwest::Client,
    api_url: String,
    model: String,
    api_key: String,
    max_tokens: usize,
    temperature: f32,
}

impl StreamClient {
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

    /// Stream AI response, yielding StreamItems as they arrive.
    /// This allows showing thinking steps as Discord messages before the final answer.
    pub async fn stream_chat(&self, messages: &[ChatMessage]) -> Vec<StreamItem> {
        let request = ChatRequest {
            model: self.model.clone(),
            messages: messages.to_vec(),
            max_tokens: self.max_tokens,
            temperature: self.temperature,
            stream: true,
        };

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
                if !resp.status().is_success() {
                    let status = resp.status().as_u16();
                    let body = resp.text().await.unwrap_or_default();
                    error!("Stream API error: {} - {}", status, body);
                    return vec![StreamItem::Error(format!("API error {}: {}", status, body))];
                }

                // Collect all bytes from the stream, then parse SSE chunks
                let all_bytes: Vec<bytes::Bytes> = match resp.bytes_stream().try_collect().await {
                    Ok(b) => b,
                    Err(e) => {
                        error!("Failed to collect stream bytes: {}", e);
                        return vec![StreamItem::Error(format!("Stream read error: {}", e))];
                    }
                };
                // Concatenate all chunks into a single Vec<u8> for parsing
                let concatenated: Vec<u8> = all_bytes.iter().flat_map(|b| b.to_vec()).collect();

                Self::parse_sse_stream(&concatenated)
            }
            Err(e) => {
                error!("Stream request failed: {}", e);
                vec![StreamItem::Error(format!("Request failed: {}", e))]
            }
        }
    }

    /// Non-streaming chat (fallback for simple responses)
    pub async fn chat(&self, messages: &[ChatMessage]) -> Result<String, Box<dyn std::error::Error>> {
        let request = super::client::ChatRequest {
            model: self.model.clone(),
            messages: messages.to_vec(),
            max_tokens: self.max_tokens,
            temperature: self.temperature,
        };

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
                    let chat_response: super::client::ChatResponse = resp.json().await?;
                    if let Some(choice) = chat_response.choices.first() {
                        Ok(choice.message.content.clone())
                    } else {
                        warn!("Stream client response has no choices");
                        Err("No response from AI".into())
                    }
                } else {
                    let status = resp.status().as_u16();
                    let body = resp.text().await.unwrap_or_default();
                    error!("Stream client API error: {} - {}", status, body);
                    Err(format!("API error {}: {}", status, body).into())
                }
            }
            Err(e) => {
                error!("Stream client request failed: {}", e);
                Err(format!("Request failed: {}", e).into())
            }
        }
    }

    fn parse_sse_stream(data: &[u8]) -> Vec<StreamItem> {
        let text = match String::from_utf8(data.to_vec()) {
            Ok(t) => t,
            Err(_) => return vec![StreamItem::Error("Failed to parse stream as UTF-8".to_string())],
        };

        let mut items = Vec::new();
        let mut full_reasoning = String::new();
        let mut content_parts: Vec<String> = Vec::new();

        for line in text.lines() {
            if !line.starts_with("data: ") {
                continue;
            }
            let data_str = &line[6..];
            if data_str == "[DONE]" {
                if !full_reasoning.is_empty() {
                    items.push(StreamItem::Reasoning {
                        content: full_reasoning.clone(),
                        done: true,
                    });
                }
                break;
            }

            let chunk: StreamChunk = match serde_json::from_str(data_str) {
                Ok(c) => c,
                Err(_) => continue,
            };

            for choice in chunk.choices {
                let delta = choice.delta;

                // Handle reasoning_content (OpenAI-style thinking)
                if let Some(reasoning) = delta.reasoning_content {
                    if !reasoning.is_empty() {
                        full_reasoning.push_str(&reasoning);
                        // Push a StreamItem with the full accumulated reasoning
                        items.push(StreamItem::Reasoning {
                            content: full_reasoning.clone(),
                            done: false,
                        });
                    }
                }

                // Handle regular content
                if let Some(content) = delta.content {
                    if !content.is_empty() {
                        content_parts.push(content);
                    }
                }
            }
        }

        // Add a final content item if we have accumulated content
        if !content_parts.is_empty() {
            items.push(StreamItem::Content(content_parts.join("")));
        }

        items
    }
}
