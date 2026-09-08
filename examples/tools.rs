use openrouter_free::{
    OpenRouterFreeClient,
    ChatRequest,
    Message,
    Role,
    Tool,
    ToolFunction,
    ToolChoice,
};
use serde_json::json;
use anyhow::{Context, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let api_key = std::env::var("OPENROUTER_API_KEY")
        .context("OPENROUTER_API_KEY must be set")?;

    let client = OpenRouterFreeClient::new(api_key);

    let tools = vec![
        Tool {
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
        }
    ];

    let req = ChatRequest {
        model: None,
        messages: vec![
            Message {
                role: Role::User,
                content: "Use the add_numbers tool with a=3 and b=4".into(),
            }
        ],
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
        tools: Some(tools),
        tool_choice: Some(ToolChoice::Simple("auto".into())),
        logprobs: None,
        top_logprobs: None,
    };

    let res = client.chat(req).await
        .context("tool-enabled chat request failed")?;

    println!("Reply: {}", res.choices[0].message.content);

    Ok(())
}
