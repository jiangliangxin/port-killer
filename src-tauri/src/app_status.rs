use crate::common::AppStatus;
use windows::Win32::UI::Shell::IsUserAnAdmin;

pub fn current_status() -> AppStatus {
    AppStatus {
        // Windows 下端口扫描可普通运行，但关闭高权限进程通常需要管理员权限。
        elevated: unsafe { IsUserAnAdmin().as_bool() },
    }
}
