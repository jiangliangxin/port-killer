use crate::common::PortInfo;
use std::collections::HashMap;
use std::process::Command;

#[derive(Debug, Clone)]
struct ProcessInfo {
    name: String,
    path: String,
    command_line: String,
}

fn process_info_for(map: &HashMap<u32, ProcessInfo>, pid: u32) -> ProcessInfo {
    map.get(&pid).cloned().unwrap_or(ProcessInfo {
        name: "Unknown".to_string(),
        path: String::new(),
        command_line: String::new(),
    })
}

/// 批量获取所有进程的基本信息。
fn get_process_info_map() -> HashMap<u32, ProcessInfo> {
    let mut map = HashMap::new();

    // ps -ax -o pid=,comm=,args=  等号去除标题行
    let output = match Command::new("ps")
        .args(["-ax", "-o", "pid=,comm=,args="])
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => return map,
    };

    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // pid 在最前面，宽度固定，用 splitn 取第一段
        let trimmed = line.trim_start();
        let mut parts = trimmed.splitn(3, |c: char| c.is_whitespace());
        let pid_str = match parts.next() {
            Some(s) => s,
            None => continue,
        };
        let pid: u32 = match pid_str.parse() {
            Ok(p) => p,
            Err(_) => continue,
        };
        let name = parts
            .next()
            .map(|s| {
                // macOS 的 comm 列是完整路径，前端按进程名分组，取 basename 更直观
                s.rsplit('/')
                    .next()
                    .filter(|b| !b.is_empty())
                    .unwrap_or(s)
                    .to_string()
            })
            .unwrap_or_else(|| "Unknown".to_string());
        let command_line = parts.next().unwrap_or("").to_string();
        let path = get_process_path(pid);
        map.insert(
            pid,
            ProcessInfo {
                name,
                path,
                command_line,
            },
        );
    }

    map
}

/// 使用 proc_pidpath 获取进程可执行文件路径。
fn get_process_path(pid: u32) -> String {
    if pid == 0 {
        return String::new();
    }
    unsafe {
        let mut buf = vec![0u8; 4096];
        let size = libc::proc_pidpath(
            pid as i32,
            buf.as_mut_ptr() as *mut libc::c_void,
            buf.len() as u32,
        );
        if size > 0 {
            String::from_utf8_lossy(&buf[..size as usize]).to_string()
        } else {
            String::new()
        }
    }
}

fn lsof_state_to_string(s: &str) -> String {
    match s {
        "CLOSED" => "CLOSED".to_string(),
        "LISTEN" => "LISTEN".to_string(),
        "SYN_SENT" => "SYN_SENT".to_string(),
        "SYN_RCVD" => "SYN_RCVD".to_string(),
        "ESTABLISHED" => "ESTABLISHED".to_string(),
        "FIN_WAIT_1" => "FIN_WAIT1".to_string(),
        "FIN_WAIT_2" => "FIN_WAIT2".to_string(),
        "CLOSE_WAIT" => "CLOSE_WAIT".to_string(),
        "CLOSING" => "CLOSING".to_string(),
        "LAST_ACK" => "LAST_ACK".to_string(),
        "TIME_WAIT" => "TIME_WAIT".to_string(),
        _ => s.to_string(),
    }
}

/// 从 lsof 的 n 字段解析出本地 (host, port)。
///
/// lsof -F n 的 name 字段可能形态：
///   n*:3722                    （监听所有地址）
///   n*:*                       （无端口，丢弃）
///   n127.0.0.1:5335            （IPv4 本地）
///   n[2409:...:5816]:50974     （IPv6 本地）
///   n192.168.1.128:61116->1.2.3.4:443  （连接，含远端，取本地部分）
///
/// 返回 None 表示该记录没有可操作的本地端口（如 UDP *:*）。
fn parse_lsof_name(name: &str) -> Option<(String, u16)> {
    // 去除连接远端部分，只取本地端
    let local = name.split("->").next().unwrap_or(name);

    // 形如 "*:*" 或 "*" —— 无具体端口
    if local == "*:*" || local == "*" {
        return None;
    }

    // 形如 "[ipv6]:port"
    if let Some(bracket_end) = local.find(']') {
        if !local.starts_with('[') {
            return None;
        }
        let addr = &local[1..bracket_end];
        let rest = &local[bracket_end + 1..];
        let port = rest.trim_start_matches(':').trim().parse().ok()?;
        return Some((addr.to_string(), port));
    }

    // 形如 "addr:port" 或 "*:port"
    if let Some(colon_pos) = local.rfind(':') {
        let addr = &local[..colon_pos];
        let port_str = &local[colon_pos + 1..];
        let port: u16 = port_str.parse().ok()?;
        return Some((addr.to_string(), port));
    }

    None
}

