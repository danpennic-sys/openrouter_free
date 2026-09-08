use openrouter_free::{
    OpenRouterFreeClient,
    ChatRequest,
    Message,
    Role,
};
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // Intentionally invalid key
    let client = OpenRouterFreeClient::new("INVALID_KEY".to_string());

    let req = ChatRequest {
        model: None,
        messages: vec![
            Message {
                role: Role::User,
                content: "Trigger an auth error.".into(),
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
        tools: None,
        tool_choice: None,
        logprobs: None,
        top_logprobs: None,
    };

    match client.chat(req).await {
        Ok(_) => println!("Unexpected success"),
        Err(e) => println!("Expected error: {e}"),
    }

    Ok(())
}
