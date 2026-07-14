use serde_json::Value;

const ACTIONS: &[&str] = &[
    "list",
    "get",
    "create",
    "update",
    "delete",
    "start",
    "stop",
    "start_all",
    "stop_all",
    "status",
    "log_list",
    "log_clear",
    "stats_get",
    "stats_reset",
];

#[cfg(test)]
pub(crate) fn supported_actions() -> &'static [&'static str] {
    ACTIONS
}

pub fn execute(action: &str, _payload: &Value) -> Result<Value, String> {
    if !ACTIONS.contains(&action) {
        return Err("unsupported request_forward action".into());
    }
    Err("request_forward action not implemented".into())
}
