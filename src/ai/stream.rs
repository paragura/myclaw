use serde::{Deserialize, Serialize};
use tracing::{error, warn};
use futures::TryStreamExt;

use super::client::{ChatMessage, ToolDefinition};

/// A streaming chunk from the AI response.
#[derive(Debug, Clone)]
pub enum StreamItem {
    /// Intermediate thinking/reasoning steps
    Reasoning {
        content: String,
        done: bool,
    },
    /// The actual answer/content
    Content(String),
    /// A tool call from the AI
    ToolCall {
        id: String,
        name: String,
        arguments: String,
    },
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolDefinition>>,
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
    #[serde(default)]
    pub tool_calls: Option<Vec<StreamToolCall>>,
}

#[derive(Debug, Deserialize)]
pub struct StreamToolCall {
    pub index: Option<usize>,
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub r#type: Option<String>,
    pub function: Option<StreamFunctionCall>,
}

#[derive(Debug, Deserialize)]
pub struct StreamFunctionCall {
    pub name: Option<String>,
    pub arguments: Option<String>,
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

    /// Stream AI response with tool support, yielding StreamItems as they arrive.
    pub async fn stream_chat_with_tools(
        &self,
        messages: &[ChatMessage],
        tools: Option<Vec<ToolDefinition>>,
    ) -> Vec<StreamItem> {
        let request = ChatRequest {
            model: self.model.clone(),
            messages: messages.to_vec(),
            max_tokens: self.max_tokens,
            temperature: self.temperature,
            stream: true,
            tools,
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

                let all_bytes: Vec<bytes::Bytes> = match resp.bytes_stream().try_collect().await {
                    Ok(b) => b,
                    Err(e) => {
                        error!("Failed to collect stream bytes: {}", e);
                        return vec![StreamItem::Error(format!("Stream read error: {}", e))];
                    }
                };
                let concatenated: Vec<u8> = all_bytes.iter().flat_map(|b| b.to_vec()).collect();

                Self::parse_sse_stream(&concatenated)
            }
            Err(e) => {
                error!("Stream request failed: {}", e);
                vec![StreamItem::Error(format!("Request failed: {}", e))]
            }
        }
    }

    /// Stream AI response (without tools).
    pub async fn stream_chat(&self, messages: &[ChatMessage]) -> Vec<StreamItem> {
        self.stream_chat_with_tools(messages, None).await
    }

    /// Non-streaming chat (fallback for simple responses)
    pub async fn chat(&self, messages: &[ChatMessage]) -> Result<String, Box<dyn std::error::Error>> {
        let request = super::client::ChatRequest {
            model: self.model.clone(),
            messages: messages.to_vec(),
            max_tokens: self.max_tokens,
            temperature: self.temperature,
            tools: None,
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
        let _last_reasoning_len = 0;

        // Accumulate tool call state across SSE chunks
        let mut tool_calls: Vec<(String, String, String)> = Vec::new(); // (id, name, arguments)
        let mut pending_tool_calls: Vec<Option<(String, String, String)>> = Vec::new();

        for line in text.lines() {
            if !line.starts_with("data: ") {
                continue;
            }
            let data_str = &line[6..];
            if data_str == "[DONE]" {
                // Flush accumulated reasoning
                if !full_reasoning.is_empty() {
                    items.push(StreamItem::Reasoning {
                        content: full_reasoning.clone(),
                        done: true,
                    });
                }
                // Flush any remaining tool calls
                for (id, name, args) in tool_calls {
                    items.push(StreamItem::ToolCall { id, name, arguments: args });
                }
                break;
            }

            let chunk: StreamChunk = match serde_json::from_str(data_str) {
                Ok(c) => c,
                Err(_) => continue,
            };

            for choice in chunk.choices {
                let delta = choice.delta;

                // Handle reasoning_content
                if let Some(reasoning) = delta.reasoning_content {
                    if !reasoning.is_empty() {
                        full_reasoning.push_str(&reasoning);
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

                // Handle tool calls (incremental JSON)
                if let Some(tcs) = delta.tool_calls {
                    for tc in tcs {
                        let idx = tc.index.unwrap_or(0);
                        // Ensure vector is large enough
                        while pending_tool_calls.len() <= idx {
                            pending_tool_calls.push(None);
                        }
                        let entry = pending_tool_calls[idx].get_or_insert_with(|| (
                            tc.id.clone().unwrap_or_default(),
                            String::new(),
                            String::new(),
                        ));

                        if let Some(id) = &tc.id {
                            entry.0 = id.clone();
                        }
                        if let Some(name) = &tc.function.as_ref().and_then(|f| f.name.clone()) {
                            entry.1 = name.clone();
                        }
                        if let Some(args) = &tc.function.as_ref().and_then(|f| f.arguments.clone()) {
                            entry.2.push_str(args);
                        }
                    }
                }

                // When finish_reason is set, emit completed tool calls
                if choice.finish_reason.is_some() {
                    for opt in pending_tool_calls.drain(..) {
                        if let Some((id, name, args)) = opt {
                            tool_calls.push((id, name, args));
                        }
                    }
                }
            }
        }

        // Emit reasoning items with incremental display
        let mut emitted_reasoning = String::new();
        for item in &items {
            if let StreamItem::Reasoning { content, .. } = item {
                if content.len() > emitted_reasoning.len() {
                    let new_content = &content[emitted_reasoning.len()..];
                    if !new_content.trim().is_empty() {
                        // Don't add duplicate items, just track display
                    }
                }
                emitted_reasoning = content.clone();
            }
        }

        // Add content item
        if !content_parts.is_empty() {
            items.push(StreamItem::Content(content_parts.join("")));
        }

        items
    }
}
