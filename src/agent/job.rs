/// AgentJob — atomic state transitions and cooperative cancellation.
///
/// Based on Codex's AgentJob model: explicit state machine with
/// `is_final()`, cooperative cancellation polling, and structured
/// retry tracking (`attempt_count`, `last_error`).
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tracing::{debug, error, info};
use uuid::Uuid;

/// Job status with explicit state transitions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum JobStatus {
    Pending,
    Running,
    Cancelled,
    Completed,
    Failed,
}

impl std::fmt::Display for JobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JobStatus::Pending => write!(f, "保留中"),
            JobStatus::Running => write!(f, "実行中"),
            JobStatus::Cancelled => write!(f, "キャンセル済み"),
            JobStatus::Completed => write!(f, "完了"),
            JobStatus::Failed => write!(f, "失敗"),
        }
    }
}

impl JobStatus {
    /// Returns true if this status is terminal (no further transitions).
    pub fn is_final(&self) -> bool {
        matches!(self, JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled)
    }

    /// Attempt a transition. Returns true if the transition succeeded.
    /// Valid transitions:
    ///   Pending → Running | Cancelled
    ///   Running → Completed | Failed | Cancelled
    pub fn can_transition_to(&self, target: &JobStatus) -> bool {
        match (self, target) {
            (JobStatus::Pending, JobStatus::Running | JobStatus::Cancelled) => true,
            (JobStatus::Running, JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled) => true,
            (_, _) => false,
        }
    }
}

/// A job tracked in the agent lifecycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentJob {
    pub id: String,
    pub name: String,
    pub status: JobStatus,
    pub attempt_count: u32,
    pub last_error: Option<String>,
    pub result: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl AgentJob {
    /// Create a new pending job.
    pub fn new(id: &str, name: &str) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            id: id.to_string(),
            name: name.to_string(),
            status: JobStatus::Pending,
            attempt_count: 0,
            last_error: None,
            result: None,
            created_at: now.clone(),
            updated_at: now,
        }
    }

    /// Serialize a JobStatus to its English DB representation.
    fn status_db_str(s: &JobStatus) -> String {
        match s {
            JobStatus::Pending => "Pending".to_string(),
            JobStatus::Running => "Running".to_string(),
            JobStatus::Cancelled => "Cancelled".to_string(),
            JobStatus::Completed => "Completed".to_string(),
            JobStatus::Failed => "Failed".to_string(),
        }
    }

    /// Atomically transition to a new status via DB update.
    /// Returns true if the transition succeeded (optimistic lock).
    pub async fn transition(&self, pool: &SqlitePool, new_status: JobStatus, result: Option<&str>, error: Option<&str>) -> bool {
        if !self.status.can_transition_to(&new_status) {
            debug!(
                "[AgentJob] Invalid transition: {:?} -> {:?} for job {}",
                self.status, new_status, self.id
            );
            return false;
        }

        let now = chrono::Utc::now().to_rfc3339();
        let attempt_count = if new_status == JobStatus::Pending {
            self.attempt_count
        } else if new_status == JobStatus::Running {
            self.attempt_count + 1
        } else {
            self.attempt_count
        };

        let new_status_str = Self::status_db_str(&new_status);
        let cur_status_str = Self::status_db_str(&self.status);

        let row_count = sqlx::query(
            "UPDATE agent_jobs SET status = ?, attempt_count = ?, last_error = ?, result = ?, updated_at = ? WHERE id = ? AND status = ?",
        )
        .bind(&new_status_str)
        .bind(attempt_count as i64)
        .bind(error.map(|s| s.to_string()))
        .bind(result.map(|s| s.to_string()))
        .bind(&now)
        .bind(&self.id)
        .bind(&cur_status_str)
        .execute(pool)
        .await;

        match row_count {
            Ok(r) if r.rows_affected() > 0 => {
                info!(
                    "[AgentJob] {} -> {} for job {} (attempt {})",
                    self.status, new_status, self.id, attempt_count
                );
                true
            }
            Ok(_) => {
                // Race: another process already changed the status
                debug!(
                    "[AgentJob] Race on transition {} -> {} for job {}",
                    self.status, new_status, self.id
                );
                false
            }
            Err(e) => {
                error!(
                    "[AgentJob] DB error on transition: {} -> {} for job {}: {}",
                    self.status, new_status, self.id, e
                );
                false
            }
        }
    }

    /// Check if the job has been cancelled (cooperative cancellation).
    pub async fn is_cancelled(pool: &SqlitePool, job_id: &str) -> bool {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT status FROM agent_jobs WHERE id = ?",
        )
        .bind(job_id)
        .fetch_optional(pool)
        .await
        .unwrap_or_default();

        row.map(|(s,)| s == "Cancelled").unwrap_or(false)
    }
}

