use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const ACTIONS: &[&str] = &["detect"];
const COMMAND_TIMEOUT: Duration = Duration::from_secs(3);
const FINDER_TIMEOUT: Duration = Duration::from_secs(1);
const POLL_INTERVAL: Duration = Duration::from_millis(10);

#[derive(Clone, Copy)]
struct ToolSpec {
    key: &'static str,
    name: &'static str,
    command: &'static str,
    version_args: &'static [&'static str],
    prefer_stderr: bool,
    install_hint: &'static str,
}

const TOOL_SPECS: &[ToolSpec] = &[
    ToolSpec {
        key: "node",
        name: "Node.js",
        command: "node",
        version_args: &["--version"],
        prefer_stderr: false,
        install_hint: "安装 Node.js LTS，并确认 node 已加入 PATH",
    },
    ToolSpec {
        key: "npm",
        name: "npm",
        command: "npm",
        version_args: &["--version"],
        prefer_stderr: false,
        install_hint: "npm 通常随 Node.js 安装，请检查 Node.js 安装目录和 PATH",
    },
    ToolSpec {
        key: "pnpm",
        name: "pnpm",
        command: "pnpm",
        version_args: &["--version"],
        prefer_stderr: false,
        install_hint: "通过 Corepack 或 npm 安装 pnpm，并确认 pnpm 已加入 PATH",
    },
    ToolSpec {
        key: "python",
        name: "Python",
        command: "python",
        version_args: &["--version"],
        prefer_stderr: false,
        install_hint: "安装 Python 3，并在安装时启用 Add Python to PATH",
    },
    ToolSpec {
        key: "pip",
        name: "pip",
        command: "pip",
        version_args: &["--version"],
        prefer_stderr: false,
        install_hint: "使用 python -m ensurepip 修复 pip，或检查 Python Scripts 目录是否在 PATH",
    },
    ToolSpec {
        key: "java",
        name: "Java",
        command: "java",
        version_args: &["-version"],
        prefer_stderr: true,
        install_hint: "安装 JDK，并配置 JAVA_HOME 与 PATH",
    },
    ToolSpec {
        key: "javac",
        name: "Javac",
        command: "javac",
        version_args: &["-version"],
        prefer_stderr: false,
        install_hint: "当前可能只有 JRE；请安装完整 JDK 并配置 JAVA_HOME",
    },
    ToolSpec {
        key: "maven",
        name: "Maven",
        command: "mvn",
        version_args: &["--version"],
        prefer_stderr: false,
        install_hint: "安装 Maven，并配置 MAVEN_HOME 与 PATH",
    },
    ToolSpec {
        key: "gradle",
        name: "Gradle",
        command: "gradle",
        version_args: &["--version"],
        prefer_stderr: false,
        install_hint: "安装 Gradle，或优先使用项目自带的 Gradle Wrapper",
    },
    ToolSpec {
        key: "rustc",
        name: "Rustc",
        command: "rustc",
        version_args: &["--version"],
        prefer_stderr: false,
        install_hint: "通过 rustup 安装 Rust 工具链",
    },
    ToolSpec {
        key: "cargo",
        name: "Cargo",
        command: "cargo",
        version_args: &["--version"],
        prefer_stderr: false,
        install_hint: "通过 rustup 安装 Rust 工具链，并检查 CARGO_HOME/bin 是否在 PATH",
    },
    ToolSpec {
        key: "git",
        name: "Git",
        command: "git",
        version_args: &["--version"],
        prefer_stderr: false,
        install_hint: "安装 Git，并在安装选项中允许从命令行使用 Git",
    },
    ToolSpec {
        key: "docker",
        name: "Docker",
        command: "docker",
        version_args: &["--version"],
        prefer_stderr: false,
        install_hint: "安装 Docker Desktop 或 Docker CLI，并确认 docker 已加入 PATH",
    },
];

