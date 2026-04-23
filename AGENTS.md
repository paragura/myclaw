# myclaw

Personal AI assistant bot for Discord. Rust + serenity + SQLite.

## Setup

1. Copy `.env.example` to `.env` and fill in your values
2. `config.toml` has defaults — env vars take priority
3. `cargo run --release`

## Secrets

Sensitive values live in `.env` (gitignored). Environment variables override `config.toml`:

| Env var | config.toml key | Description |
|---|---|---|
| `DISCORD_BOT_TOKEN` | `[bot].token` | Discord bot token |
| `AI_API_URL` | `[ai].api_url` | OpenAI-compatible API endpoint |
| `AI_MODEL` | `[ai].model` | Model name |
| `AI_API_KEY` | `[ai].api_key` | API key |

Non-secret settings stay in `config.toml` only.

## Architecture

- `src/ai/` — AI client (streaming + non-streaming)
- `src/ai/stream.rs` — SSE parser with tool call support
- `src/discord/` — Discord handler, commands
- `src/skills/` — Skills manager + markdown skills
- `src/tools/` — Executable tools (shell_exec, file_read, file_write, web_fetch)
- `src/agent/` — Sub-agent system
- `src/memory/` — SQLite memory store
- `src/config/` — Config loading (toml + env override)

## Tool Use

The bot uses OpenAI-style function calling. Tool definitions are sent with every request and the AI can invoke them in a multi-turn loop:

```
User → AI → tool_calls → execute → results → AI → final answer
```
