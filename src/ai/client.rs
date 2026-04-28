use serde::{Deserialize, Serialize};
use tracing::{debug, error, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub r#type: String,
    pub function: FunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,  // JSON string
}

#[derive(Debug, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub max_tokens: usize,
    pub temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolDefinition>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolDefinition {
    #[serde(rename = "type")]
    pub r#type: String,
    pub function: FunctionDefinition,
}

#[derive(Debug, Clone, Serialize)]
pub struct FunctionDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct ChatResponse {
    pub choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
pub struct Choice {
    pub message: ChatMessage,
    #[serde(skip_deserializing, default)]
    pub finish_reason: Option<String>,
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
            tools: None,
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

    /// Chat with tool support. Returns the full response including potential tool calls.
    pub async fn chat_with_tools(
        &self,
        messages: &[ChatMessage],
        tools: Option<Vec<ToolDefinition>>,
    ) -> Result<ChatMessage, Box<dyn std::error::Error>> {
        let request = ChatRequest {
            model: self.model.clone(),
            messages: messages.to_vec(),
            max_tokens: self.max_tokens,
            temperature: self.temperature,
            tools,
        };

        debug!("Sending AI request with tools to {} with {} messages", self.api_url, messages.len());

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
                        Ok(choice.message.clone())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_message_serialization() {
        let msg = ChatMessage {
            role: "user".to_string(),
            content: "Hello".to_string(),
            tool_calls: None,
            tool_call_id: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"role\":\"user\""));
        assert!(json.contains("\"content\":\"Hello\""));
    }

    #[test]
    fn test_chat_message_with_tool_calls() {
        let msg = ChatMessage {
            role: "assistant".to_string(),
            content: "".to_string(),
            tool_calls: Some(vec![ToolCall {
                id: "call_1".to_string(),
                r#type: "function".to_string(),
                function: FunctionCall {
                    name: "file_read".to_string(),
                    arguments: r#"{"command":"test.txt"}"#.to_string(),
                },
            }]),
            tool_call_id: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("tool_calls"));
        assert!(json.contains("file_read"));
    }

    #[test]
    fn test_chat_message_tool_call_id_skipped_when_none() {
        let msg = ChatMessage {
            role: "assistant".to_string(),
            content: "result".to_string(),
            tool_calls: None,
            tool_call_id: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(!json.contains("tool_call_id"));
    }

    #[test]
    fn test_tool_definition_serialization() {
        let def = ToolDefinition {
            r#type: "function".to_string(),
            function: FunctionDefinition {
                name: "shell_exec".to_string(),
                description: "Execute a shell command".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "command": {"type": "string"}
                    }
                }),
            },
        };
        let json = serde_json::to_string(&def).unwrap();
        assert!(json.contains("\"type\":\"function\""));
        assert!(json.contains("shell_exec"));
    }

    #[test]
    fn test_chat_request_serialization() {
        let req = ChatRequest {
            model: "test-model".to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: "hi".to_string(),
                tool_calls: None,
                tool_call_id: None,
            }],
            max_tokens: 1024,
            temperature: 0.7,
            tools: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"model\":\"test-model\""));
        assert!(json.contains("\"max_tokens\":1024"));
        assert!(json.contains("\"temperature\":0.7"));
        assert!(!json.contains("tools"));
    }

    #[test]
    fn test_chat_request_with_tools() {
        let req = ChatRequest {
            model: "test".to_string(),
            messages: vec![],
            max_tokens: 512,
            temperature: 0.0,
            tools: Some(vec![
                ToolDefinition {
                    r#type: "function".to_string(),
                    function: FunctionDefinition {
                        name: "test".to_string(),
                        description: "desc".to_string(),
                        parameters: serde_json::json!({}),
                    },
                },
            ]),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("tools"));
    }

    #[test]
    fn test_chat_response_deserialization() {
        let json = r#"{
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "Hello world"
                }
            }]
        }"#;
        let resp: ChatResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.choices.len(), 1);
        assert_eq!(resp.choices[0].message.content, "Hello world");
    }

    #[test]
    fn test_chat_response_empty_choices() {
        let json = r#"{"choices": []}"#;
        let resp: ChatResponse = serde_json::from_str(json).unwrap();
        assert!(resp.choices.is_empty());
    }

    #[test]
    fn test_function_call_serialization() {
        let fc = FunctionCall {
            name: "web_fetch".to_string(),
            arguments: r#"{"url":"https://example.com"}"#.to_string(),
        };
        let json = serde_json::to_string(&fc).unwrap();
        assert!(json.contains("web_fetch"));
        assert!(json.contains("https://example.com"));
    }
}
