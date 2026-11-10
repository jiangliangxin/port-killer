use crate::common::{
    permission_denied_result, protected_reason, protected_result, skipped_result, system32_binary,
    target_still_matches, verify_released, KillResult, KillTarget, PortInfo,
};
use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::os::windows::process::CommandExt;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};
use windows::Win32::Globalization::{GetOEMCP, MultiByteToWideChar, MULTI_BYTE_TO_WIDE_CHAR_FLAGS};

const CREATE_NO_WINDOW: u32 = 0x08000000;
const TASKKILL_TIMEOUT: Duration = Duration::from_secs(5);

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

/// taskkill 的权限不足提示是固定的几句，不要用 "permission" 这类宽泛子串，
/// 否则任何含该词的输出都会被误判成权限问题。
fn is_permission_denied_message(message: &str) -> bool {
    let lower = message.to_lowercase();
    lower.contains("access is denied")
        || lower.contains("access denied")
        || message.contains("拒绝访问")
        || message.contains("权限不足")
}

/// 只有这些提示才说明"需要用 /F 强制结束"。
fn requires_force_message(message: &str) -> bool {
    let lower = message.to_lowercase();
    lower.contains("forcefully")
        || lower.contains("with /f")
        || message.contains("强制结束")
        || message.contains("只能被强制")
}

fn run_taskkill(pid: u32, force: bool) -> KillResult {
    if pid <= 4 {
        return KillResult {
            pid,
            success: false,
            status: "protected".to_string(),
            message: "已拒绝关闭：系统保留进程".to_string(),
        };
    }

    let pid_arg = pid.to_string();
    let mut command = Command::new(system32_binary("taskkill.exe"));
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
                }

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
            Ok(None) => {
                if start.elapsed() > TASKKILL_TIMEOUT {
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
                };
            }
        }
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

    // 并发执行 taskkill，全部结束后只做一次端口复查。
    let killed: Vec<(u32, KillResult)> = std::thread::scope(|s| {
        targets_by_pid
            .keys()
            .map(|pid| {
                let pid = *pid;
                s.spawn(move || (pid, run_taskkill(pid, force)))
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
        crate::port_scanner::scan_ports().map_err(|e| e.to_string())
    }));
    results
}
