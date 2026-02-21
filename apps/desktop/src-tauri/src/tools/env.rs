use serde_json::{json, Value};
use std::process::Command;
use std::time::Instant;

fn first_non_empty_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("UNKNOWN")
        .to_string()
}

fn command_path(command: &str) -> Option<String> {
    #[cfg(target_os = "windows")]
    let finder = "where";
    #[cfg(not(target_os = "windows"))]
    let finder = "which";

    let output = Command::new(finder).arg(command).output().ok()?;
    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let path = stdout
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(ToString::to_string)?;

    Some(path)
}

fn detect_tool(
    key: &str,
    name: &str,
    command: &str,
    version_args: &[&str],
    version_from_stderr: bool,
) -> Value {
    let output = Command::new(command).args(version_args).output();

    match output {
        Ok(o) => {
            let raw = if version_from_stderr {
                String::from_utf8_lossy(&o.stderr).to_string()
            } else {
                String::from_utf8_lossy(&o.stdout).to_string()
            };
            let version = first_non_empty_line(&raw);
            let path = command_path(command).unwrap_or_default();

            json!({
                "key": key,
                "name": name,
                "installed": true,
                "version": version,
                "path": path,
                "error": Value::Null
            })
        }
        Err(err) => {
            json!({
                "key": key,
                "name": name,
                "installed": false,
                "version": "NOT_FOUND",
                "path": "",
                "error": err.to_string()
            })
        }
    }
}

pub fn execute(action: &str, _payload: &Value) -> Result<Value, String> {
    match action {
        "detect" => {
            let started_at = Instant::now();
            let tools = vec![
                detect_tool("node", "Node.js", "node", &["--version"], false),
                detect_tool("npm", "npm", "npm", &["--version"], false),
                detect_tool("pnpm", "pnpm", "pnpm", &["--version"], false),
                detect_tool("python", "Python", "python", &["--version"], false),
                detect_tool("pip", "pip", "pip", &["--version"], false),
                detect_tool("java", "Java", "java", &["-version"], true),
                detect_tool("javac", "Javac", "javac", &["-version"], false),
                detect_tool("rustc", "Rustc", "rustc", &["--version"], false),
                detect_tool("cargo", "Cargo", "cargo", &["--version"], false),
                detect_tool("git", "Git", "git", &["--version"], false),
            ];

            let total = tools.len();
            let installed = tools
                .iter()
                .filter(|item| item.get("installed").and_then(Value::as_bool).unwrap_or(false))
                .count();
            let missing = total.saturating_sub(installed);

            Ok(json!({
                "platform": std::env::consts::OS,
                "arch": std::env::consts::ARCH,
                "duration_ms": started_at.elapsed().as_millis(),
                "summary": {
                    "total": total,
                    "installed": installed,
                    "missing": missing
                },
                "tools": tools
            }))
        }
        _ => Err(format!("unsupported env action: {action}")),
    }
}
