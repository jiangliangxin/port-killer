use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

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
}

pub enum ReleaseCheckStatus {
    Released,
    StillOwned,
    Reoccupied(Vec<u32>),
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

/// 等待端口释放，返回最终状态。
/// `scan_fn` 是平台特定的端口扫描函数，解耦对 `crate::port_scanner` 的直接依赖。
pub fn wait_until_released(
    targets: &[KillTarget],
    timeout: Duration,
    scan_fn: impl Fn() -> Result<Vec<PortInfo>, String>,
) -> Result<ReleaseCheckStatus, String> {
    let start = Instant::now();
    let stable_free_duration = Duration::from_millis(300);
    let mut free_since: Option<Instant> = None;

    loop {
        let ports = scan_fn()?;
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