#[derive(Debug)]
struct TimedOutput {
    output: Output,
    timed_out: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolResult {
    key: String,
    name: String,
    installed: bool,
    status: &'static str,
    version: String,
    path: String,
    paths: Vec<String>,
    error: Option<String>,
    suggestion: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EnvironmentItem {
    key: &'static str,
    value: String,
    status: &'static str,
    detail: String,
}

#[derive(Serialize)]
struct Diagnostic {
    level: &'static str,
    title: String,
    detail: String,
    suggestion: String,
}

#[cfg(test)]
pub(crate) fn supported_actions() -> &'static [&'static str] {
    ACTIONS
}

pub fn execute(action: &str, _payload: &Value) -> Result<Value, String> {
    if !ACTIONS.contains(&action) {
        return Err(format!("unsupported env action: {action}"));
    }
    match action {
        "detect" => detect(),
        _ => Err(format!("unsupported env action: {action}")),
    }
}

fn detect() -> Result<Value, String> {
    let started_at = Instant::now();
    let handles = TOOL_SPECS
        .iter()
        .copied()
        .map(|spec| (spec, thread::spawn(move || detect_tool(spec))))
        .collect::<Vec<_>>();

    let tools = handles
        .into_iter()
        .map(|(spec, handle)| {
            handle.join().unwrap_or_else(|_| ToolResult {
                key: spec.key.to_string(),
                name: spec.name.to_string(),
                installed: false,
                status: "error",
                version: "检测失败".to_string(),
                path: String::new(),
                paths: Vec::new(),
                error: Some("检测线程异常退出".to_string()),
                suggestion: Some("重新检测；若持续失败，请检查系统进程与安全软件限制".to_string()),
            })
        })
        .collect::<Vec<_>>();

    let environment = inspect_environment();
    let diagnostics = build_diagnostics(&tools, &environment);
    let installed = tools.iter().filter(|tool| tool.installed).count();
    let problems = tools
        .iter()
        .filter(|tool| matches!(tool.status, "error" | "timeout"))
        .count();
    let warnings = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.level == "warning")
        .count();

    Ok(json!({
        "platform": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "durationMs": started_at.elapsed().as_millis(),
        "summary": {
            "total": tools.len(),
            "installed": installed,
            "missing": tools.len().saturating_sub(installed),
            "problems": problems,
            "warnings": warnings
        },
        "tools": tools,
        "environment": environment,
        "diagnostics": diagnostics
    }))
}

