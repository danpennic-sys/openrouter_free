# openrouter_free

Operator-grade, zero-friction Rust client for the OpenRouter free-model tier.

Deterministic. Replay-safe. Built for autonomy runtimes.

## Features

- Async-first (`tokio` + `reqwest`)
- Full chat + streaming support
- Tool calling / function calling
- Explicit, exhaustive request structs (no magic defaults that surprise you at 03:00)
- Clean error surface
- Zero required configuration beyond `OPENROUTER_API_KEY`

## Quick Start

```bash
export OPENROUTER_API_KEY=sk-or-...
cargo run --example basic_chat
```

## Examples

All examples are self-contained and live under `examples/`.

| Example              | Command                              | Purpose                          |
|----------------------|--------------------------------------|----------------------------------|
| `basic_chat`         | `cargo run --example basic_chat`     | Minimal non-streaming request    |
| `streaming`          | `cargo run --example streaming`      | Token-by-token streaming         |
| `tools`              | `cargo run --example tools`          | Tool declaration + `tool_choice` |
| `error_handling`     | `cargo run --example error_handling` | Auth / API error surface         |

## Usage

```rust
use openrouter_free::{
    OpenRouterFreeClient,
    ChatRequest,
    Message,
    Role,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = OpenRouterFreeClient::new(
        std::env::var("OPENROUTER_API_KEY")?
    );

    let req = ChatRequest {
        model: None, // free-tier default
        messages: vec![
            Message {
                role: Role::User,
                content: "Status report.".into(),
            }
        ],
        ..Default::default()
    };

    let res = client.chat(req).await?;
    println!("{}", res.choices[0].message.content);
    Ok(())
}
```

## Design Notes

- Every field on `ChatRequest` is explicit. No silent defaults that change behavior between versions.
- Streaming uses `futures::Stream`.
- Tool calling follows the OpenRouter / OpenAI-compatible schema.
- Errors are returned as-is; no over-wrapping.

## License

MIT OR Apache-2.0
