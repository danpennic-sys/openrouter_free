use openrouter_free::{
    OpenRouterFreeClient,
    ChatRequest,
    Message,
    Role,
};
use futures::StreamExt;
use anyhow::{Context, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let api_key = std::env::var("OPENROUTER_API_KEY")
        .context("OPENROUTER_API_KEY must be set")?;

    let client = OpenRouterFreeClient::new(api_key);

    let req = ChatRequest {
        model: None,
        messages: vec![
            Message {
                role: Role::User,
                content: "Stream a short poem about autonomy systems.".into(),
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

    let mut stream = client.chat_stream(req)
        .await
        .context("failed to open stream")?;

    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(c) => {
                if let Some(choices) = c.choices {
                    if let Some(delta) = &choices[0].delta {
                        if let Some(content) = &delta.content {
                            print!("{}", content);
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Stream error: {e}");
                break;
            }
        }
    }

    println!(); // final newline
    Ok(())
}