fn detect_tool(spec: ToolSpec) -> ToolResult {
    let paths = command_paths(spec.command);
    match run_command(spec.command, spec.version_args, COMMAND_TIMEOUT) {
        Ok(result) if result.timed_out => ToolResult {
            key: spec.key.to_string(),
            name: spec.name.to_string(),
            installed: false,
            status: "timeout",
            version: "检测超时".to_string(),
            path: paths.first().cloned().unwrap_or_default(),
            paths,
            error: Some(format!(
                "版本命令超过 {} 秒未完成",
                COMMAND_TIMEOUT.as_secs()
            )),
            suggestion: Some(
                "单独运行版本命令检查卡点，并确认代理、运行时或安全软件没有阻塞".to_string(),
            ),
        },
        Ok(result) if !result.output.status.success() => {
            let detail = command_error_detail(&result.output);
            ToolResult {
                key: spec.key.to_string(),
                name: spec.name.to_string(),
                installed: false,
                status: "error",
                version: "命令异常".to_string(),
                path: paths.first().cloned().unwrap_or_default(),
                paths,
                error: Some(detail),
                suggestion: Some(
                    "该命令可找到但无法正常运行，请检查安装完整性、运行时依赖和 PATH 顺序"
                        .to_string(),
                ),
            }
        }
        Ok(result) => {
            let version = preferred_output_line(&result.output, spec.prefer_stderr);
            ToolResult {
                key: spec.key.to_string(),
                name: spec.name.to_string(),
                installed: true,
                status: "ok",
                version,
                path: paths.first().cloned().unwrap_or_default(),
                paths,
                error: None,
                suggestion: None,
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => ToolResult {
            key: spec.key.to_string(),
            name: spec.name.to_string(),
            installed: false,
            status: "missing",
            version: "未找到".to_string(),
            path: String::new(),
            paths: Vec::new(),
            error: Some("系统找不到命令".to_string()),
            suggestion: Some(spec.install_hint.to_string()),
        },
        Err(error) => ToolResult {
            key: spec.key.to_string(),
            name: spec.name.to_string(),
            installed: false,
            status: "error",
            version: "检测失败".to_string(),
            path: paths.first().cloned().unwrap_or_default(),
            paths,
            error: Some(error.to_string()),
            suggestion: Some("检查命令执行权限与系统安全策略后重试".to_string()),
        },
    }
}

fn run_command(command: &str, args: &[&str], timeout: Duration) -> std::io::Result<TimedOutput> {
    let mut process = Command::new(command);
    process
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        process.creation_flags(0x0800_0000);
    }
    let mut child = process.spawn()?;
    let started_at = Instant::now();
    loop {
        if child.try_wait()?.is_some() {
            return child.wait_with_output().map(|output| TimedOutput {
                output,
                timed_out: false,
            });
        }
        if started_at.elapsed() >= timeout {
            let _ = child.kill();
            return child.wait_with_output().map(|output| TimedOutput {
                output,
                timed_out: true,
            });
        }
        thread::sleep(POLL_INTERVAL);
    }
}

fn command_paths(command: &str) -> Vec<String> {
    #[cfg(target_os = "windows")]
    let finder = ("where", vec![command]);
    #[cfg(not(target_os = "windows"))]
    let finder = ("which", vec!["-a", command]);

    let Ok(result) = run_command(finder.0, &finder.1, FINDER_TIMEOUT) else {
        return Vec::new();
    };
    if result.timed_out || !result.output.status.success() {
        return Vec::new();
    }
    unique_lines(&String::from_utf8_lossy(&result.output.stdout))
}

fn unique_lines(text: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            #[cfg(target_os = "windows")]
            let key = line.to_lowercase();
            #[cfg(not(target_os = "windows"))]
            let key = line.to_string();
            seen.insert(key).then(|| line.to_string())
        })
        .collect()
}

fn first_non_empty_line(text: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(ToString::to_string)
}

fn preferred_output_line(output: &Output, prefer_stderr: bool) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let (preferred, fallback) = if prefer_stderr {
        (stderr.as_ref(), stdout.as_ref())
    } else {
        (stdout.as_ref(), stderr.as_ref())
    };
    first_non_empty_line(preferred)
        .or_else(|| first_non_empty_line(fallback))
        .unwrap_or_else(|| "版本命令未返回文本".to_string())
}

fn command_error_detail(output: &Output) -> String {
    let exit = output
        .status
        .code()
        .map(|code| format!("退出码 {code}"))
        .unwrap_or_else(|| "进程被系统终止".to_string());
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let message = first_non_empty_line(&stderr).or_else(|| first_non_empty_line(&stdout));
    match message {
        Some(message) => format!("{exit}：{message}"),
        None => exit,
    }
}

fn inspect_environment() -> Vec<EnvironmentItem> {
    const VARIABLES: &[(&str, &str)] = &[
        ("JAVA_HOME", "Java/JDK 根目录"),
        ("MAVEN_HOME", "Maven 根目录"),
        ("GRADLE_HOME", "Gradle 根目录"),
        ("CARGO_HOME", "Cargo 数据目录"),
    ];
    VARIABLES
        .iter()
        .map(|(key, description)| match std::env::var(key) {
            Ok(value) if value.trim().is_empty() => EnvironmentItem {
                key,
                value,
                status: "missing",
                detail: format!("{description}已定义但值为空"),
            },
            Ok(value) if Path::new(&value).exists() => EnvironmentItem {
                key,
                value,
                status: "ok",
                detail: format!("{description}可访问"),
            },
            Ok(value) => EnvironmentItem {
                key,
                value,
                status: "invalid",
                detail: format!("{description}指向的路径不存在"),
            },
            Err(_) => EnvironmentItem {
                key,
                value: String::new(),
                status: "missing",
                detail: format!("未配置{description}"),
            },
        })
        .collect()
}

