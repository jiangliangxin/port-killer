use crate::port_scanner::PortInfo;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
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

fn kill_single(pid: u32, timeout: Duration) -> KillResult {
    if pid <= 4 {
        return KillResult {
            pid,
            success: false,
            message: "无法终止系统进程".to_string(),
        };
    }

    let mut child = match Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/F"])
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

pub fn kill_processes(targets: &[KillTarget], current_ports: &[PortInfo]) -> Vec<KillResult> {
    let mut pids = BTreeSet::new();
    let mut skipped = Vec::new();

    for target in targets {
        // 杀进程前重新确认端口归属，避免 PID 复用或刷新延迟导致误杀。
        if current_ports
            .iter()
            .any(|port| target_still_matches(target, port))
        {
            pids.insert(target.pid);
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
        pids.into_iter()
            .map(|pid| s.spawn(move || kill_single(pid, timeout)))
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
