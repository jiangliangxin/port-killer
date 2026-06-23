use crate::port_scanner::PortInfo;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KillTarget {
    pub port: u16,
    pub protocol: String,
    pub pid: u32,
    pub local_address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KillResult {
    pub pid: u32,
    pub success: bool,
    pub message: String,
}

fn output_message(output: Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !stderr.is_empty() {
        return stderr;
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !stdout.is_empty() {
        return stdout;
    }

    "taskkill 返回失败".to_string()
}

fn target_still_matches(target: &KillTarget, port: &PortInfo) -> bool {
    target.port == port.port
        && target.protocol == port.protocol
        && target.pid == port.pid
        && target.local_address == port.local_address
}

fn targets_released(targets: &[KillTarget], ports: &[PortInfo]) -> bool {
    !targets
        .iter()
        .any(|target| ports.iter().any(|port| target_still_matches(target, port)))
}

fn wait_until_released(targets: &[KillTarget], timeout: Duration) -> Result<bool, String> {
    let start = Instant::now();

    loop {
        let ports = crate::port_scanner::scan_ports().map_err(|e| e.to_string())?;
        if targets_released(targets, &ports) {
            return Ok(true);
        }

        if start.elapsed() >= timeout {
            return Ok(false);
        }

        std::thread::sleep(Duration::from_millis(150));
    }
}

fn run_taskkill(pid: u32, timeout: Duration, force: bool) -> KillResult {
    if pid <= 4 {
        return KillResult {
            pid,
            success: false,
            message: "无法终止系统进程".to_string(),
        };
    }

    let pid_arg = pid.to_string();
    let mut command = Command::new("taskkill");
    command.args(["/PID", pid_arg.as_str()]);
    if force {
        command.arg("/F");
    }

    let mut child = match command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            return KillResult {
                pid,
                success: false,
                message: format!("启动 taskkill 失败: {}", e),
            }
        }
    };

    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if status.success() {
                    return KillResult {
                        pid,
                        success: true,
                        message: "已成功终止".to_string(),
                    };
                } else {
                    let message = child
                        .wait_with_output()
                        .ok()
                        .map(output_message)
                        .unwrap_or_else(|| "未知错误".to_string());
                    return KillResult {
                        pid,
                        success: false,
                        message,
                    };
                }
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    child.wait().ok();
                    return KillResult {
                        pid,
                        success: false,
                        message: "终止超时".to_string(),
                    };
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                return KillResult {
                    pid,
                    success: false,
                    message: format!("等待 taskkill 失败: {}", e),
                }
            }
        }
    }
}

fn close_single(pid: u32, targets: Vec<KillTarget>, timeout: Duration, force: bool) -> KillResult {
    let result = run_taskkill(pid, timeout, force);
    if !result.success {
        return result;
    }

    match wait_until_released(&targets, Duration::from_secs(2)) {
        Ok(true) => KillResult {
            pid,
            success: true,
            message: if force {
                "已强制关闭并释放端口".to_string()
            } else {
                "已正常关闭并释放端口".to_string()
            },
        },
        Ok(false) => KillResult {
            pid,
            success: false,
            message: if force {
                "已执行强制关闭，但端口仍未释放".to_string()
            } else {
                "正常关闭后端口仍未释放，可尝试强制关闭".to_string()
            },
        },
        Err(e) => KillResult {
            pid,
            success: false,
            message: format!("已执行关闭，但复查端口失败: {}", e),
        },
    }
}

pub fn kill_processes(
    targets: &[KillTarget],
    current_ports: &[PortInfo],
    force: bool,
) -> Vec<KillResult> {
    let mut targets_by_pid: BTreeMap<u32, Vec<KillTarget>> = BTreeMap::new();
    let mut skipped = Vec::new();

    for target in targets {
        // 关闭进程前重新确认端口归属，避免 PID 复用或刷新延迟导致误杀。
        if current_ports
            .iter()
            .any(|port| target_still_matches(target, port))
        {
            targets_by_pid
                .entry(target.pid)
                .or_default()
                .push(target.clone());
        } else {
            skipped.push(KillResult {
                pid: target.pid,
                success: false,
                message: format!("{} 已不再由该进程占用，已跳过", target.local_address),
            });
        }
    }

    let timeout = Duration::from_secs(5);
    let mut results: Vec<KillResult> = std::thread::scope(|s| {
        targets_by_pid
            .into_iter()
            .map(|(pid, targets)| s.spawn(move || close_single(pid, targets, timeout, force)))
            .map(|h| {
                h.join().unwrap_or(KillResult {
                    pid: 0,
                    success: false,
                    message: "线程执行失败".to_string(),
                })
            })
            .collect()
    });
    results.extend(skipped);
    results
}
