use crate::common::AppStatus;

pub fn current_status() -> AppStatus {
    // macOS 下，root（uid 0）可以终止任何进程。普通用户只能终止自己的进程。
    AppStatus {
        elevated: unsafe { libc::geteuid() == 0 },
    }
}