/// Initialize the agent_jobs table.
pub async fn init_agent_jobs_table(pool: &SqlitePool) {
    let _ = sqlx::query(
        "CREATE TABLE IF NOT EXISTS agent_jobs (
            id         TEXT PRIMARY KEY,
            name       TEXT NOT NULL,
            status     TEXT NOT NULL DEFAULT 'Pending',
            attempt_count INTEGER NOT NULL DEFAULT 0,
            last_error TEXT,
            result     TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await;
}

/// Insert a new job into the DB.
pub async fn insert_job(pool: &SqlitePool, job: &AgentJob) {
    let status_str = AgentJob::status_db_str(&job.status);
    let _ = sqlx::query(
        "INSERT INTO agent_jobs (id, name, status, attempt_count, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&job.id)
    .bind(&job.name)
    .bind(&status_str)
    .bind(job.attempt_count as i64)
    .bind(&job.created_at)
    .bind(&job.updated_at)
    .execute(pool)
    .await;
}

fn parse_job_status(s: &str) -> JobStatus {
    match s {
        "Pending" => JobStatus::Pending,
        "Running" => JobStatus::Running,
        "Cancelled" => JobStatus::Cancelled,
        "Completed" => JobStatus::Completed,
        "Failed" => JobStatus::Failed,
        _ => JobStatus::Pending,
    }
}

/// List jobs by status.
pub async fn list_jobs(pool: &SqlitePool, status: Option<&str>) -> Vec<AgentJob> {
    match status {
        Some(s) => {
            let rows: Vec<(String, String, String, i64, Option<String>, Option<String>, String, String)> = sqlx::query_as(
                "SELECT id, name, status, attempt_count, last_error, result, created_at, updated_at FROM agent_jobs WHERE status = ? ORDER BY created_at DESC",
            )
            .bind(s)
            .fetch_all(pool)
            .await
            .unwrap_or_default();

            rows.into_iter()
                .map(|r| AgentJob {
                    id: r.0,
                    name: r.1,
                    status: parse_job_status(&r.2),
                    attempt_count: r.3 as u32,
                    last_error: r.4,
                    result: r.5,
                    created_at: r.6,
                    updated_at: r.7,
                })
                .collect()
        }
        None => {
            let rows: Vec<(String, String, String, i64, Option<String>, Option<String>, String, String)> = sqlx::query_as(
                "SELECT id, name, status, attempt_count, last_error, result, created_at, updated_at FROM agent_jobs ORDER BY created_at DESC",
            )
            .fetch_all(pool)
            .await
            .unwrap_or_default();

            rows.into_iter()
                .map(|r| AgentJob {
                    id: r.0,
                    name: r.1,
                    status: parse_job_status(&r.2),
                    attempt_count: r.3 as u32,
                    last_error: r.4,
                    result: r.5,
                    created_at: r.6,
                    updated_at: r.7,
                })
                .collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_status_is_final() {
        assert!(JobStatus::Completed.is_final());
        assert!(JobStatus::Failed.is_final());
        assert!(JobStatus::Cancelled.is_final());
        assert!(!JobStatus::Pending.is_final());
        assert!(!JobStatus::Running.is_final());
    }

    #[test]
    fn test_job_status_valid_transitions() {
        assert!(JobStatus::Pending.can_transition_to(&JobStatus::Running));
        assert!(JobStatus::Pending.can_transition_to(&JobStatus::Cancelled));
        assert!(!JobStatus::Pending.can_transition_to(&JobStatus::Completed));

        assert!(JobStatus::Running.can_transition_to(&JobStatus::Completed));
        assert!(JobStatus::Running.can_transition_to(&JobStatus::Failed));
        assert!(JobStatus::Running.can_transition_to(&JobStatus::Cancelled));
        assert!(!JobStatus::Running.can_transition_to(&JobStatus::Pending));

        assert!(!JobStatus::Completed.can_transition_to(&JobStatus::Pending));
        assert!(!JobStatus::Completed.can_transition_to(&JobStatus::Running));
    }

    #[test]
    fn test_job_status_display() {
        assert_eq!(format!("{}", JobStatus::Pending), "保留中");
        assert_eq!(format!("{}", JobStatus::Running), "実行中");
        assert_eq!(format!("{}", JobStatus::Completed), "完了");
        assert_eq!(format!("{}", JobStatus::Failed), "失敗");
        assert_eq!(format!("{}", JobStatus::Cancelled), "キャンセル済み");
    }

    #[test]
    fn test_job_new() {
        let job = AgentJob::new("j1", "test task");
        assert_eq!(job.status, JobStatus::Pending);
        assert_eq!(job.attempt_count, 0);
        assert!(job.result.is_none());
        assert!(job.last_error.is_none());
    }

    #[test]
    fn test_job_serialization() {
        let job = AgentJob::new("j1", "test task");
        let json = serde_json::to_string(&job).unwrap();
        assert!(json.contains("test task"));
        assert!(json.contains("Pending"));
    }

    #[tokio::test]
    async fn test_job_transition_pending_to_running() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        init_agent_jobs_table(&pool).await;

        let job = AgentJob::new("j1", "test");
        insert_job(&pool, &job).await;

        // Pending -> Running
        let ok = job.transition(&pool, JobStatus::Running, None, None).await;
        assert!(ok);
    }

    #[tokio::test]
    async fn test_job_transition_pending_to_completed_fails() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        init_agent_jobs_table(&pool).await;

        let job = AgentJob::new("j2", "test");
        insert_job(&pool, &job).await;

        // Pending cannot go directly to Completed
        let ok = job.transition(&pool, JobStatus::Completed, None, None).await;
        assert!(!ok);
    }

    #[tokio::test]
    async fn test_job_full_lifecycle() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        init_agent_jobs_table(&pool).await;

        let job = AgentJob::new("j3", "full lifecycle test");
        insert_job(&pool, &job).await;

        // Pending -> Running
        assert!(job.transition(&pool, JobStatus::Running, None, None).await);

        // Reload job to get updated status
        let jobs = list_jobs(&pool, Some("Running")).await;
        assert_eq!(jobs.len(), 1);
        let running_job = &jobs[0];
        assert_eq!(running_job.attempt_count, 1);

        // Running -> Completed
        assert!(running_job.transition(&pool, JobStatus::Completed, Some("done"), None).await);

        // Verify final state
        let completed = list_jobs(&pool, Some("Completed")).await;
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].result.as_deref(), Some("done"));
    }

    #[tokio::test]
    async fn test_job_cancel() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        init_agent_jobs_table(&pool).await;

        let job = AgentJob::new("j4", "cancel test");
        insert_job(&pool, &job).await;

        // Pending -> Running
        assert!(job.transition(&pool, JobStatus::Running, None, None).await);

        // Reload for updated status
        let running = list_jobs(&pool, Some("Running")).await;
        assert!(running[0].transition(&pool, JobStatus::Cancelled, None, None).await);

        assert!(AgentJob::is_cancelled(&pool, "j4").await);
    }

    #[tokio::test]
    async fn test_job_list_by_status() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        init_agent_jobs_table(&pool).await;

        let j1 = AgentJob::new("a", "task a");
        let j2 = AgentJob::new("b", "task b");
        insert_job(&pool, &j1).await;
        insert_job(&pool, &j2).await;

        let all = list_jobs(&pool, None).await;
        assert_eq!(all.len(), 2);

        let pending = list_jobs(&pool, Some("Pending")).await;
        assert_eq!(pending.len(), 2);
    }

    #[tokio::test]
    async fn test_job_list_empty() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        init_agent_jobs_table(&pool).await;

        let jobs = list_jobs(&pool, None).await;
        assert!(jobs.is_empty());
    }
}
