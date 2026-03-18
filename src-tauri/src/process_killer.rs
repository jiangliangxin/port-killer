use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KillResult {
    pub pid: u32,
    pub success: bool,
    pub message: String,
}

pub fn kill_process(pid: u32) -> KillResult {
    if pid <= 4 {
        return KillResult {
            pid,
            success: false,
            message: "无法终止系统进程".to_string(),
        };
    }

    let output = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/F"])
        .output();

    match output {
        Ok(result) => {
            if result.status.success() {
                KillResult {
                    pid,
                    success: true,
                    message: "已成功终止".to_string(),
                }
            } else {
                let stderr = String::from_utf8_lossy(&result.stderr);
                KillResult {
                    pid,
                    success: false,
                    message: stderr.trim().to_string(),
                }
            }
        }
        Err(e) => KillResult {
            pid,
            success: false,
            message: format!("执行失败: {}", e),
        },
    }
}