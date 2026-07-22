#[cfg(test)]
use ssh2::Session;

#[cfg(test)]
pub(crate) fn new_session() -> Result<Session, String> {
    Session::new().map_err(|error| format!("创建 SSH 会话失败：{error}"))
}

#[cfg(test)]
mod tests {
    use super::new_session;

    #[test]
    fn creates_an_ssh_session_without_network_access() {
        assert!(new_session().is_ok());
    }
}