fn build_diagnostics(tools: &[ToolResult], environment: &[EnvironmentItem]) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for tool in tools {
        match tool.status {
            "missing" => diagnostics.push(Diagnostic {
                level: "info",
                title: format!("{} 未安装或不在 PATH", tool.name),
                detail: "只有使用对应技术栈时才需要处理。".to_string(),
                suggestion: tool.suggestion.clone().unwrap_or_default(),
            }),
            "timeout" | "error" => diagnostics.push(Diagnostic {
                level: "error",
                title: format!("{} 无法正常执行", tool.name),
                detail: tool.error.clone().unwrap_or_else(|| "检测失败".to_string()),
                suggestion: tool.suggestion.clone().unwrap_or_default(),
            }),
            _ => {}
        }
        if tool.paths.len() > 1 {
            diagnostics.push(Diagnostic {
                level: "warning",
                title: format!("{} 存在多个 PATH 命中", tool.name),
                detail: format!(
                    "当前优先使用 {}，共发现 {} 个路径。",
                    tool.path,
                    tool.paths.len()
                ),
                suggestion: "确认首个路径就是期望版本；如不是，请调整 PATH 顺序并重新打开应用。"
                    .to_string(),
            });
        }
    }

    for item in environment.iter().filter(|item| item.status == "invalid") {
        diagnostics.push(Diagnostic {
            level: "warning",
            title: format!("{} 配置无效", item.key),
            detail: format!("{}：{}", item.detail, item.value),
            suggestion: format!("修正或移除 {}，然后重新打开终端和 LazyCat。", item.key),
        });
    }

    if tool_is_ok(tools, "java") && environment_status(environment, "JAVA_HOME") == "missing" {
        diagnostics.push(Diagnostic {
            level: "warning",
            title: "Java 可用，但未配置 JAVA_HOME".to_string(),
            detail: "部分 Maven、Gradle 和 IDE 任务仍可能找不到 JDK。".to_string(),
            suggestion: "将 JAVA_HOME 指向 JDK 根目录，并把其 bin 目录加入 PATH。".to_string(),
        });
    }
    diagnostics
}

fn tool_is_ok(tools: &[ToolResult], key: &str) -> bool {
    tools
        .iter()
        .any(|tool| tool.key == key && tool.status == "ok")
}

fn environment_status<'a>(environment: &'a [EnvironmentItem], key: &str) -> &'a str {
    environment
        .iter()
        .find(|item| item.key == key)
        .map(|item| item.status)
        .unwrap_or("missing")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_failure_is_not_reported_as_installed() {
        #[cfg(target_os = "windows")]
        let result =
            run_command("cmd", &["/C", "exit 7"], Duration::from_secs(1)).expect("run cmd");
        #[cfg(not(target_os = "windows"))]
        let result = run_command("sh", &["-c", "exit 7"], Duration::from_secs(1)).expect("run sh");
        assert!(!result.output.status.success());
        assert!(!result.timed_out);
        assert!(command_error_detail(&result.output).contains("7"));
    }

    #[test]
    fn command_timeout_stops_waiting() {
        #[cfg(target_os = "windows")]
        let result = run_command(
            "cmd",
            &["/C", "ping -n 3 127.0.0.1 > nul"],
            Duration::from_millis(20),
        )
        .expect("run cmd");
        #[cfg(not(target_os = "windows"))]
        let result =
            run_command("sh", &["-c", "sleep 1"], Duration::from_millis(20)).expect("run sh");
        assert!(result.timed_out);
    }

    #[test]
    fn unique_lines_removes_repeated_paths() {
        let paths = unique_lines("C:/tools/node.exe\nC:/tools/node.exe\nD:/node.exe\n");
        assert_eq!(paths.len(), 2);
    }

    #[test]
    fn detect_should_return_diagnostics_structure() {
        let out = execute("detect", &json!({})).expect("detect");
        assert!(out["summary"]["total"].as_u64().unwrap_or(0) >= 13);
        assert!(out["tools"].is_array());
        assert!(out["environment"].is_array());
        assert!(out["diagnostics"].is_array());
        assert!(out["durationMs"].is_number());
    }
}
