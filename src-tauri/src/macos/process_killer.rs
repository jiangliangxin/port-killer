use crate::common::{
    permission_denied_result, protected_reason, protected_result, skipped_result,
    target_still_matches, verify_released, KillResult, KillTarget, PortInfo,
};
use std::collections::BTreeMap;
use std::process::{Command, Stdio};

/// 固定使用系统自带的 kill，避免 PATH 被篡改后执行到别的可执行文件。
const KILL_BIN: &str = "/bin/kill";

fn run_kill(pid: u32, force: bool) -> KillResult {
    let mut command = Command::new(KILL_BIN);
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

pub fn kill_processes(
    targets: &[KillTarget],
    current_ports: &[PortInfo],
    force: bool,
) -> Vec<KillResult> {
    let mut targets_by_pid: BTreeMap<u32, Vec<KillTarget>> = BTreeMap::new();
    let mut results: Vec<KillResult> = Vec::new();

    for target in targets {
        // 关闭进程前重新确认端口归属，避免 PID 复用或刷新延迟导致误杀。
        let matched = current_ports
            .iter()
            .find(|port| target_still_matches(target, port));

        let Some(port) = matched else {
            results.push(skipped_result(target));
            continue;
        };

        if let Some(reason) = protected_reason(target.pid, &port.process_name) {
            results.push(protected_result(target.pid, &reason));
            continue;
        }

        targets_by_pid
            .entry(target.pid)
            .or_default()
            .push(target.clone());
    }

    // 并发执行 kill，全部结束后只做一次端口复查。
    let killed: Vec<(u32, KillResult)> = std::thread::scope(|s| {
        targets_by_pid
            .keys()
            .map(|pid| {
                let pid = *pid;
                s.spawn(move || (pid, run_kill(pid, force)))
            })
            // release 构建开启了 panic = "abort"，线程 panic 会直接终止进程，
            // 这里的 expect 只用于表达"join 不会返回 Err"。
            .map(|handle| handle.join().expect("kill thread panicked"))
            .collect()
    });

    let to_verify: BTreeMap<u32, Vec<KillTarget>> = killed
        .iter()
        .filter(|(_, result)| result.success)
        .filter_map(|(pid, _)| {
            targets_by_pid
                .get(pid)
                .map(|targets| (*pid, targets.clone()))
        })
        .collect();

    results.extend(
        killed
            .iter()
            .filter(|(_, result)| !result.success)
            .map(|(_, result)| result.clone()),
    );
    results.extend(verify_released(&to_verify, force, || {
        crate::macos::port_scanner::scan_ports().map_err(|e| e.to_string())
    }));
    results
}
