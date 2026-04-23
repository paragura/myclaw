use serenity::prelude::*;
use serenity::model::channel::Message;
use tracing::{info, error};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::ai::client::{AIClient, ChatMessage};
use crate::ai::stream::{StreamClient, StreamItem};
use crate::ai::context::ContextManager;
use crate::memory::store::MemoryStore;
use crate::coding::executor::CodingExecutor;
use crate::skills::manager::SkillsManager;
use crate::agent::manager::AgentManager;
use super::commands;

pub struct BotHandler {
    pub ai_client: Arc<AIClient>,
    pub stream_client: Arc<StreamClient>,
    pub context_mgr: Arc<ContextManager>,
    pub store: Arc<MemoryStore>,
    pub coding_exec: Arc<CodingExecutor>,
    pub skills_mgr: Arc<SkillsManager>,
    pub agent_mgr: Arc<AgentManager>,
    pub prefix: String,
    pub always_respond_channels: Arc<Mutex<Vec<String>>>,
    /// Tracks which channels have pending thinking messages to avoid spam
    pub pending_thinking: Arc<Mutex<Vec<String>>>,
}

impl BotHandler {
    pub fn new(
        ai_client: Arc<AIClient>,
        stream_client: Arc<StreamClient>,
        context_mgr: Arc<ContextManager>,
        store: Arc<MemoryStore>,
        coding_exec: Arc<CodingExecutor>,
        skills_mgr: Arc<SkillsManager>,
        agent_mgr: Arc<AgentManager>,
        prefix: String,
        always_respond_channels: Vec<String>,
    ) -> Self {
        Self {
            ai_client,
            stream_client,
            context_mgr,
            store,
            coding_exec,
            skills_mgr,
            agent_mgr,
            prefix,
            always_respond_channels: Arc::new(Mutex::new(always_respond_channels)),
            pending_thinking: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub async fn seed_channel_config(&self) {
        let channels = self.always_respond_channels.lock().await;
        for ch in channels.iter() {
            self.store.add_always_respond_channel(ch).await;
        }
    }
}

#[serenity::async_trait]
impl EventHandler for BotHandler {
    async fn message(&self, ctx: Context, msg: Message) {
        // Ignore bot's own messages
        if msg.author.bot {
            return;
        }

        // Handle DMs - free chat mode (no guild_id means DM)
        if msg.guild_id.is_none() {
            let ctx_clone = ctx.clone();
            let msg_clone = msg.clone();
            let ai_client = self.ai_client.clone();
            let stream_client = self.stream_client.clone();
            let context_mgr = self.context_mgr.clone();
            let skills_mgr = self.skills_mgr.clone();
            let store = self.store.clone();
            let pending = self.pending_thinking.clone();
            tokio::spawn(async move {
                handle_free_chat_thinking(&ctx_clone, &msg_clone, &ai_client, &stream_client, &context_mgr, &skills_mgr, &store, &pending).await;
            });
            return;
        }

        // Check if message starts with prefix
        if msg.content.starts_with(&self.prefix) {
            let without_prefix = &msg.content[self.prefix.len()..];
            let args: Vec<String> = without_prefix
                .split_whitespace()
                .map(|s| s.to_string())
                .collect();

            if !args.is_empty() {
                // Handle skill commands directly here before general commands
                if args[0] == "skill" {
                    self.handle_skill_command(&ctx, &msg, &args).await;
                    return;
                }
                if args[0] == "skills" {
                    self.handle_skills_list(&ctx, &msg).await;
                    return;
                }
                if args[0] == "agent" {
                    self.handle_agent_command(&ctx, &msg, &args).await;
                    return;
                }
                if args[0] == "channel" {
                    self.handle_channel_command(&ctx, &msg, &args).await;
                    return;
                }
                if args[0] == "think" {
                    let ctx_clone = ctx.clone();
                    let msg_clone = msg.clone();
                    let stream_client = self.stream_client.clone();
                    let context_mgr = self.context_mgr.clone();
                    let query = args[1..].join(" ");
                    if !query.is_empty() {
                        tokio::spawn(async move {
                            handle_thinking_chat(&ctx_clone, &msg_clone, &query, &stream_client, &context_mgr).await;
                        });
                    }
                    return;
                }

                commands::handle_command(
                    &ctx,
                    &msg,
                    &args,
                    &self.ai_client,
                    &self.stream_client,
                    &self.context_mgr,
                    &self.store,
                    &self.coding_exec,
                    &self.skills_mgr,
                    &self.agent_mgr,
                )
                .await;
            }
            return;
        }

        // Check if this channel has always_respond enabled
        let channel_id_str = msg.channel_id.to_string();
        let channels = self.always_respond_channels.lock().await;
        let is_always_respond = channels.contains(&channel_id_str);
        drop(channels);
        if is_always_respond {
            let ctx_clone = ctx.clone();
            let msg_clone = msg.clone();
            let ai_client = self.ai_client.clone();
            let stream_client = self.stream_client.clone();
            let context_mgr = self.context_mgr.clone();
            let skills_mgr = self.skills_mgr.clone();
            let store = self.store.clone();
            let pending = self.pending_thinking.clone();
            tokio::spawn(async move {
                handle_free_chat_thinking(&ctx_clone, &msg_clone, &ai_client, &stream_client, &context_mgr, &skills_mgr, &store, &pending).await;
            });
            return;
        }

        // Check for bot mention - respond to mentions
        let current_user_id = ctx.cache.current_user().id;

        let is_mentioned = msg.mentions.iter().any(|m| m.id == current_user_id);
        if is_mentioned {
            let ctx_clone = ctx.clone();
            let msg_clone = msg.clone();
            let ai_client = self.ai_client.clone();
            let stream_client = self.stream_client.clone();
            let context_mgr = self.context_mgr.clone();
            let skills_mgr = self.skills_mgr.clone();
            let store = self.store.clone();
            let pending = self.pending_thinking.clone();
            tokio::spawn(async move {
                handle_free_chat_thinking(&ctx_clone, &msg_clone, &ai_client, &stream_client, &context_mgr, &skills_mgr, &store, &pending).await;
            });
        }
    }
}

// --- Command handlers for new features ---

impl BotHandler {
    async fn handle_skill_command(&self, ctx: &Context, msg: &Message, args: &[String]) {
        if args.len() < 3 {
            let _ = msg.channel_id.say(&ctx.http, "使い方を教えてみます: `!skill search_memories Qwen`").await;
            return;
        }
        let skill_name = &args[1];
        let skill_args = args[2..].join(" ");

        let _ = msg.channel_id.say(&ctx.http, format!("🔧 スキル `{}` を実行中...", skill_name)).await;

        let output = self.skills_mgr.invoke(skill_name, &skill_args, &self.store).await;
        let _ = msg.channel_id.say(&ctx.http, &output).await;
    }

    async fn handle_skills_list(&self, ctx: &Context, msg: &Message) {
        let descriptions = self.skills_mgr.list_descriptions();
        let text = format!("**利用可能なスキル:**\n{}", descriptions);
        let _ = msg.channel_id.say(&ctx.http, &text).await;
    }

    async fn handle_agent_command(&self, ctx: &Context, msg: &Message, args: &[String]) {
        if args.len() < 2 {
            let _ = msg.channel_id.say(&ctx.http, "使い方:\n`!agent <名前> <タスク>` - エージェントを起動\n`!agent list` - 一覧\n`!agent stop <ID>` - 停止").await;
            return;
        }

        match args[1].to_lowercase().as_str() {
            "list" => {
                let agents = self.agent_mgr.list_agents().await;
                if agents.is_empty() {
                    let _ = msg.channel_id.say(&ctx.http, "実行中のエージェントはありません。").await;
                } else {
                    let agent_list: Vec<String> = agents
                        .iter()
                        .map(|a| format!("`{}`: {} [{}]", a.id.chars().take(8).collect::<String>(), a.name, a.status))
                        .collect();
                    let response = format!("**エージェント一覧** ({}体)\n{}", agents.len(), agent_list.join("\n"));
                    let _ = msg.channel_id.say(&ctx.http, &response).await;
                }
            }
            "stop" => {
                if args.len() < 3 {
                    let _ = msg.channel_id.say(&ctx.http, "停止するエージェントのIDを入力してください。").await;
                    return;
                }
                let stopped = self.agent_mgr.interrupt_agent(&args[2]).await;
                if stopped {
                    let _ = msg.channel_id.say(&ctx.http, format!("エージェント `{}` を停止しました。", args[2])).await;
                } else {
                    let _ = msg.channel_id.say(&ctx.http, format!("エージェント `{}` は見つかりませんでした。", args[2])).await;
                }
            }
            _ => {
                // Spawn a new agent
                if args.len() < 3 {
                    let _ = msg.channel_id.say(&ctx.http, "エージェントのタスクを入力してください: `!agent 調査员 Qwen3.6の仕様を調べて`").await;
                    return;
                }

                let agent_name = &args[1];
                let task_prompt = args[2..].join(" ");

                let thinking_sender = {
                    let http = ctx.http.clone();
                    let channel = msg.channel_id;
                    move |text: &str| {
                        let http = http.clone();
                        let channel = channel;
                        let text = text.to_string();
                        tokio::spawn(async move {
                            let _ = channel.say(&http, &text).await;
                        });
                    }
                };

                let _ = msg.channel_id.say(&ctx.http, format!("🧠 エージェント「{}」を起動します...", agent_name)).await;

                let agent_id = self.agent_mgr.spawn_agent(
                    agent_name,
                    &task_prompt,
                    self.ai_client.clone(),
                    self.stream_client.clone(),
                    self.store.clone(),
                    self.skills_mgr.clone(),
                    thinking_sender,
                    move |_id, status| {
                        info!("[Agent] Status changed: {}", status);
                    },
                ).await;

                let _ = msg.channel_id.say(&ctx.http, format!("エージェントID: `{}`", agent_id.chars().take(8).collect::<String>())).await;
            }
        }
    }
}

// --- Channel config commands ---

impl BotHandler {
    async fn handle_channel_command(&self, ctx: &Context, msg: &Message, args: &[String]) {
        if args.len() < 2 {
            let _ = msg.channel_id.say(&ctx.http, "使い方:\n`!channel list` - 一覧\n`!channel toggle` - 現在のチャンネルを切り替え\n`!channel add <ID>` - 追加\n`!channel remove <ID>` - 削除").await;
            return;
        }

        match args[1].to_lowercase().as_str() {
            "list" => {
                let channels = self.store.get_always_respond_channels().await;
                if channels.is_empty() {
                    let _ = msg.channel_id.say(&ctx.http, "常に返信するチャンネルはありません。`!channel toggle` で現在チャンネルを追加できます。").await;
                } else {
                    let ch_list: Vec<String> = channels.iter().map(|c| format!("`{}`", c)).collect();
                    let response = format!("**常に返信するチャンネル** ({}件)\n{}", channels.len(), ch_list.join("\n"));
                    let _ = msg.channel_id.say(&ctx.http, &response).await;
                }
            }
            "toggle" => {
                let ch_id = msg.channel_id.to_string();
                let is_on = self.store.is_channel_always_respond(&ch_id).await;
                if is_on {
                    self.store.remove_always_respond_channel(&ch_id).await;
                    let mut channels = self.always_respond_channels.lock().await;
                    channels.retain(|c| c != &ch_id);
                    let _ = msg.channel_id.say(&ctx.http, format!("チャンネル `{}` を常に返信モードから削除しました。", ch_id)).await;
                } else {
                    self.store.add_always_respond_channel(&ch_id).await;
                    let mut channels = self.always_respond_channels.lock().await;
                    channels.push(ch_id.clone());
                    let _ = msg.channel_id.say(&ctx.http, format!("チャンネル `{}` を常に返信モードに追加しました。このチャンネルではメンションなしで返信します。", ch_id)).await;
                }
            }
            "add" => {
                if args.len() < 3 {
                    let _ = msg.channel_id.say(&ctx.http, "チャンネルIDを入力してください: `!channel add 123456789`").await;
                    return;
                }
                let ch_id = &args[2];
                self.store.add_always_respond_channel(ch_id).await;
                let mut channels = self.always_respond_channels.lock().await;
                channels.push(ch_id.clone());
                let _ = msg.channel_id.say(&ctx.http, format!("チャンネル `{}` を常に返信モードに追加しました。", ch_id)).await;
            }
            "remove" => {
                if args.len() < 3 {
                    let _ = msg.channel_id.say(&ctx.http, "チャンネルIDを入力してください: `!channel remove 123456789`").await;
                    return;
                }
                let ch_id = &args[2];
                self.store.remove_always_respond_channel(ch_id).await;
                let mut channels = self.always_respond_channels.lock().await;
                channels.retain(|c| c != ch_id);
                let _ = msg.channel_id.say(&ctx.http, format!("チャンネル `{}` を常に返信モードから削除しました。", ch_id)).await;
            }
            _ => {
                let _ = msg.channel_id.say(&ctx.http, format!("未知のサブコマンド: `{}`\n`!channel list` で一覧を表示できます。", args[1])).await;
            }
        }
    }
}

// --- Free chat with thinking display + tool use ---

async fn handle_free_chat_thinking(
    ctx: &Context,
    msg: &Message,
    _ai_client: &Arc<AIClient>,
    stream_client: &Arc<StreamClient>,
    context_mgr: &Arc<ContextManager>,
    skills_mgr: &SkillsManager,
    store: &Arc<MemoryStore>,
    pending: &Arc<tokio::sync::Mutex<Vec<String>>>,
) {
    let query = msg.content.trim().to_string();
    if query.is_empty() {
        return;
    }

    info!("[Free Chat] {} in {}: {}", msg.author.name, msg.channel_id, query);

    // Save user message
    context_mgr
        .add_to_context(&msg.author.id.to_string(), &msg.channel_id.to_string(), "user", &query)
        .await;

    // Build messages
    let mut messages: Vec<ChatMessage> = context_mgr
        .get_messages_for_channel(&msg.channel_id.to_string(), 10)
        .await;

    // Tool-use loop
    let mut tool_call_results: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let tool_definitions = skills_mgr.to_tool_definitions();
    let max_rounds = 10;
    let mut round = 0;

    {
        let mut pending_lock = pending.lock().await;
        pending_lock.push(msg.channel_id.to_string());
    }

    let mut final_answer = String::new();
    let mut last_reasoning_len = 0usize;

    // Send thinking indicator
    let _ = msg.channel_id.say(&ctx.http, "🧠 **思考を開始します...**").await;

    loop {
        round += 1;
        if round > max_rounds {
            error!("[Free Chat] Max rounds ({}) reached", max_rounds);
            final_answer = "エラー: 最大ラウンド数を超えました".to_string();
            break;
        }

        info!("[Free Chat] Round {} — sending {} messages", round, messages.len());

        // Stream response with tools
        let items = stream_client.stream_chat_with_tools(&messages, Some(tool_definitions.clone())).await;

        // Collect tool calls
        let mut pending_tool_calls: Vec<(String, String, String)> = Vec::new(); // (id, name, args)

        for item in &items {
            match item {
                StreamItem::Reasoning { content, done } => {
                    if !content.is_empty() && content.len() > last_reasoning_len {
                        let new_text = &content[last_reasoning_len..];
                        if !new_text.trim().is_empty() {
                            let display = format!("🤔 {}", new_text.trim().chars().take(200).collect::<String>());
                            let _ = msg.channel_id.say(&ctx.http, &display).await;
                            last_reasoning_len = content.len();
                        }
                    }
                    if *done && !content.is_empty() {
                        let _ = msg.channel_id.say(&ctx.http, "💭 思考完了").await;
                    }
                }
                StreamItem::ToolCall { id, name, arguments } => {
                    pending_tool_calls.push((id.clone(), name.clone(), arguments.clone()));
                }
                StreamItem::Content(part) => {
                    final_answer = part.clone();
                }
                StreamItem::Error(e) => {
                    error!("[Free Chat] Stream error: {}", e);
                    final_answer = format!("エラー: {}", e);
                }
            }
        }

        // If we got a final answer or error, break
        if !final_answer.is_empty() && !final_answer.starts_with("エラー") {
            break;
        }
        if items.iter().any(|i| matches!(i, StreamItem::Error(_))) {
            break;
        }

        // Execute tool calls
        if pending_tool_calls.is_empty() {
            // No content, no tool calls — something went wrong
            final_answer = "応答がありませんでした。".to_string();
            break;
        }

        for (tc_id, tc_name, tc_args) in &pending_tool_calls {
            info!("[Free Chat] Round {} — Tool Call: {} args={}", round, tc_name, tc_args);

            let display = format!("🔧 **スキル `{}` を実行中...**", tc_name);
            let _ = msg.channel_id.say(&ctx.http, &display).await;

            let output = skills_mgr.invoke(&tc_name, &tc_args, store).await;
            info!("[Free Chat] Round {} — Tool result: {} chars", round, output.len());

            tool_call_results.insert(tc_id.clone(), output.clone());

            let result_msg = format!("✅ **結果:**\n```{}\n```", crate::util::truncate(&output, 1500));
            let _ = msg.channel_id.say(&ctx.http, &result_msg).await;
        }

        // Build messages with tool results for next round
        messages.clear();
        // Restore conversation history
        let history: Vec<ChatMessage> = context_mgr
            .get_messages_for_channel(&msg.channel_id.to_string(), 10)
            .await;
        messages.extend(history);

        // Add tool results as assistant + tool messages
        for (tc_id, tc_name, tc_args) in &pending_tool_calls {
            let output = tool_call_results.get(tc_id).unwrap();
            messages.push(ChatMessage {
                role: "assistant".to_string(),
                content: format!("Tool call: {} with args: {}", tc_name, tc_args),
                tool_calls: None,
                tool_call_id: None,
            });
            messages.push(ChatMessage {
                role: "tool".to_string(),
                content: output.clone(),
                tool_calls: None,
                tool_call_id: Some(tc_id.clone()),
            });
        }
    }

    let reply_text = crate::util::truncate(&final_answer, 1800);

    // Save AI response
    context_mgr
        .add_to_context(&msg.author.id.to_string(), &msg.channel_id.to_string(), "assistant", &reply_text)
        .await;

    let _ = msg.channel_id.say(&ctx.http, &reply_text).await;

    {
        let mut pending_lock = pending.lock().await;
        pending_lock.retain(|c| c != &msg.channel_id.to_string());
    }
}

// --- Thinking chat (for !think command) ---

async fn handle_thinking_chat(
    ctx: &Context,
    msg: &Message,
    query: &str,
    stream_client: &Arc<StreamClient>,
    context_mgr: &Arc<ContextManager>,
) {
    info!("[Think] {} in {}: {}", msg.author.name, msg.channel_id, query);

    context_mgr
        .add_to_context(&msg.author.id.to_string(), &msg.channel_id.to_string(), "user", query)
        .await;

    let messages = context_mgr
        .get_messages_for_channel(&msg.channel_id.to_string(), 10)
        .await;

    let _ = msg.channel_id.say(&ctx.http, "🧠 **思考を開始します...**").await;

    let mut content_parts = Vec::new();
    use crate::ai::stream::StreamItem;

    let items = stream_client.stream_chat(&messages).await;
    for item in items {
        match item {
            StreamItem::Reasoning { content, done } => {
                if !content.is_empty() {
                    let display = format!("💭 {}", content.chars().take(200).collect::<String>());
                    let _ = msg.channel_id.say(&ctx.http, &display).await;
                }
                if done {
                    let _ = msg.channel_id.say(&ctx.http, "思考完了").await;
                }
            }
            StreamItem::Content(part) => {
                content_parts.push(part);
            }
            StreamItem::ToolCall { .. } => {}
            StreamItem::Error(e) => {
                error!("[Think] Stream error: {}", e);
                let _ = msg.channel_id.say(&ctx.http, &format!("エラー: {}", e)).await;
            }
        }
    }

    let answer = content_parts.join("");
    let final_answer = crate::util::truncate(&answer, 1800);

    context_mgr
        .add_to_context(&msg.author.id.to_string(), &msg.channel_id.to_string(), "assistant", &final_answer)
        .await;

    let _ = msg.channel_id.say(&ctx.http, &final_answer).await;
}

pub fn get_intents() -> GatewayIntents {
    GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT
}