/// 解析 lsof -F pcntT 输出，把每条网络记录转成 PortInfo。
///
/// 输出行以单字符标识开头：p=pid, c=command, n=name, t=type, T=TCP state。
/// 一组字段描述一个网络文件，遇到新的 p 字段即为新记录。
fn parse_lsof_output(
    text: &str,
    is_udp: bool,
    process_map: &HashMap<u32, ProcessInfo>,
) -> Vec<PortInfo> {
    // 记录 id 去重：同一 PID 可能对同一端口有多个 FD（如 IPv4+IPv6）
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();

    let mut current_pid: Option<u32> = None;
    let mut current_name: Option<String> = None;
    let mut current_tcp_state: Option<String> = None;

    let flush = |pid: u32,
                 name: Option<String>,
                 is_udp: bool,
                 tcp_state: &Option<String>,
                 process_map: &HashMap<u32, ProcessInfo>|
     -> Option<PortInfo> {
        let name = name?;
        let (addr, port) = parse_lsof_name(&name)?;
        let protocol = if is_udp {
            "UDP".to_string()
        } else {
            "TCP".to_string()
        };
        let state = if is_udp {
            "BOUND".to_string()
        } else {
            tcp_state
                .as_deref()
                .map(lsof_state_to_string)
                .unwrap_or_else(|| "UNKNOWN".to_string())
        };
        let process = process_info_for(process_map, pid);
        let local_address = if addr.contains(':') {
            format!("[{}]:{}", addr, port)
        } else {
            format!("{}:{}", addr, port)
        };
        Some(PortInfo {
            port,
            protocol,
            pid,
            process_name: process.name,
            process_path: process.path,
            command_line: process.command_line,
            state,
            local_address,
        })
    };

    for line in text.lines() {
        if line.is_empty() {
            continue;
        }
        let (field, value) = line.split_at(1);
        match field {
            "p" => {
                if let Some(pid) = current_pid {
                    if let Some(info) = flush(
                        pid,
                        current_name.take(),
                        is_udp,
                        &current_tcp_state,
                        process_map,
                    ) {
                        if seen.insert(format!(
                            "{}|{}|{}|{}",
                            info.protocol, info.local_address, info.pid, info.state
                        )) {
                            result.push(info);
                        }
                    }
                }
                current_pid = value.parse().ok();
                current_name = None;
                current_tcp_state = None;
            }
            "n" => current_name = Some(value.to_string()),
            "t" => {}
            // TCP 状态，形如 "TST=LISTEN"（field 是 T，value 是 "ST=LISTEN"）
            "T" => {
                if let Some(stripped) = value.strip_prefix("ST=") {
                    current_tcp_state = Some(stripped.to_string());
                }
            }
            _ => {}
        }
    }

    // 处理最后一条
    if let Some(pid) = current_pid {
        if let Some(info) = flush(
            pid,
            current_name.take(),
            is_udp,
            &current_tcp_state,
            process_map,
        ) {
            if seen.insert(format!(
                "{}|{}|{}|{}",
                info.protocol, info.local_address, info.pid, info.state
            )) {
                result.push(info);
            }
        }
    }

    result
}

