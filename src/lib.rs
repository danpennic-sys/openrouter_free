//! # openrouter_free
//!
//! Operator-grade, zero-friction Rust client for the [OpenRouter](https://openrouter.ai) free-model tier.
//!
//! Deterministic. Replay-safe. Built for autonomy runtimes.
//!
//! ## Quick Start
//!
//! ```no_run
//! use openrouter_free::{OpenRouterFreeClient, ChatRequest, Message, Role};
//!
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! let client = OpenRouterFreeClient::new(std::env::var("OPENROUTER_API_KEY")?);
//!
//! let req = ChatRequest {
//!     messages: vec![Message {
//!         role: Role::User,
//!         content: "Status report.".into(),
//!     }],
//!     ..Default::default()
//! };
//!
//! let res = client.chat(req).await?;
//! println!("{}", res.choices[0].message.content);
//! # Ok(())
//! # }
//! ```
//!
//! ## Features
//!
//! - Async-first (`tokio` + `reqwest`)
//! - Non-streaming and streaming chat completions
//! - Tool / function calling
//! - Explicit request structs (no hidden defaults)
//! - Clean, typed error surface

use futures::Stream;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use thiserror::Error;

const BASE_URL: &str = "https://openrouter.ai/api/v1";

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

/// Errors returned by the OpenRouter Free client.
#[derive(Debug, Error)]
pub enum Error {
    /// Underlying HTTP transport error.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// OpenRouter returned a non-success status code.
    #[error("API error ({status}): {message}")]
    Api {
        /// HTTP status code.
        status: u16,
        /// Response body or error message.
        message: String,
    },

    /// Failed to serialize or deserialize JSON.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Error while processing a streaming response.
    #[error("Stream error: {0}")]
    Stream(String),
}

/// Convenient result alias for this crate.
pub type Result<T> = std::result::Result<T, Error>;

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

/// Asynchronous client for the OpenRouter Chat Completions API.
///
/// Construct with [`OpenRouterFreeClient::new`] and an API key.
/// The client is cheap to clone and safe to share across tasks.
#[derive(Clone)]
pub struct OpenRouterFreeClient {
    http: Client,
    api_key: String,
}

impl OpenRouterFreeClient {
    /// Create a new client with the given OpenRouter API key.
    ///
    /// # Example
    ///
    /// ```
    /// use openrouter_free::OpenRouterFreeClient;
    /// let client = OpenRouterFreeClient::new("sk-or-...");
    /// ```
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            http: Client::new(),
            api_key: api_key.into(),
        }
    }

    /// Send a non-streaming chat completion request.
    ///
    /// Returns the full [`ChatResponse`] once generation is complete.
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

    /// Send a streaming chat completion request.
    ///
    /// Returns a stream of [`ChatStreamChunk`]s. The stream ends when the
    /// server sends the final `[DONE]` marker or an error occurs.
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

/// Chat completion request.
///
/// All fields except `messages` are optional. Unset fields are omitted from
/// the JSON payload so OpenRouter applies its own defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    /// Model ID. When `None`, OpenRouter selects a free-tier model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Conversation messages.
    pub messages: Vec<Message>,

    /// Whether to stream the response. Controlled automatically by the client methods.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,

    /// Maximum number of tokens to generate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,

    /// Alternative to `max_tokens` used by some models.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_completion_tokens: Option<u32>,

    /// Sampling temperature (0.0 – 2.0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    /// Nucleus sampling probability mass.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,

    /// Top-k sampling.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,

    /// Minimum probability threshold.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_p: Option<f32>,

    /// Stop sequences.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,

    /// Frequency penalty.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,

    /// Presence penalty.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,

    /// Repetition penalty.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repetition_penalty: Option<f32>,

    /// Random seed for deterministic sampling.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,

    /// Enable reasoning / chain-of-thought (model-dependent).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<bool>,

    /// Reasoning effort level (model-dependent).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,

    /// Response format (e.g. JSON mode).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<serde_json::Value>,

    /// Tools available to the model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,

    /// Controls which tool the model should call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,

    /// Whether to return log probabilities.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<bool>,

    /// Number of top log probabilities to return.
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

/// A single message in a chat conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// The role of the message author.
    pub role: Role,
    /// The content of the message.
    pub content: String,
}

/// Role of a message author.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// System instructions.
    System,
    /// End-user input.
    User,
    /// Model response.
    Assistant,
    /// Tool / function result.
    Tool,
}

/// A tool the model may call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    /// Currently always `"function"`.
    #[serde(rename = "type")]
    pub kind: String,
    /// Function definition.
    pub function: ToolFunction,
}

/// Definition of a callable function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFunction {
    /// Function name.
    pub name: String,
    /// Human-readable description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// JSON Schema for the parameters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
}

/// Controls tool selection behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolChoice {
    /// Simple string form (`"none"`, `"auto"`, or a function name).
    Simple(String),
}

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

/// Full non-streaming chat completion response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    /// Unique response identifier.
    pub id: Option<String>,
    /// Model that produced the response.
    pub model: String,
    /// Generated choices.
    pub choices: Vec<Choice>,
    /// Token usage statistics.
    pub usage: Option<Usage>,
}

/// A single completion choice.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Choice {
    /// Index of this choice.
    pub index: Option<u32>,
    /// The generated message.
    pub message: ResponseMessage,
    /// Why generation stopped.
    pub finish_reason: Option<String>,
}

/// Message returned by the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseMessage {
    /// Role (usually `"assistant"`).
    pub role: String,
    /// Generated text content.
    pub content: String,
    /// Any tool calls requested by the model.
    #[serde(default)]
    pub tool_calls: Option<Vec<serde_json::Value>>,
}

/// Token usage statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    /// Tokens in the prompt.
    pub prompt_tokens: Option<u32>,
    /// Tokens in the completion.
    pub completion_tokens: Option<u32>,
    /// Total tokens used.
    pub total_tokens: Option<u32>,
}

/// A single chunk from a streaming response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatStreamChunk {
    /// Response identifier.
    pub id: Option<String>,
    /// Model name.
    pub model: Option<String>,
    /// Partial choices.
    pub choices: Option<Vec<StreamChoice>>,
}

/// A partial choice in a stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChoice {
    /// Index of this choice.
    pub index: Option<u32>,
    /// Incremental content delta.
    pub delta: Option<Delta>,
    /// Why generation stopped (present on final chunk).
    pub finish_reason: Option<String>,
}

/// Incremental update in a stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Delta {
    /// Role (usually present only on the first chunk).
    pub role: Option<String>,
    /// New text content.
    pub content: Option<String>,
}
