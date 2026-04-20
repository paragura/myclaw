use serenity::prelude::*;
use serenity::model::channel::Message;
use tracing::error;
use sqlx::Row;
use crate::ai::client::ChatMessage;
use crate::ai::stream::StreamClient;
use crate::ai::context::ContextManager;
use crate::memory::store::MemoryStore;
use crate::coding::executor::CodingExecutor;
use crate::skills::manager::SkillsManager;
use crate::agent::manager::AgentManager;
use std::sync::Arc;

pub async fn handle_command(
    ctx: &Context,
    msg: &Message,
    args: &[String],
    ai_client: &Arc<crate::ai::client::AIClient>,
    _stream_client: &Arc<StreamClient>,
    context_mgr: &Arc<ContextManager>,
    store: &Arc<MemoryStore>,
    _coding_exec: &Arc<CodingExecutor>,
    skills_mgr: &Arc<SkillsManager>,
    _agent_mgr: &Arc<AgentManager>,
) {
    if args.is_empty() {
        return;
    }

    let command = args[0].to_lowercase();

    match command.as_str() {
        "help" => {
            let help_text = "\
**myclaw コマンド一覧:**\n\
`!help` - このヘルプを表示\n\
`!chat <メッセージ>` - AIとチャット\n\
`!think <メッセージ>` - 思考過程を表示して回答\n\
`!learn <カテゴリ> <内容>` - 私を学習させる\n\
`!memories [カテゴリ]` - メモリ一覧\n\
`!search <キーワード>` - メモリを検索\n\
`!skill <スキル名> <引数>` - スキルを実行\n\
`!skills` - 利用可能なスキル一覧\n\
`!channel list` - 常に返信するチャンネル一覧\n\
`!channel toggle` - 現在のチャンネルを切り替え\n\
`!code <説明>` - コーディングタスク\n\
`!code status` - コーディングタスクのステータス確認\n\
`!agent <名前> <タスク>` - エージェントを起動\n\
`!agent list` - エージェント一覧\n\
`!agent stop <ID>` - エージェントを停止\n\
`!schedule <名前> <cron>` - 定期タスクを追加\n\
`!schedule list` - 定期タスク一覧\n\
`!forget <メモリID>` - メモリを削除\n\
`!status` - ボットのステータス表示\n\
`!ping` - Pong!
            ";
            let _ = msg.channel_id.say(&ctx.http, help_text).await;
        }

        "ping" => {
            let _ = msg.channel_id.say(&ctx.http, "Pong! 🐾").await;
        }

        "chat" => {
            let query = args[1..].join(" ");
            if query.is_empty() {
                let _ = msg.channel_id.say(&ctx.http, "チャットのメッセージを入力してください: `!chat こんにちは`").await;
                return;
            }

            context_mgr
                .add_to_context(&msg.author.id.to_string(), &msg.channel_id.to_string(), "user", &query)
                .await;

            let messages = context_mgr
                .get_messages_for_channel(&msg.channel_id.to_string(), 10)
                .await;

            let response = match ai_client.chat(&messages).await {
                Ok(resp) => crate::util::truncate(&resp, 1800),
                Err(e) => {
                    let err_msg = e.to_string();
                    error!("AI chat error: {}", err_msg);
                    "AI応答中にエラーが発生しました。".to_string()
                }
            };

            context_mgr
                .add_to_context(&msg.author.id.to_string(), &msg.channel_id.to_string(), "assistant", &response)
                .await;

            let _ = msg.channel_id.say(&ctx.http, &response).await;
        }

        "learn" => {
            if args.len() < 3 {
                let _ = msg
                    .channel_id
                    .say(&ctx.http, "使い方を教えてみます: `!learn 趣味 パスタを作ることが好き`")
                    .await;
                return;
            }

            let category = args[1].clone();
            let content = args[2..].join(" ");

            let memory = store.add_memory(&category, &content, 0.8).await;

            let _ = msg
                .channel_id
                .say(&ctx.http, format!("学習しました！🧠\nカテゴリ: {}\n内容: {}\nID: {}", category, content, memory.id))
                .await;
        }

        "memories" => {
            let category = args.get(1).map(|s| s.as_str());
            let memories = store.get_memories(category, 20).await;

            if memories.is_empty() {
                let _ = msg.channel_id.say(&ctx.http, "メモリはありません。").await;
            } else {
                let memory_list: Vec<String> = memories
                    .iter()
                    .map(|m| format!("`[{}]` {}: {}", m.id.chars().take(8).collect::<String>(), m.category, m.content))
                    .collect();

                let response = format!("**メモリ一覧** ({}件):\n{}", memories.len(), memory_list.join("\n"));
                let _ = msg.channel_id.say(&ctx.http, &response).await;
            }
        }

        "search" => {
            if args.len() < 2 {
                let _ = msg
                    .channel_id
                    .say(&ctx.http, "検索キーワードを入力してください: `!search 趣味`")
                    .await;
                return;
            }

            let query = args[1..].join(" ");
            let memories = store.search_memories(&query, 10).await;

            if memories.is_empty() {
                let _ = msg.channel_id.say(&ctx.http, "該当するメモリはありません。").await;
            } else {
                let memory_list: Vec<String> = memories
                    .iter()
                    .map(|m| format!("`[{}]` {}: {}", m.id.chars().take(8).collect::<String>(), m.category, m.content))
                    .collect();

                let response = format!("**検索結果** ({}件):\n{}", memories.len(), memory_list.join("\n"));
                let _ = msg.channel_id.say(&ctx.http, &response).await;
            }
        }

        "forget" => {
            if args.len() < 2 {
                let _ = msg
                    .channel_id
                    .say(&ctx.http, "削除するメモリのIDを入力してください: `!forget abc123`")
                    .await;
                return;
            }

            let id = &args[1];
            let deleted = store.delete_memory(id).await;

            if deleted {
                let _ = msg.channel_id.say(&ctx.http, format!("メモリ `{}` を削除しました。🗑️", id)).await;
            } else {
                let _ = msg.channel_id.say(&ctx.http, format!("メモリ `{}` は見つかりませんでした。", id)).await;
            }
        }

        "code" => {
            if args.len() < 2 {
                let _ = msg
                    .channel_id
                    .say(&ctx.http, "コーディングタスクの説明を入力してください: `!code RustのHTTPサーバーを書いて`")
                    .await;
                return;
            }

            if args[1].to_lowercase() == "status" {
                let rows = sqlx::query(
                    "SELECT id, description, status, created_at FROM coding_tasks ORDER BY created_at DESC LIMIT 10"
                )
                .fetch_all(store.pool())
                .await
                .unwrap_or_default();

                if rows.is_empty() {
                    let _ = msg.channel_id.say(&ctx.http, "実行中のコーディングタスクはありません。").await;
                } else {
                    let task_list: Vec<String> = rows
                        .iter()
                        .map(|r| {
                            let id: &str = r.get("id");
                            let description: &str = r.get("description");
                            let status: &str = r.get("status");
                            format!("`{}`: {} [{}]",
                                id.chars().take(8).collect::<String>(),
                                description,
                                status
                            )
                        }).collect();
                    let response = format!("**コーディングタスク一覧** ({}件):\n{}", rows.len(), task_list.join("\n"));
                    let _ = msg.channel_id.say(&ctx.http, &response).await;
                }
                return;
            }

            let description = args[1..].join(" ");

            let _ = msg
                .channel_id
                .say(&ctx.http, format!("コーディングタスクを開始します: `{}`\n少しお待ちください...", description))
                .await;

            let description_clone = description.clone();
            let channel_id = msg.channel_id;
            let author_id = msg.author.id;
            let http = ctx.http.clone();
            let ai_client_clone = ai_client.clone();
            let store_clone = store.clone();

            tokio::spawn(async move {
                let (task_id, _) = store_clone
                    .add_coding_task(&author_id.to_string(), &channel_id.to_string(), &description_clone)
                    .await;

                let system_prompt = "あなたはRustのコーディングアシスタントです。正確で効率的なコードを生成してください。";
                let messages = vec![
                    ChatMessage {
                        role: "system".to_string(),
                        content: system_prompt.to_string(),
                    },
                    ChatMessage {
                        role: "user".to_string(),
                        content: format!("以下の要件でコードを生成してください:\n\n{}", description_clone),
                    },
                ];

                let code_str = match ai_client_clone.chat(&messages).await {
                    Ok(code) => {
                        if code.len() > 1500 {
                            format!("```rust\n{}\n...\n```", code.chars().take(1200).collect::<String>())
                        } else {
                            code
                        }
                    }
                    Err(e) => {
                        let err_msg = e.to_string();
                        error!("Code generation failed: {}", err_msg);
                        "エラーが発生しました".to_string()
                    }
                };

                let status = if code_str.contains("エラー") { "failed" } else { "completed" };

                store_clone
                    .update_coding_task_result(&task_id, Some(&code_str), status, None)
                    .await;

                let response_text = if status == "completed" {
                    format!("**コーディング完了！**\n\n{}", code_str)
                } else {
                    "コード生成中にエラーが発生しました。".to_string()
                };

                let _ = channel_id
                    .say(&http, &response_text)
                    .await;
            });
        }

        "schedule" => {
            if args.len() < 2 {
                let _ = msg
                    .channel_id
                    .say(&ctx.http, "使い方:\n`!schedule list` - タスク一覧\n`!schedule <名前> <cron式>` - 追加\n`!schedule stop <名前>` - 停止")
                    .await;
                return;
            }

            match args[1].to_lowercase().as_str() {
                "list" => {
                    let tasks = store.get_all_tasks().await;
                    if tasks.is_empty() {
                        let _ = msg.channel_id.say(&ctx.http, "登録された定期タスクはありません。").await;
                    } else {
                        let task_list: Vec<String> = tasks
                            .iter()
                            .map(|t| format!("`{}` - {} (cron: {}) [{}]", t.name, t.next_run.as_deref().unwrap_or("N/A"), t.cron_expression, if t.enabled { "active" } else { "stopped" }))
                            .collect();
                        let response = format!("**定期タスク一覧** ({}件):\n{}", tasks.len(), task_list.join("\n"));
                        let _ = msg.channel_id.say(&ctx.http, &response).await;
                    }
                }
                "stop" => {
                    if args.len() < 3 {
                        let _ = msg.channel_id.say(&ctx.http, "停止するタスクの名前を入力してください。").await;
                        return;
                    }
                    let task_name = &args[2];
                    let rows = sqlx::query("UPDATE scheduled_tasks SET enabled = 0 WHERE name = ?")
                        .bind(task_name)
                        .execute(store.pool())
                        .await
                        .unwrap_or_default();
                    if rows.rows_affected() > 0 {
                        let _ = msg.channel_id.say(&ctx.http, format!("タスク `{}` を停止しました。", task_name)).await;
                    } else {
                        let _ = msg.channel_id.say(&ctx.http, format!("タスク `{}` は見つかりませんでした。", task_name)).await;
                    }
                }
                _ => {
                    if args.len() < 3 {
                        let _ = msg.channel_id.say(&ctx.http, "cron式を入力してください: `!schedule 毎時報告 0 * * * *`").await;
                        return;
                    }

                    let name = args[1].clone();
                    let cron_expr = args[2..].join(" ");

                    let task = store.add_scheduled_task(&name, &cron_expr).await;
                    let _ = msg
                        .channel_id
                        .say(&ctx.http, format!("定期タスクを追加しました: `{}` (cron: `{}`)", task.name, task.cron_expression))
                        .await;
                }
            }
        }

        "status" => {
            let memories_count = store.get_memories(None, 1).await.len();
            let _ = msg
                .channel_id
                .say(&ctx.http, format!(
                    "**myclaw Status**\n\
                    モデル: Qwen3.6-35B-A3B-FP8\n\
                    メモリ: {}件\n\
                    プレフィックス: ``\n\
                    スキル: {}個\n\
                    エージェント: 最大3体\n\
                    \n\
                    Discord専用AIアシスタント🐾"
                    , memories_count
                    , skills_mgr.list().len()
                ))
                .await;
        }

        _ => {
            let _ = msg
                .channel_id
                .say(&ctx.http, format!("未知のコマンド: `{}`\n`!help` でコマンド一覧を表示できます。", command))
                .await;
        }
    }
}