pub fn scan_ports() -> Result<Vec<PortInfo>, Box<dyn std::error::Error>> {
    let process_map = get_process_info_map();
    let mut ports = Vec::new();

    // TCP — 只获取真正监听中的 + 已建立连接的
    let tcp_output = Command::new("lsof")
        .args(["-i", "TCP", "-sTCP:LISTEN", "-F", "pcntT", "-P", "-n"])
        .output()?;
    if tcp_output.status.success() {
        let text = String::from_utf8_lossy(&tcp_output.stdout);
        ports.extend(parse_lsof_output(&text, false, &process_map));
    }

    // UDP
    let udp_output = Command::new("lsof")
        .args(["-i", "UDP", "-F", "pcntT", "-P", "-n"])
        .output()?;
    if udp_output.status.success() {
        let text = String::from_utf8_lossy(&udp_output.stdout);
        ports.extend(parse_lsof_output(&text, true, &process_map));
    }

    ports.sort_by_key(|p| p.port);
    Ok(ports)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proc(name: &str, path: &str, cmdline: &str) -> ProcessInfo {
        ProcessInfo {
            name: name.to_string(),
            path: path.to_string(),
            command_line: cmdline.to_string(),
        }
    }

    #[test]
    fn parse_various_name_formats() {
        assert_eq!(parse_lsof_name("*:8080"), Some(("*".to_string(), 8080)));
        assert_eq!(
            parse_lsof_name("127.0.0.1:5335"),
            Some(("127.0.0.1".to_string(), 5335))
        );
        assert_eq!(
            parse_lsof_name("[::1]:3000"),
            Some(("::1".to_string(), 3000))
        );
        // 连接（含远端），取本地端
        assert_eq!(
            parse_lsof_name("192.168.1.128:61116->8.8.8.8:443"),
            Some(("192.168.1.128".to_string(), 61116))
        );
        // UDP 无端口
        assert_eq!(parse_lsof_name("*:*"), None);
        assert_eq!(parse_lsof_name("*"), None);
        // 无法解析
        assert_eq!(parse_lsof_name("notaport"), None);
    }

    #[test]
    fn tcp_listen_parsed() {
        let mut map = HashMap::new();
        map.insert(475, proc("rapportd", "/usr/libexec/rapportd", "rapportd"));
        let text = "p475\ncrapportd\nf10\ntIPv4\nn*:49167\nTST=LISTEN\nTQR=0\n";
        let ports = parse_lsof_output(text, false, &map);
        assert_eq!(ports.len(), 1);
        assert_eq!(ports[0].port, 49167);
        assert_eq!(ports[0].protocol, "TCP");
        assert_eq!(ports[0].state, "LISTEN");
        assert_eq!(ports[0].pid, 475);
        assert_eq!(ports[0].process_name, "rapportd");
        assert_eq!(ports[0].local_address, "*:49167");
    }

    #[test]
    fn udp_filter_no_port() {
        let text =
            "p473\ncidentityservicesd\nf7\ntIPv4\nn*:*\np475\ncrapportd\nf19\ntIPv4\nn*:3722\n";
        let mut map = HashMap::new();
        map.insert(475, proc("rapportd", "/usr/libexec/rapportd", "rapportd"));
        let ports = parse_lsof_output(text, true, &map);
        // *:* 无端口被过滤，只保留 *:3722
        assert_eq!(ports.len(), 1);
        assert_eq!(ports[0].port, 3722);
        assert_eq!(ports[0].protocol, "UDP");
        assert_eq!(ports[0].state, "BOUND");
    }

    #[test]
    fn dedupe_same_pid_port() {
        // 同一 PID 对同一端口有 IPv4+IPv6 两个 FD，应去重
        let text =
            "p475\ncrapportd\nf10\ntIPv4\nn*:49167\nTST=LISTEN\nf11\ntIPv6\nn*:49167\nTST=LISTEN\n";
        let mut map = HashMap::new();
        map.insert(475, proc("rapportd", "/usr/libexec/rapportd", "rapportd"));
        let ports = parse_lsof_output(text, false, &map);
        assert_eq!(ports.len(), 1);
    }

    #[test]
    fn ipv6_address_formatting() {
        let mut map = HashMap::new();
        map.insert(100, proc("test", "/usr/bin/test", "test"));
        let text = "p100\nctest\nf1\ntIPv6\nn[2409::5816]:50974\nTST=LISTEN\n";
        let ports = parse_lsof_output(text, false, &map);
        assert_eq!(ports.len(), 1);
        assert_eq!(ports[0].port, 50974);
        assert_eq!(ports[0].local_address, "[2409::5816]:50974");
    }
}
