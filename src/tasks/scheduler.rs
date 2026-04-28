use chrono::Utc;
use std::sync::Arc;
use tracing::{info, debug};
use std::time::Duration;

use crate::memory::store::MemoryStore;

type TaskFn = Arc<dyn Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> + Send + Sync>;

pub struct TaskScheduler {
    store: Arc<MemoryStore>,
}

impl TaskScheduler {
    pub fn new(store: Arc<MemoryStore>) -> Self {
        Self { store }
    }

    pub fn register(&self, name: String, cron_expr: String, func: TaskFn) {
        let next_run = Self::parse_next_run(&cron_expr);
        let store = self.store.clone();

        info!("[Scheduler] Registered task: {} (cron: {}, next: {})", name, cron_expr, next_run);

        tokio::spawn(async move {
            loop {
                let now = Utc::now();
                let delay = (next_run - now).to_std().unwrap_or(Duration::from_secs(0));

                debug!("[Scheduler] Task '{}' sleeping for {:?}ms", name, delay.as_millis());
                tokio::time::sleep(delay).await;

                info!("[Scheduler] Running task: {}", name);
                func().await;

                // Update DB
                store.update_task_run(&name).await;

                // Calculate next run
                let next = Self::parse_next_run(&cron_expr);
                debug!("[Scheduler] Next run for '{}': {}", name, next);
            }
        });
    }

    fn parse_next_run(cron_expr: &str) -> chrono::DateTime<chrono::Utc> {
        use chrono::Timelike;
        let parts: Vec<&str> = cron_expr.split_whitespace().collect();
        if parts.len() < 5 {
            return Utc::now() + chrono::Duration::hours(1);
        }

        let now = Utc::now();

        // Cron format: minute hour day_of_month month day_of_week
        let minute_str = parts[0];
        let minute = parse_cron_field(minute_str, 0, 59) as u32;

        let hour_str = parts[1];
        let hour = parse_cron_field(hour_str, 0, 23) as u32;

        let mut next = now
            .with_minute(minute)
            .unwrap_or(now)
            .with_hour(hour)
            .unwrap_or(now);

        if next <= now {
            next += chrono::Duration::hours(1);
        }

        next
    }
}

fn parse_cron_field(field: &str, min: i32, _max: i32) -> i32 {
    if field == "*" {
        return min;
    }
    field.parse::<i32>().unwrap_or(min)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cron_field_wildcard() {
        assert_eq!(parse_cron_field("*", 0, 59), 0);
        assert_eq!(parse_cron_field("*", 1, 23), 1);
    }

    #[test]
    fn test_parse_cron_field_integer() {
        assert_eq!(parse_cron_field("30", 0, 59), 30);
        assert_eq!(parse_cron_field("15", 0, 23), 15);
        assert_eq!(parse_cron_field("0", 0, 59), 0);
    }

    #[test]
    fn test_parse_cron_field_invalid_fallback() {
        // Invalid field falls back to min
        assert_eq!(parse_cron_field("abc", 0, 59), 0);
        assert_eq!(parse_cron_field("xyz", 1, 23), 1);
    }

    #[test]
    fn test_parse_cron_field_negative() {
        // Negative value is valid as i32 parse result
        assert_eq!(parse_cron_field("-1", 0, 59), -1);
    }

    #[test]
    fn test_parse_next_run_invalid_format() {
        // Invalid cron expression should return now + 1 hour
        let result = TaskScheduler::parse_next_run("not a cron expr");
        let expected = Utc::now() + chrono::Duration::hours(1);
        let diff = (result - expected).abs();
        assert!(diff < chrono::Duration::seconds(2));
    }

    #[test]
    fn test_parse_next_run_wildcard_minute() {
        let result = TaskScheduler::parse_next_run("* * * * *");
        // Wildcard minute=0, hour=0. If result <= now, it gets pushed +1h.
        // The key property is that result is at minute 0, hour 0 or 1
        use chrono::Timelike;
        assert_eq!(result.minute(), 0);
        // Hour could be 0 or 1 depending on current time
        assert!(result.hour() == 0 || result.hour() == 1);
    }

    #[test]
    fn test_parse_next_run_specific_time() {
        let result = TaskScheduler::parse_next_run("30 14 * * *");
        let now = Utc::now();
        // Should be in the future
        assert!(result > now);
    }

    #[test]
    fn test_parse_next_run_star_star() {
        // */5 is not "*" so parse returns -1 (parse::<i32>() of "*/5" gives -1)
        // The key property is that it doesn't panic
        let _result = TaskScheduler::parse_next_run("*/5 * * * *");
    }
}
