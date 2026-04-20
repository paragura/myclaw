use axum::{Router, routing::{get, post, delete, get_service}, http::StatusCode, response::Response, body::Body, middleware::{Next, from_fn_with_state}, extract::{Request, State}};
use tower_http::services::ServeDir;
use std::sync::Arc;
use tokio::net::TcpListener;
use std::time::Instant;

use crate::memory::store::MemoryStore;
use crate::skills::manager::SkillsManager;
use crate::agent::manager::AgentManager;

#[derive(Clone)]
struct AuthState {
    user: String,
    pass: String,
}

pub struct WebServer {
    listen: String,
    auth_user: Option<String>,
    auth_pass: Option<String>,
}

impl WebServer {
    pub fn new(listen: &str, auth_user: Option<String>, auth_pass: Option<String>) -> Self {
        Self {
            listen: listen.to_string(),
            auth_user,
            auth_pass,
        }
    }

    fn base64_decode(input: &str) -> Result<String, ()> {
        let decode_char = |c: u8| -> Option<u32> {
            match c {
                b'A'..=b'Z' => Some((c - b'A') as u32),
                b'a'..=b'z' => Some((c - b'a' + 26) as u32),
                b'0'..=b'9' => Some((c - b'0' + 52) as u32),
                b'+' => Some(62),
                b'/' => Some(63),
                _ => None,
            }
        };

        let bytes: Vec<u8> = input.bytes().filter(|c| !c.is_ascii_whitespace()).collect();
        if bytes.is_empty() || bytes.len() % 4 != 0 {
            return Err(());
        }

        let mut out = Vec::new();
        for chunk in bytes.chunks(4) {
            let a = decode_char(chunk[0]).ok_or(())?;
            let b = if chunk.len() > 1 && chunk[1] != b'=' { decode_char(chunk[1]).ok_or(())? } else { 0 };
            let c = if chunk.len() > 2 && chunk[2] != b'=' { decode_char(chunk[2]).ok_or(())? } else { 0 };
            let d = if chunk.len() > 3 && chunk[3] != b'=' { decode_char(chunk[3]).ok_or(())? } else { 0 };

            out.push(((a << 2) | (b >> 4)) as u8);
            if chunk.len() > 2 && chunk[2] != b'=' {
                out.push((((b & 0xf) << 4) | (c >> 2)) as u8);
            }
            if chunk.len() > 3 && chunk[3] != b'=' {
                out.push((((c & 0x3) << 6) | d) as u8);
            }
        }
        String::from_utf8(out).map_err(|_| ())
    }

    pub async fn serve(
        self,
        store: Arc<MemoryStore>,
        skills_mgr: Arc<SkillsManager>,
        agent_mgr: Arc<AgentManager>,
        bot_name: String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let start_time = Instant::now();
        let shared = crate::web::routes::AppState {
            store,
            skills_mgr,
            agent_mgr,
            bot_name,
            start_time,
        };

        let app = Router::new()
            .route("/", get(crate::web::routes::dashboard))
            .route("/api/status", get(crate::web::routes::api_status))
            .route("/api/memories", get(crate::web::routes::api_memories))
            .route("/api/memories/:id", delete(crate::web::routes::api_delete_memory))
            .route("/api/agents", get(crate::web::routes::api_agents))
            .route("/api/agents/:id/stop", post(crate::web::routes::api_stop_agent))
            .route("/api/skills", get(crate::web::routes::api_skills))
            .route("/api/skills/:name/execute", post(crate::web::routes::api_execute_skill))
            .route("/api/channels", get(crate::web::routes::api_channels))
            .route("/api/channels/toggle", post(crate::web::routes::api_toggle_channel))
            .route("/api/channels/add", post(crate::web::routes::api_add_channel))
            .route("/api/channels/remove", post(crate::web::routes::api_remove_channel))
            .route("/api/tasks", get(crate::web::routes::api_tasks))
            .nest_service("/static", get_service(ServeDir::new("src/web/static_")))
            .with_state(shared);

        let app = if let (Some(user), Some(pass)) = (&self.auth_user, &self.auth_pass) {
            let auth_state = AuthState {
                user: user.clone(),
                pass: pass.clone(),
            };
            app.layer(from_fn_with_state(auth_state, Self::auth_middleware))
        } else {
            app
        };

        let addr: std::net::SocketAddr = self.listen.parse()?;
        tracing::info!("[Web] Starting dashboard on http://{}", self.listen);

        let listener = TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;

        Ok(())
    }

    async fn auth_middleware(
        State(auth): State<AuthState>,
        req: Request,
        next: Next,
    ) -> Response {
        let auth_header = req.headers().get(axum::http::header::AUTHORIZATION);
        match auth_header {
            Some(header) => {
                if let Ok(header_str) = std::str::from_utf8(header.as_bytes()) {
                    if header_str.starts_with("Basic ") {
                        let encoded = &header_str[6..];
                        if let Ok(decoded) = Self::base64_decode(encoded) {
                            let parts: Vec<&str> = decoded.splitn(2, ':').collect();
                            if parts.len() == 2 && parts[0] == auth.user && parts[1] == auth.pass {
                                return next.run(req).await;
                            }
                        }
                    }
                }
            }
            None => {}
        }
        let mut resp = Response::new(Body::from("Unauthorized"));
        *resp.status_mut() = StatusCode::UNAUTHORIZED;
        resp
    }
}
