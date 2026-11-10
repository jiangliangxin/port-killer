use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

/// 关闭后复查端口时的轮询间隔。
const POLL_INTERVAL: Duration = Duration::from_millis(150);
/// 端口需要连续空闲这么久才算真正释放（防止刚关闭时的中间态误判）。
const STABLE_FREE_WINDOW: Duration = Duration::from_millis(300);
/// 关闭后等待端口释放的总超时。
pub const RELEASE_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortInfo {
    pub port: u16,
    pub protocol: String,
    pub pid: u32,
    pub process_name: String,
    pub process_path: String,
    pub command_line: String,
    pub state: String,
    pub local_address: String,
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStatus {
    pub elevated: bool,
    pub platform: String,
}

/// Windows: 固定使用 System32 下的系统程序，避免 PATH 被篡改后执行到同名文件。
#[cfg(target_os = "windows")]
pub fn system32_binary(name: &str) -> String {
    std::env::var("SystemRoot")
        .map(|root| format!("{}\\System32\\{}", root, name))
        .unwrap_or_else(|_| name.to_string())
}

/// 系统关键进程黑名单。误杀这些进程会导致桌面会话崩溃、网络中断或整机重启，
/// 因此无论是否以管理员身份运行、是否强制关闭，都直接拒绝。
const PROTECTED_PROCESSES: &[&str] = &[
    // macOS
    "launchd",
    "kernel_task",
    "windowserver",
    "loginwindow",
    "controlcenter",
    "sharingd",
    "mdnsresponder",
    "configd",
    "powerd",
    "securityd",
    // Windows
    "system",
    "system idle process",
    "svchost.exe",
    "lsass.exe",
    "csrss.exe",
    "wininit.exe",
    "winlogon.exe",
    "services.exe",
    "smss.exe",
    "explorer.exe",
    "dwm.exe",
    "fontdrvhost.exe",
    "spoolsv.exe",
];

/// 判断进程是否受保护；返回 Some(原因) 表示拒绝关闭。
pub fn protected_reason(pid: u32, process_name: &str) -> Option<String> {
    if pid == 0 || pid == 4 {
        return Some("系统保留进程".to_string());
    }
    if cfg!(target_os = "macos") && pid == 1 {
        return Some("launchd 是系统初始化进程".to_string());
    }

    let name = process_name.trim().to_ascii_lowercase();
    // macOS 的 comm 可能是完整路径，取最后一段再比对。
    let base = name.rsplit('/').next().unwrap_or(name.as_str());
    if PROTECTED_PROCESSES.contains(&base) {
        return Some(format!("{} 是系统关键进程", process_name.trim()));
    }

    None
}

pub fn target_still_matches(target: &KillTarget, port: &PortInfo) -> bool {
    target.port == port.port
        && target.protocol == port.protocol
        && target.pid == port.pid
        && target.local_address == port.local_address
}

pub fn target_endpoint_matches(target: &KillTarget, port: &PortInfo) -> bool {
    target.port == port.port
        && target.protocol == port.protocol
        && target.local_address == port.local_address
}

pub fn original_targets_still_owned(targets: &[KillTarget], ports: &[PortInfo]) -> bool {
    targets
        .iter()
        .any(|target| ports.iter().any(|port| target_still_matches(target, port)))
}

pub fn target_endpoints_still_occupied(targets: &[KillTarget], ports: &[PortInfo]) -> bool {
    targets.iter().any(|target| {
        ports
            .iter()
            .any(|port| target_endpoint_matches(target, port))
    })
}

pub fn reoccupying_pids(targets: &[KillTarget], ports: &[PortInfo]) -> Vec<u32> {
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

pub fn permission_denied_result(pid: u32) -> KillResult {
    KillResult {
        pid,
        success: false,
        status: "permissionDenied".to_string(),
        message: "权限不足，无法关闭该进程；请以管理员身份重新运行后再试".to_string(),
    }
}

pub fn protected_result(pid: u32, reason: &str) -> KillResult {
    KillResult {
        pid,
        success: false,
        status: "protected".to_string(),
        message: format!("已拒绝关闭：{}", reason),
    }
}

pub fn skipped_result(target: &KillTarget) -> KillResult {
    KillResult {
        pid: target.pid,
        success: false,
        status: "skipped".to_string(),
        message: format!("{} 已不再由该进程占用，已跳过", target.local_address),
    }
}

/// 等待所有目标端口释放，返回最后一次端口快照（供逐个 PID 判定结果）。
///
/// 整批目标只共享一个扫描循环：之前是按 PID 各扫一轮，杀 N 个进程会触发 N 倍的
/// `lsof` / `ps` / PowerShell 调用。
pub fn wait_until_released(
    targets: &[KillTarget],
    timeout: Duration,
    scan_fn: impl Fn() -> Result<Vec<PortInfo>, String>,
) -> Result<Vec<PortInfo>, String> {
    let start = Instant::now();
    let mut free_since: Option<Instant> = None;

    loop {
        let ports = scan_fn()?;

        if !target_endpoints_still_occupied(targets, &ports) {
            let free_start = free_since.get_or_insert_with(Instant::now);
            if free_start.elapsed() >= STABLE_FREE_WINDOW {
                return Ok(ports);
            }
        } else {
            free_since = None;
            // 原进程都不在了，端口已被别的进程接手，结论不会再变。
            if !original_targets_still_owned(targets, &ports) {
                return Ok(ports);
            }
        }

        if start.elapsed() >= timeout {
            return Ok(ports);
        }

        std::thread::sleep(POLL_INTERVAL);
    }
}

/// 依据一份端口快照判定单个 PID 的关闭结果。
pub fn release_result(
    pid: u32,
    targets: &[KillTarget],
    ports: &[PortInfo],
    force: bool,
) -> KillResult {
    if original_targets_still_owned(targets, ports) {
        return KillResult {
            pid,
            success: false,
            status: "notReleased".to_string(),
            message: if force {
                "已执行强制关闭，但端口仍未释放".to_string()
            } else {
                "正常关闭后端口仍未释放，可尝试强制关闭".to_string()
            },
        };
    }

    if target_endpoints_still_occupied(targets, ports) {
        let pids = reoccupying_pids(targets, ports);
        return KillResult {
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
        };
    }

    KillResult {
        pid,
        success: true,
        status: "released".to_string(),
        message: if force {
            "已强制关闭并释放端口".to_string()
        } else {
            "已正常关闭并释放端口".to_string()
        },
    }
}

/// kill 全部执行完之后统一复查一次，把快照映射回每个 PID 的结果。
pub fn verify_released(
    targets_by_pid: &BTreeMap<u32, Vec<KillTarget>>,
    force: bool,
    scan_fn: impl Fn() -> Result<Vec<PortInfo>, String>,
) -> Vec<KillResult> {
    if targets_by_pid.is_empty() {
        return Vec::new();
    }

    let all_targets: Vec<KillTarget> = targets_by_pid.values().flatten().cloned().collect();

    match wait_until_released(&all_targets, RELEASE_TIMEOUT, &scan_fn) {
        Ok(ports) => targets_by_pid
            .iter()
            .map(|(pid, targets)| release_result(*pid, targets, &ports, force))
            .collect(),
        Err(e) => targets_by_pid
            .keys()
            .map(|pid| KillResult {
                pid: *pid,
                success: false,
                status: "failed".to_string(),
                message: format!("已执行关闭，但复查端口失败: {}", e),
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(port: u16, pid: u32, addr: &str) -> KillTarget {
        KillTarget {
            port,
            protocol: "TCP".to_string(),
            pid,
            local_address: addr.to_string(),
        }
    }

    fn port(port: u16, pid: u32, addr: &str) -> PortInfo {
        PortInfo {
            port,
            protocol: "TCP".to_string(),
            pid,
            process_name: "node".to_string(),
            process_path: String::new(),
            command_line: String::new(),
            state: "LISTEN".to_string(),
            local_address: addr.to_string(),
        }
    }

    #[test]
    fn protects_system_processes() {
        assert!(protected_reason(1, "launchd").is_some());
        assert!(protected_reason(0, "").is_some());
        assert!(protected_reason(4, "System").is_some());
        assert!(protected_reason(559, "ControlCenter").is_some());
        assert!(protected_reason(900, "/usr/sbin/mDNSResponder").is_some());
        assert!(protected_reason(1200, "svchost.exe").is_some());
    }

    #[test]
    fn allows_normal_processes() {
        assert!(protected_reason(49528, "node").is_none());
        assert!(protected_reason(883, "WeChat").is_none());
    }

    #[test]
    fn classifies_released() {
        let targets = vec![target(8080, 100, "0.0.0.0:8080")];
        let result = release_result(100, &targets, &[], false);
        assert!(result.success);
        assert_eq!(result.status, "released");
    }

    #[test]
    fn classifies_still_owned_and_reoccupied() {
        let targets = vec![target(8080, 100, "0.0.0.0:8080")];

        let owned = release_result(100, &targets, &[port(8080, 100, "0.0.0.0:8080")], false);
        assert!(!owned.success);
        assert_eq!(owned.status, "notReleased");

        let taken = release_result(100, &targets, &[port(8080, 200, "0.0.0.0:8080")], true);
        assert!(!taken.success);
        assert_eq!(taken.status, "reoccupied");
        assert!(taken.message.contains("200"));
    }

    #[test]
    fn batch_verification_uses_single_snapshot() {
        let mut targets_by_pid = BTreeMap::new();
        targets_by_pid.insert(100, vec![target(8080, 100, "0.0.0.0:8080")]);
        targets_by_pid.insert(200, vec![target(9090, 200, "0.0.0.0:9090")]);

        // 闭包作为 Fn 传入，用 Cell 记录调用次数
        let scans = std::cell::Cell::new(0);
        let results = verify_released(&targets_by_pid, false, || {
            scans.set(scans.get() + 1);
            // 100 已释放，9090 被 300 接手
            Ok(vec![port(9090, 300, "0.0.0.0:9090")])
        });

        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|r| r.pid == 100 && r.success));
        assert!(results
            .iter()
            .any(|r| r.pid == 200 && r.status == "reoccupied"));
        // 两个目标只应触发一轮扫描，而不是每个 PID 各扫一轮
        assert_eq!(scans.get(), 1);
    }
}
