use crate::common::{
    permission_denied_result, wait_until_released, KillResult, KillTarget, PortInfo,
    ReleaseCheckStatus,
};
use std::collections::BTreeMap;
use std::process::{Command, Stdio};
use std::time::Duration;

fn run_kill(pid: u32, force: bool) -> KillResult {
    let mut command = Command::new("kill");
    if force {
        command.arg("-9");
    }
    command.arg(pid.to_string());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    match command.output() {
        Ok(output) => {
            if output.status.success() {
                KillResult {
                    pid,
                    success: true,
                    status: "closed".to_string(),
                    message: "已成功终止".to_string(),
                }
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                if stderr.contains("No such process") {
                    // 进程已退出，视为成功
                    KillResult {
                        pid,
                        success: true,
                        status: "closed".to_string(),
                        message: "进程已退出".to_string(),
                    }
                } else if stderr.contains("Operation not permitted") {
                    permission_denied_result(pid)
                } else if stderr.contains("No matching processes") {
                    KillResult {
                        pid,
                        success: false,
                        status: "failed".to_string(),
                        message: "未找到匹配的进程".to_string(),
                    }
                } else {
                    KillResult {
                        pid,
                        success: false,
                        status: "failed".to_string(),
                        message: if stderr.is_empty() {
                            "终止失败".to_string()
                        } else {
                            stderr
                        },
                    }
                }
            }
        }
        Err(e) => KillResult {
            pid,
            success: false,
            status: "failed".to_string(),
            message: format!("启动 kill 失败: {}", e),
        },
    }
}

fn close_single(pid: u32, targets: Vec<KillTarget>, force: bool) -> KillResult {
    let result = run_kill(pid, force);
    if !result.success {
        return result;
    }

    let scan_fn = || crate::macos::port_scanner::scan_ports().map_err(|e| e.to_string());
    match wait_until_released(&targets, Duration::from_secs(2), scan_fn) {
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
        if current_ports
            .iter()
            .any(|port| crate::common::target_still_matches(target, port))
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

    let mut results: Vec<KillResult> = std::thread::scope(|s| {
        targets_by_pid
            .into_iter()
            .map(|(pid, targets)| s.spawn(move || close_single(pid, targets, force)))
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
