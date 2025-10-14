use serde::{Deserialize, Serialize};

use crate::errors::RouterErrorCode;

#[derive(Serialize, Deserialize, Debug)]
pub struct RouterError {
    pub code: RouterErrorCode,
    pub error: String,
    pub subsystem: String,
    pub stage: String,
    pub cause: String,
    pub ts: u64,
    pub thread: String,
    pub description: String,
    pub extra: Option<serde_json::Value>,
}

impl RouterError {
    fn generate_metadata() -> (u64, String) {
        let ts = {
            use std::time::{SystemTime, UNIX_EPOCH};
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default();
            now.as_millis() as u64
        };
        let thread = format!("{:?}", std::thread::current().id());
        (ts, thread)
    }

    pub fn new(
        code: RouterErrorCode,
        subsystem: &str,
        stage: &str,
        cause: &str,
        description: String,
        detail: Option<serde_json::Value>,
    ) -> Self {
        let (ts, thread) = Self::generate_metadata();

        RouterError {
            error: code.name().to_string(),
            code,
            subsystem: subsystem.to_string(),
            stage: stage.to_string(),
            cause: cause.to_string(),
            ts,
            thread,
            description,
            extra: detail,
        }
    }

    pub fn merge_extra(&mut self, new_extra: serde_json::Value) {
        if let Some(existing_extra) = self.extra.as_mut() {
            if let serde_json::Value::Object(existing_map) = existing_extra
                && let serde_json::Value::Object(new_map) = new_extra
            {
                existing_map.extend(new_map);
            }
        } else {
            self.extra = Some(new_extra);
        }
    }
}

pub type RouterResult<T> = Result<T, Box<RouterError>>;
