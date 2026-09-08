//! Integration tests for openrouter_free
//!
//! Network tests are gated behind OPENROUTER_API_KEY.
//! Run with:
//!   OPENROUTER_API_KEY=sk-or-... cargo test --test integration -- --nocapture
//!
//! Or skip network tests entirely:
//!   cargo test --test integration -- --skip network

use openrouter_free::{
    OpenRouterFreeClient, ChatRequest, Message, Role, Tool, ToolFunction, ToolChoice,
};
use serde_json::json;

fn api_key() -> Option<String> {
    std::env::var("OPENROUTER_API_KEY").ok().filter(|k| !k.is_empty() && k != "INVALID_KEY")
}

// ---------------------------------------------------------------------------
// Unit-style tests (no network)
// ---------------------------------------------------------------------------

#[test]
fn chat_request_default_is_empty() {
    let req = ChatRequest::default();
    assert!(req.messages.is_empty());
    assert!(req.model.is_none());
    assert!(req.tools.is_none());
}

#[test]
fn message_serializes_role_lowercase() {
    let msg = Message {
        role: Role::User,
        content: "ping".into(),
    };
    let v = serde_json::to_value(&msg).unwrap();
    assert_eq!(v["role"], "user");
    assert_eq!(v["content"], "ping");
}

#[test]
fn tool_choice_simple_serializes() {
    let tc = ToolChoice::Simple("auto".into());
    let v = serde_json::to_value(&tc).unwrap();
    assert_eq!(v, "auto");
}

#[test]
fn client_new_accepts_string() {
    let _client = OpenRouterFreeClient::new("test-key");
}

// ---------------------------------------------------------------------------
// Network tests (require OPENROUTER_API_KEY)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn network_basic_chat() {
    let Some(key) = api_key() else {
        eprintln!("Skipping network_basic_chat: OPENROUTER_API_KEY not set");
        return;
    };

    let client = OpenRouterFreeClient::new(key);

    let req = ChatRequest {
        model: None,
        messages: vec![Message {
            role: Role::User,
            content: "Reply with exactly the word: pong".into(),
        }],
        max_tokens: Some(16),
        ..Default::default()
    };

    let res = client.chat(req).await.expect("chat should succeed");
    assert!(!res.model.is_empty(), "model should be set");
    assert!(!res.choices.is_empty(), "should have at least one choice");
    assert!(!res.choices[0].message.content.is_empty(), "content should not be empty");
}

#[tokio::test]
async fn network_streaming() {
    let Some(key) = api_key() else {
        eprintln!("Skipping network_streaming: OPENROUTER_API_KEY not set");
        return;
    };

    use futures::StreamExt;

    let client = OpenRouterFreeClient::new(key);

    let req = ChatRequest {
        model: None,
        messages: vec![Message {
            role: Role::User,
            content: "Say hello in one short sentence.".into(),
        }],
        max_tokens: Some(32),
        ..Default::default()
    };

    let mut stream = client.chat_stream(req).await.expect("stream should open");
    let mut got_content = false;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.expect("chunk should be ok");
        if let Some(choices) = chunk.choices {
            if let Some(delta) = &choices[0].delta {
                if delta.content.as_ref().map(|c| !c.is_empty()).unwrap_or(false) {
                    got_content = true;
                    break;
                }
            }
        }
    }

    assert!(got_content, "stream should yield at least some content");
}

#[tokio::test]
async fn network_tools() {
    let Some(key) = api_key() else {
        eprintln!("Skipping network_tools: OPENROUTER_API_KEY not set");
        return;
    };

    let client = OpenRouterFreeClient::new(key);

    let tools = vec![Tool {
        kind: "function".into(),
        function: ToolFunction {
            name: "add_numbers".into(),
            description: Some("Add two numbers".into()),
            parameters: Some(json!({
                "type": "object",
                "properties": {
                    "a": { "type": "number" },
                    "b": { "type": "number" }
                },
                "required": ["a", "b"]
            })),
        },
    }];

    let req = ChatRequest {
        model: None,
        messages: vec![Message {
            role: Role::User,
            content: "Use the add_numbers tool with a=3 and b=4".into(),
        }],
        tools: Some(tools),
        tool_choice: Some(ToolChoice::Simple("auto".into())),
        max_tokens: Some(128),
        ..Default::default()
    };

    let res = client.chat(req).await.expect("tool chat should succeed");
    assert!(!res.choices.is_empty());
    // Model may either call the tool or answer directly; both are acceptable.
}

#[tokio::test]
async fn network_invalid_key_returns_error() {
    let client = OpenRouterFreeClient::new("INVALID_KEY");

    let req = ChatRequest {
        messages: vec![Message {
            role: Role::User,
            content: "This should fail".into(),
        }],
        ..Default::default()
    };

    let err = client.chat(req).await.expect_err("invalid key must error");
    let msg = err.to_string();
    assert!(
        msg.contains("API error") || msg.contains("401") || msg.contains("Unauthorized") || msg.contains("auth"),
        "unexpected error shape: {msg}"
    );
}
