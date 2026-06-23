use crate::port_scanner::PortInfo;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::io::ErrorKind;
use std::os::windows::process::CommandExt;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};
use windows::Win32::Globalization::{GetOEMCP, MultiByteToWideChar, MULTI_BYTE_TO_WIDE_CHAR_FLAGS};

const CREATE_NO_WINDOW: u32 = 0x08000000;

enum ReleaseCheckStatus {
    Released,
    StillOwned,
    Reoccupied(Vec<u32>),
}

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
    pub status: String,
    pub message: String,
}

fn decode_command_output(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }

    unsafe {
        let code_page = GetOEMCP();
        let len = MultiByteToWideChar(code_page, MULTI_BYTE_TO_WIDE_CHAR_FLAGS(0), bytes, None);
        if len > 0 {
            let mut wide = vec![0u16; len as usize];
            let written = MultiByteToWideChar(
                code_page,
                MULTI_BYTE_TO_WIDE_CHAR_FLAGS(0),
                bytes,
                Some(&mut wide),
            );
            if written > 0 {
                return String::from_utf16_lossy(&wide[..written as usize]);
            }
        }
    }

    String::from_utf8_lossy(bytes).to_string()
}

fn output_message(output: Output) -> String {
    let stderr = decode_command_output(&output.stderr).trim().to_string();
    if !stderr.is_empty() {
        return stderr;
    }

    let stdout = decode_command_output(&output.stdout).trim().to_string();
    if !stdout.is_empty() {
        return stdout;
    }

    "taskkill 返回失败".to_string()
}

fn is_permission_denied_message(message: &str) -> bool {
    let lower = message.to_lowercase();
    lower.contains("access is denied")
        || lower.contains("permission")
        || message.contains("拒绝访问")
        || message.contains("权限")
}

fn requires_force_message(message: &str) -> bool {
    let lower = message.to_lowercase();
    lower.contains("/f")
        || lower.contains("forcefully")
        || lower.contains("force")
        || message.contains("强制")
}

fn permission_denied_result(pid: u32) -> KillResult {
    KillResult {
        pid,
        success: false,
        status: "permissionDenied".to_string(),
        message: "权限不足，无法关闭该进程；请以管理员身份重新运行后再试".to_string(),
    }
}

fn target_still_matches(target: &KillTarget, port: &PortInfo) -> bool {
    target.port == port.port
        && target.protocol == port.protocol
        && target.pid == port.pid
        && target.local_address == port.local_address
}

fn target_endpoint_matches(target: &KillTarget, port: &PortInfo) -> bool {
    target.port == port.port
        && target.protocol == port.protocol
        && target.local_address == port.local_address
}

fn original_targets_still_owned(targets: &[KillTarget], ports: &[PortInfo]) -> bool {
    targets
        .iter()
        .any(|target| ports.iter().any(|port| target_still_matches(target, port)))
}

fn target_endpoints_still_occupied(targets: &[KillTarget], ports: &[PortInfo]) -> bool {
    targets.iter().any(|target| {
        ports
            .iter()
            .any(|port| target_endpoint_matches(target, port))
    })
}

fn reoccupying_pids(targets: &[KillTarget], ports: &[PortInfo]) -> Vec<u32> {
    let mut pids = BTreeSet::new();
    for target in targets {
        for port in ports {
            if target_endpoint_matches(target, port) && !target_still_matches(target, port) {
                pids.insert(port.pid);
            }
        }
    }
    pids.into_iter().collect()
}

fn wait_until_released(
    targets: &[KillTarget],
    timeout: Duration,
) -> Result<ReleaseCheckStatus, String> {
    let start = Instant::now();
    let stable_free_duration = Duration::from_millis(300);
    let mut free_since: Option<Instant> = None;

    loop {
        let ports = crate::port_scanner::scan_ports().map_err(|e| e.to_string())?;
        let original_still_owned = original_targets_still_owned(targets, &ports);
        let endpoint_still_occupied = target_endpoints_still_occupied(targets, &ports);

        if !endpoint_still_occupied {
            let free_start = free_since.get_or_insert_with(Instant::now);
            if free_start.elapsed() >= stable_free_duration {
                return Ok(ReleaseCheckStatus::Released);
            }
        } else {
            free_since = None;
        }

        if !original_still_owned && endpoint_still_occupied {
            return Ok(ReleaseCheckStatus::Reoccupied(reoccupying_pids(
                targets, &ports,
            )));
        }

        if start.elapsed() >= timeout {
            return if original_still_owned {
                Ok(ReleaseCheckStatus::StillOwned)
            } else {
                Ok(ReleaseCheckStatus::Reoccupied(reoccupying_pids(
                    targets, &ports,
                )))
            };
        }

        std::thread::sleep(Duration::from_millis(150));
    }
}

fn run_taskkill(pid: u32, timeout: Duration, force: bool) -> KillResult {
    if pid <= 4 {
        return KillResult {
            pid,
            success: false,
            status: "failed".to_string(),
            message: "无法终止系统进程".to_string(),
        };
    }

    let pid_arg = pid.to_string();
    let mut command = Command::new("taskkill");
    command.args(["/PID", pid_arg.as_str()]);
    command.creation_flags(CREATE_NO_WINDOW);
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
            if e.kind() == ErrorKind::PermissionDenied {
                return permission_denied_result(pid);
            }

            return KillResult {
                pid,
                success: false,
                status: "failed".to_string(),
                message: format!("启动 taskkill 失败: {}", e),
            };
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
                        status: "closed".to_string(),
                        message: "已成功终止".to_string(),
                    };
                } else {
                    let message = child
                        .wait_with_output()
                        .ok()
                        .map(output_message)
                        .unwrap_or_else(|| "未知错误".to_string());
                    if is_permission_denied_message(&message) {
                        return permission_denied_result(pid);
                    }
                    if !force && requires_force_message(&message) {
                        return KillResult {
                            pid,
                            success: false,
                            status: "notReleased".to_string(),
                            message: "正常关闭失败：该进程需要使用强制结束，可点击“强制关闭”重试"
                                .to_string(),
                        };
                    }

                    return KillResult {
                        pid,
                        success: false,
                        status: "failed".to_string(),
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
                        status: "failed".to_string(),
                        message: "终止超时".to_string(),
                    };
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                return KillResult {
                    pid,
                    success: false,
                    status: "failed".to_string(),
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
        Ok(ReleaseCheckStatus::Released) => KillResult {
            pid,
            success: true,
            status: "released".to_string(),
            message: if force {
                "已强制关闭并释放端口".to_string()
            } else {
                "已正常关闭并释放端口".to_string()
            },
        },
        Ok(ReleaseCheckStatus::StillOwned) => KillResult {
            pid,
            success: false,
            status: "notReleased".to_string(),
            message: if force {
                "已执行强制关闭，但端口仍未释放".to_string()
            } else {
                "正常关闭后端口仍未释放，可尝试强制关闭".to_string()
            },
        },
        Ok(ReleaseCheckStatus::Reoccupied(pids)) => KillResult {
            pid,
            success: false,
            status: "reoccupied".to_string(),
            message: if pids.is_empty() {
                "原进程已关闭，但端口又被其他进程占用".to_string()
            } else {
                format!(
                    "原进程已关闭，但端口又被 PID {} 占用",
                    pids.iter()
                        .map(u32::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            },
        },
        Err(e) => KillResult {
            pid,
            success: false,
            status: "failed".to_string(),
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
                status: "skipped".to_string(),
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
                    status: "failed".to_string(),
                    message: "线程执行失败".to_string(),
                })
            })
            .collect()
    });
    results.extend(skipped);
    results
}
