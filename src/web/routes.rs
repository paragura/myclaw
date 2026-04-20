use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, Json},
    Json as JsonBody,
};
use serde::Serialize;
use std::sync::Arc;
use std::time::Instant;

use crate::memory::store::MemoryStore;
use crate::skills::manager::SkillsManager;
use crate::agent::manager::AgentManager;

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<MemoryStore>,
    pub skills_mgr: Arc<SkillsManager>,
    pub agent_mgr: Arc<AgentManager>,
    pub bot_name: String,
    pub start_time: Instant,
}

// --- Dashboard page ---

pub async fn dashboard() -> Html<&'static str> {
    Html(include_str!("static_/index.html"))
}

// --- API: Status ---

#[derive(Serialize)]
pub struct StatusResponse {
    pub bot_name: String,
    pub memories: usize,
    pub active_agents: usize,
    pub skills: usize,
    pub uptime_text: String,
}

pub async fn api_status(
    State(state): State<AppState>,
) -> Json<StatusResponse> {
    let memories = state.store.get_memories(None, 1).await;
    let active_agents = state.agent_mgr.active_count().await;
    let skills = state.skills_mgr.list().len();
    let elapsed = state.start_time.elapsed();
    let hours = elapsed.as_secs() / 3600;
    let mins = (elapsed.as_secs() % 3600) / 60;
    let uptime_text = if hours > 0 {
        format!("{}h {}m", hours, mins)
    } else {
        format!("{}m", mins)
    };

    Json(StatusResponse {
        bot_name: state.bot_name,
        memories: memories.len(),
        active_agents,
        skills,
        uptime_text,
    })
}

// --- API: Memories ---

#[derive(Serialize)]
pub struct MemoryItem {
    pub id: String,
    pub category: String,
    pub content: String,
    pub importance: f32,
    pub created_at: String,
}

pub async fn api_memories(
    State(state): State<AppState>,
) -> Json<Vec<MemoryItem>> {
    let memories = state.store.get_memories(None, 100).await;
    Json(memories.into_iter().map(|m| MemoryItem {
        id: m.id,
        category: m.category,
        content: m.content,
        importance: m.importance,
        created_at: m.created_at,
    }).collect())
}

pub async fn api_delete_memory(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<String>, (StatusCode, Json<String>)> {
    let deleted = state.store.delete_memory(&id).await;
    if deleted {
        Ok(Json("deleted".to_string()))
    } else {
        Err((StatusCode::NOT_FOUND, Json("not found".to_string())))
    }
}

// --- API: Agents ---

#[derive(Serialize)]
pub struct AgentItem {
    pub id: String,
    pub name: String,
    pub status: String,
}

pub async fn api_agents(
    State(state): State<AppState>,
) -> Json<Vec<AgentItem>> {
    let agents = state.agent_mgr.list_agents().await;
    Json(agents.into_iter().map(|a| AgentItem {
        id: a.id.chars().take(8).collect(),
        name: a.name,
        status: a.status.to_string(),
    }).collect())
}

pub async fn api_stop_agent(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<String>, (StatusCode, Json<String>)> {
    let stopped = state.agent_mgr.interrupt_agent(&id).await;
    if stopped {
        Ok(Json("stopped".to_string()))
    } else {
        Err((StatusCode::NOT_FOUND, Json("not found".to_string())))
    }
}

// --- API: Skills ---

#[derive(Serialize)]
pub struct SkillItem {
    pub name: String,
    pub description: String,
}

pub async fn api_skills(
    State(state): State<AppState>,
) -> Json<Vec<SkillItem>> {
    let mut items: Vec<SkillItem> = state.skills_mgr.list()
        .iter()
        .map(|s| SkillItem {
            name: s.name.clone(),
            description: s.description.clone(),
        })
        .collect();

    // Add markdown skills
    let md_skills = crate::skills::manager::SkillsManager::load_markdown_skills();
    for s in md_skills {
        items.push(SkillItem {
            name: s.name.replace('-', "_"),
            description: s.description,
        });
    }

    Json(items)
}

pub async fn api_execute_skill(
    State(state): State<AppState>,
    Path(name): Path<String>,
    JsonBody(args): JsonBody<String>,
) -> Json<String> {
    let output = state.skills_mgr.invoke(&name, &args, &state.store).await;
    Json(output)
}

// --- API: Channels ---

#[derive(Serialize)]
pub struct ChannelItem {
    pub channel_id: String,
    pub always_respond: bool,
}

pub async fn api_channels(
    State(state): State<AppState>,
) -> Json<Vec<ChannelItem>> {
    let configs = state.store.list_channel_configs().await;
    Json(configs.into_iter().map(|(id, _, val)| ChannelItem {
        channel_id: id,
        always_respond: val,
    }).collect())
}

pub async fn api_toggle_channel(
    State(state): State<AppState>,
    JsonBody(body): JsonBody<serde_json::Value>,
) -> Result<Json<String>, (StatusCode, Json<String>)> {
    let channel_id = body.get("channel_id").and_then(|v| v.as_str()).ok_or_else(|| {
        (StatusCode::BAD_REQUEST, Json("missing channel_id".to_string()))
    })?;
    let channel_id = channel_id.to_string();

    let is_on = state.store.is_channel_always_respond(&channel_id).await;
    if is_on {
        state.store.remove_always_respond_channel(&channel_id).await;
        Ok(Json("removed".to_string()))
    } else {
        state.store.add_always_respond_channel(&channel_id).await;
        Ok(Json("added".to_string()))
    }
}

pub async fn api_add_channel(
    State(state): State<AppState>,
    JsonBody(body): JsonBody<serde_json::Value>,
) -> Result<Json<String>, (StatusCode, Json<String>)> {
    let channel_id = body.get("channel_id").and_then(|v| v.as_str()).ok_or_else(|| {
        (StatusCode::BAD_REQUEST, Json("missing channel_id".to_string()))
    })?;
    let channel_id = channel_id.to_string();
    state.store.add_always_respond_channel(&channel_id).await;
    Ok(Json("added".to_string()))
}

pub async fn api_remove_channel(
    State(state): State<AppState>,
    JsonBody(body): JsonBody<serde_json::Value>,
) -> Result<Json<String>, (StatusCode, Json<String>)> {
    let channel_id = body.get("channel_id").and_then(|v| v.as_str()).ok_or_else(|| {
        (StatusCode::BAD_REQUEST, Json("missing channel_id".to_string()))
    })?;
    let channel_id = channel_id.to_string();
    state.store.remove_always_respond_channel(&channel_id).await;
    Ok(Json("removed".to_string()))
}

// --- API: Tasks ---

#[derive(Serialize)]
pub struct TaskItem {
    pub id: String,
    pub name: String,
    pub cron_expression: String,
    pub last_run: Option<String>,
    pub next_run: Option<String>,
    pub enabled: bool,
}

pub async fn api_tasks(
    State(state): State<AppState>,
) -> Json<Vec<TaskItem>> {
    let tasks = state.store.get_all_tasks().await;
    Json(tasks.into_iter().map(|t| TaskItem {
        id: t.id,
        name: t.name,
        cron_expression: t.cron_expression,
        last_run: t.last_run,
        next_run: t.next_run,
        enabled: t.enabled,
    }).collect())
}
