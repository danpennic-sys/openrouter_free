//! openrouter_free
//!
//! Operator-grade, zero-friction Rust client for the OpenRouter free-model tier.
//! Deterministic. Replay-safe. Built for autonomy runtimes.

use futures::Stream;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use thiserror::Error;

const BASE_URL: &str = "https://openrouter.ai/api/v1";

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum Error {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("API error ({status}): {message}")]
    Api { status: u16, message: String },

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Stream error: {0}")]
    Stream(String),
}

pub type Result<T> = std::result::Result<T, Error>;

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct OpenRouterFreeClient {
    http: Client,
    api_key: String,
}

impl OpenRouterFreeClient {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            http: Client::new(),
            api_key: api_key.into(),
        }
    }

    /// Non-streaming chat completion.
    pub async fn chat(&self, req: ChatRequest) -> Result<ChatResponse> {
        let mut body = serde_json::to_value(&req)?;
        // Force non-stream
        if let Some(obj) = body.as_object_mut() {
            obj.insert("stream".to_string(), serde_json::Value::Bool(false));
        }

        let resp = self
            .http
            .post(format!("{BASE_URL}/chat/completions"))
            .bearer_auth(&self.api_key)
            .header("HTTP-Referer", "https://github.com/danpennic-sys/openrouter_free")
            .header("X-Title", "openrouter_free")
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let message = resp.text().await.unwrap_or_else(|_| "unknown error".into());
            return Err(Error::Api {
                status: status.as_u16(),
                message,
            });
        }

        let parsed: ChatResponse = resp.json().await?;
        Ok(parsed)
    }

    /// Streaming chat completion. Returns a stream of ChatStreamChunk.
    pub async fn chat_stream(
        &self,
        req: ChatRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatStreamChunk>> + Send>>> {
        let mut body = serde_json::to_value(&req)?;
        if let Some(obj) = body.as_object_mut() {
            obj.insert("stream".to_string(), serde_json::Value::Bool(true));
        }

        let resp = self
            .http
            .post(format!("{BASE_URL}/chat/completions"))
            .bearer_auth(&self.api_key)
            .header("HTTP-Referer", "https://github.com/danpennic-sys/openrouter_free")
            .header("X-Title", "openrouter_free")
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let message = resp.text().await.unwrap_or_else(|_| "unknown error".into());
            return Err(Error::Api {
                status: status.as_u16(),
                message,
            });
        }

        let byte_stream = resp.bytes_stream();

        let stream = async_stream::stream! {
            use futures::StreamExt;
            let mut buffer = String::new();

            let mut byte_stream = byte_stream;
            while let Some(item) = byte_stream.next().await {
                match item {
                    Ok(bytes) => {
                        buffer.push_str(&String::from_utf8_lossy(&bytes));

                        while let Some(pos) = buffer.find("\n") {
                            let line = buffer[..pos].trim().to_string();
                            buffer = buffer[pos + 1..].to_string();

                            if line.is_empty() || line == "data: [DONE]" {
                                continue;
                            }

                            let data = if let Some(stripped) = line.strip_prefix("data: ") {
                                stripped
                            } else {
                                &line
                            };

                            if data == "[DONE]" {
                                break;
                            }

                            match serde_json::from_str::<ChatStreamChunk>(data) {
                                Ok(chunk) => yield Ok(chunk),
                                Err(e) => yield Err(Error::Stream(format!("parse error: {e} | data: {data}"))),
                            }
                        }
                    }
                    Err(e) => {
                        yield Err(Error::Http(e));
                        break;
                    }
                }
            }
        };

        Ok(Box::pin(stream))
    }
}

// ---------------------------------------------------------------------------
// Request types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    pub messages: Vec<Message>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_completion_tokens: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_p: Option<f32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub repetition_penalty: Option<f32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<serde_json::Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_logprobs: Option<u32>,
}

impl Default for ChatRequest {
    fn default() -> Self {
        Self {
            model: None,
            messages: vec![],
            stream: None,
            max_tokens: None,
            max_completion_tokens: None,
            temperature: None,
            top_p: None,
            top_k: None,
            min_p: None,
            stop: None,
            frequency_penalty: None,
            presence_penalty: None,
            repetition_penalty: None,
            seed: None,
            reasoning: None,
            reasoning_effort: None,
            response_format: None,
            tools: None,
            tool_choice: None,
            logprobs: None,
            top_logprobs: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    #[serde(rename = "type")]
    pub kind: String,
    pub function: ToolFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFunction {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolChoice {
    Simple(String),
    // Future: object form if needed
}

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub id: Option<String>,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Choice {
    pub index: Option<u32>,
    pub message: ResponseMessage,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseMessage {
    pub role: String,
    pub content: String,
    #[serde(default)]
    pub tool_calls: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
}

// Streaming

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatStreamChunk {
    pub id: Option<String>,
    pub model: Option<String>,
    pub choices: Option<Vec<StreamChoice>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChoice {
    pub index: Option<u32>,
    pub delta: Option<Delta>,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Delta {
    pub role: Option<String>,
    pub content: Option<String>,
}
