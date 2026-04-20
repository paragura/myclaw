mod ai;
mod agent;
mod coding;
mod config;
mod discord;
mod memory;
mod skills;
mod tasks;
mod tools;
mod util;
mod web;

use config::Config;
use memory::db::init_db;
use memory::store::MemoryStore;
use ai::client::AIClient;
use ai::stream::StreamClient;
use ai::context::ContextManager;
use coding::executor::CodingExecutor;
use skills::manager::SkillsManager;
use agent::manager::AgentManager;
use tasks::scheduler::TaskScheduler;

use serenity::Client;
use tracing::{info, error};
use tracing_subscriber::EnvFilter;
use std::sync::Arc;
use std::env;

#[tokio::main]
async fn main() {
    // Initialize logging
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new("info,serenity=warn,sqlx=warn")
    });
    tracing_subscriber::fmt().with_env_filter(env_filter).init();

    info!("=== myclaw starting ===");
    info!("Model: Qwen3.6-35B-A3B-FP8");
    info!("Features: スキルシステム, 思考表示, エージェント");

    // Load config
    let config_path = env::var("OPENCLAW_CONFIG")
        .unwrap_or_else(|_| "config.toml".to_string());

    let config = match Config::from_file(&config_path) {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to load config from '{}': {}", config_path, e);
            eprintln!("config.toml が見つかりません。config.toml.example をコピーして設定してください:");
            eprintln!("  cp config.toml.example config.toml");
            std::process::exit(1);
        }
    };

    info!("Config loaded: db={}", config.db.path);

    // Initialize SQLite database
    let pool = match sqlx::SqlitePool::connect(&format!("sqlite://{}", config.db.path)).await {
        Ok(p) => p,
        Err(e) => {
            error!("Failed to connect to database: {}", e);
            std::process::exit(1);
        }
    };

    init_db(&pool).await.expect("Failed to initialize database");
    info!("Database initialized");

    // Create memory store
    let store = Arc::new(MemoryStore::new(pool.clone()));

    // Seed channel config from config.toml into DB
    if let Some(ref channels) = config.channels.always_respond_channels {
        for ch in channels {
            store.add_always_respond_channel(ch).await;
        }
    }

    // Initialize AI clients (both streaming and non-streaming)
    let ai_client = Arc::new(AIClient::new(
        &config.ai.api_url,
        &config.ai.model,
        &config.ai.api_key,
        config.ai.max_tokens,
        config.ai.temperature,
    ));

    let stream_client = Arc::new(StreamClient::new(
        &config.ai.api_url,
        &config.ai.model,
        &config.ai.api_key,
        config.ai.max_tokens,
        config.ai.temperature,
    ));

    // Create context manager
    let system_prompt = r#"あなたはmyclaw。ユーザーだけのパーソナルAIアシスタントです。

- 日本語で会話する
- ユーザーのことをよく理解しようとする
- コーディングが得意
- 学んだことはメモリに保存して、次の会話で活かす
- 短く簡潔に答える
- スキルが使える場合はそれを使ってください

## 安全ルール
- 削除（rm, 削除を含むファイル編集）、移動（mv）、名前変更（rename）などの破壊的変更を行う場合は、必ず実行前にユーザーに確認を取る
- 確認なしにファイルを削除したり、ディレクトリを移動したりしない
- 上書き編集する場合は、変更内容を確認してから実行する
- 危険なコマンド（rm -rf, chmod 777, dd 等）は絶対に実行しない"#.to_string();

    let context_mgr = Arc::new(ContextManager::new(store.clone(), system_prompt));

    // Create coding executor
    let coding_exec = Arc::new(CodingExecutor::new(ai_client.clone(), store.clone()));

    // Create skills manager
    let skills_mgr = Arc::new(SkillsManager::new());

    // Create agent manager
    let agent_mgr = Arc::new(AgentManager::new(3));

    // Create Discord handler
    let always_respond = config.channels.always_respond_channels.clone().unwrap_or_default();
    let handler = discord::handler::BotHandler::new(
        ai_client.clone(),
        stream_client.clone(),
        context_mgr.clone(),
        store.clone(),
        coding_exec.clone(),
        skills_mgr.clone(),
        agent_mgr.clone(),
        config.bot.prefix.clone(),
        always_respond.clone(),
    );

    // Seed channel config from config into DB
    handler.seed_channel_config().await;

    // Initialize task scheduler
    let scheduler = Arc::new(TaskScheduler::new(store.clone()));

    // Register built-in tasks
    let store_hb = store.clone();
    scheduler.register(
        "heartbeat".to_string(),
        "*/5 * * * *".to_string(),
        Arc::new(move || {
            let store = store_hb.clone();
            Box::pin(async move {
                let _ = store.get_memories(None, 1).await;
                info!("[Task] Heartbeat OK");
            })
        }),
    );

    let store_mc = store.clone();
    scheduler.register(
        "memory_cleanup".to_string(),
        "0 0 * * *".to_string(),
        Arc::new(move || {
            let store = store_mc.clone();
            Box::pin(async move {
                info!("[Task] Memory cleanup running");
                let _ = store.get_memories(None, 100).await;
            })
        }),
    );

    let store_sl = store.clone();
    scheduler.register(
        "self_learning".to_string(),
        "0 */6 * * *".to_string(),
        Arc::new(move || {
            let store = store_sl.clone();
            Box::pin(async move {
                info!("[Task] Self-learning running");
                let _ = store.get_memories(None, 50).await;
            })
        }),
    );

    // Start web server in background
    let web_server = config.web.clone();
    let store_web = store.clone();
    let skills_web = skills_mgr.clone();
    let agent_web = agent_mgr.clone();
    let auth_user = web_server.auth_user.clone();
    let auth_pass = web_server.auth_pass.clone();
    tokio::spawn(async move {
        let server = web::server::WebServer::new(&web_server.listen, auth_user, auth_pass);
        if let Err(e) = server.serve(store_web, skills_web, agent_web, "myclaw".to_string()).await {
            error!("[Web] Server error: {}", e);
        }
    });

    // Build Discord client
    let mut client = Client::builder(&config.bot.token, discord::handler::get_intents())
        .event_handler(handler)
        .await
        .expect("Failed to create Discord client");

    let current_user = client.http.get_current_user().await.expect("Failed to get current user");
    info!("Bot connected: {}", current_user.name);
    info!("Ready! Prefix: `!`");
    info!("Commands: `!help` でコマンド一覧を表示");
    info!("Skills: `!skills` で利用可能なスキルを確認");

    if let Err(why) = client.start().await {
        error!("Discord client error: {:?}", why);
    }
}
