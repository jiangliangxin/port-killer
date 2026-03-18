use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{Ipv4Addr, Ipv6Addr};
use windows::Win32::Foundation::ERROR_INSUFFICIENT_BUFFER;
use windows::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, GetExtendedUdpTable, MIB_TCPROW_OWNER_PID,
    MIB_TCPTABLE_OWNER_PID, MIB_UDPROW_OWNER_PID, MIB_UDPTABLE_OWNER_PID,
    TCP_TABLE_CLASS, UDP_TABLE_CLASS,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32First, Process32Next, PROCESSENTRY32, TH32CS_SNAPPROCESS,
};

// AF_INET = 2, AF_INET6 = 23
const AF_INET: u32 = 2;
const AF_INET6: u32 = 23;
// TCP_TABLE_OWNER_PID_ALL = 5
const TCP_TABLE_OWNER_PID_ALL: i32 = 5;
// UDP_TABLE_OWNER_PID = 1
const UDP_TABLE_OWNER_PID: i32 = 1;

/// IPv6 TCP 行结构（Windows API 未提供，手动定义）
#[repr(C)]
#[allow(non_snake_case)]
struct MIB_TCP6ROW_OWNER_PID {
    ucLocalAddr: [u8; 16],
    dwLocalScopeId: u32,
    dwLocalPort: u32,
    ucRemoteAddr: [u8; 16],
    dwRemoteScopeId: u32,
    dwRemotePort: u32,
    dwState: u32,
    dwOwningPid: u32,
}

/// IPv6 TCP 表结构
#[repr(C)]
#[allow(non_snake_case)]
struct MIB_TCP6TABLE_OWNER_PID {
    dwNumEntries: u32,
    table: [MIB_TCP6ROW_OWNER_PID; 1],
}

/// IPv6 UDP 行结构
#[repr(C)]
#[allow(non_snake_case)]
struct MIB_UDP6ROW_OWNER_PID {
    ucLocalAddr: [u8; 16],
    dwLocalScopeId: u32,
    dwLocalPort: u32,
    dwOwningPid: u32,
}

/// IPv6 UDP 表结构
#[repr(C)]
#[allow(non_snake_case)]
struct MIB_UDP6TABLE_OWNER_PID {
    dwNumEntries: u32,
    table: [MIB_UDP6ROW_OWNER_PID; 1],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortInfo {
    pub port: u16,
    pub protocol: String,
    pub pid: u32,
    pub process_name: String,
    pub state: String,
    pub local_address: String,
}

fn tcp_state_to_string(state: u32) -> String {
    match state {
        1 => "CLOSED".to_string(),
        2 => "LISTEN".to_string(),
        3 => "SYN_SENT".to_string(),
        4 => "SYN_RCVD".to_string(),
        5 => "ESTABLISHED".to_string(),
        6 => "FIN_WAIT1".to_string(),
        7 => "FIN_WAIT2".to_string(),
        8 => "CLOSE_WAIT".to_string(),
        9 => "CLOSING".to_string(),
        10 => "LAST_ACK".to_string(),
        11 => "TIME_WAIT".to_string(),
        12 => "DELETE_TCB".to_string(),
        _ => format!("UNKNOWN({})", state),
    }
}

/// 一次性获取所有进程的 PID -> 名称映射
fn get_process_name_map() -> HashMap<u32, String> {
    let mut map = HashMap::new();

    // 添加系统进程
    map.insert(0, "System Idle Process".to_string());
    map.insert(4, "System".to_string());

    unsafe {
        let snapshot = match CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            Ok(h) => h,
            Err(_) => return map,
        };

        let mut entry = PROCESSENTRY32 {
            dwSize: std::mem::size_of::<PROCESSENTRY32>() as u32,
            ..Default::default()
        };

        if Process32First(snapshot, &mut entry).is_ok() {
            loop {
                let bytes: Vec<u8> = entry.szExeFile.iter()
                    .take_while(|&&b| b != 0)
                    .map(|&b| b as u8)
                    .collect();
                let name = String::from_utf8_lossy(&bytes).to_string();
                map.insert(entry.th32ProcessID, name);

                if Process32Next(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }

        let _ = windows::Win32::Foundation::CloseHandle(snapshot);
    }

    map
}

fn get_tcp_ports(process_map: &HashMap<u32, String>) -> Result<Vec<PortInfo>, Box<dyn std::error::Error>> {
    unsafe {
        let mut size: u32 = 0;
        let mut result = Vec::new();

        // 第一次调用获取所需缓冲区大小
        GetExtendedTcpTable(
            None,
            &mut size,
            false,
            AF_INET,
            TCP_TABLE_CLASS(TCP_TABLE_OWNER_PID_ALL),
            0,
        );

        let mut buffer: Vec<u8> = vec![0; size as usize];
        let table_ptr = buffer.as_mut_ptr() as *mut MIB_TCPTABLE_OWNER_PID;

        loop {
            let ret = GetExtendedTcpTable(
                Some(table_ptr as *mut _),
                &mut size,
                false,
                AF_INET,
                TCP_TABLE_CLASS(TCP_TABLE_OWNER_PID_ALL),
                0,
            );

            if ret == 0 {
                break;
            } else if ret == ERROR_INSUFFICIENT_BUFFER.0 {
                buffer.resize(size as usize, 0);
                continue;
            } else {
                return Err(format!("GetExtendedTcpTable failed: {}", ret).into());
            }
        }

        let table = &*table_ptr;
        let row_count = table.dwNumEntries;

        let rows = table.table.as_ptr() as *const MIB_TCPROW_OWNER_PID;
        for i in 0..row_count {
            let row = &*rows.add(i as usize);
            let port = u16::from_be(row.dwLocalPort as u16);
            let ip = Ipv4Addr::from(u32::from_be(row.dwLocalAddr));
            let pid = row.dwOwningPid;
            let state = tcp_state_to_string(row.dwState);

            result.push(PortInfo {
                port,
                protocol: "TCP".to_string(),
                pid,
                process_name: process_map.get(&pid).cloned().unwrap_or_else(|| "Unknown".to_string()),
                state,
                local_address: format!("{}:{}", ip, port),
            });
        }

        Ok(result)
    }
}

fn get_udp_ports(process_map: &HashMap<u32, String>) -> Result<Vec<PortInfo>, Box<dyn std::error::Error>> {
    unsafe {
        let mut size: u32 = 0;
        let mut result = Vec::new();

        // 第一次调用获取所需缓冲区大小
        GetExtendedUdpTable(
            None,
            &mut size,
            false,
            AF_INET,
            UDP_TABLE_CLASS(UDP_TABLE_OWNER_PID),
            0,
        );

        let mut buffer: Vec<u8> = vec![0; size as usize];
        let table_ptr = buffer.as_mut_ptr() as *mut MIB_UDPTABLE_OWNER_PID;

        loop {
            let ret = GetExtendedUdpTable(
                Some(table_ptr as *mut _),
                &mut size,
                false,
                AF_INET,
                UDP_TABLE_CLASS(UDP_TABLE_OWNER_PID),
                0,
            );

            if ret == 0 {
                break;
            } else if ret == ERROR_INSUFFICIENT_BUFFER.0 {
                buffer.resize(size as usize, 0);
                continue;
            } else {
                return Err(format!("GetExtendedUdpTable failed: {}", ret).into());
            }
        }

        let table = &*table_ptr;
        let row_count = table.dwNumEntries;

        let rows = table.table.as_ptr() as *const MIB_UDPROW_OWNER_PID;
        for i in 0..row_count {
            let row = &*rows.add(i as usize);
            let port = u16::from_be(row.dwLocalPort as u16);
            let ip = Ipv4Addr::from(u32::from_be(row.dwLocalAddr));
            let pid = row.dwOwningPid;

            result.push(PortInfo {
                port,
                protocol: "UDP".to_string(),
                pid,
                process_name: process_map.get(&pid).cloned().unwrap_or_else(|| "Unknown".to_string()),
                state: "LISTEN".to_string(),
                local_address: format!("{}:{}", ip, port),
            });
        }

        Ok(result)
    }
}

fn get_tcp6_ports(process_map: &HashMap<u32, String>) -> Result<Vec<PortInfo>, Box<dyn std::error::Error>> {
    unsafe {
        let mut size: u32 = 0;
        let mut result = Vec::new();

        GetExtendedTcpTable(
            None,
            &mut size,
            false,
            AF_INET6,
            TCP_TABLE_CLASS(TCP_TABLE_OWNER_PID_ALL),
            0,
        );

        let mut buffer: Vec<u8> = vec![0; size as usize];
        let table_ptr = buffer.as_mut_ptr() as *mut MIB_TCP6TABLE_OWNER_PID;

        loop {
            let ret = GetExtendedTcpTable(
                Some(table_ptr as *mut _),
                &mut size,
                false,
                AF_INET6,
                TCP_TABLE_CLASS(TCP_TABLE_OWNER_PID_ALL),
                0,
            );

            if ret == 0 {
                break;
            } else if ret == ERROR_INSUFFICIENT_BUFFER.0 {
                buffer.resize(size as usize, 0);
                continue;
            } else {
                return Err(format!("GetExtendedTcpTable (IPv6) failed: {}", ret).into());
            }
        }

        let table = &*table_ptr;
        let row_count = table.dwNumEntries;

        let rows = table.table.as_ptr() as *const MIB_TCP6ROW_OWNER_PID;
        for i in 0..row_count {
            let row = &*rows.add(i as usize);
            let port = u16::from_be(row.dwLocalPort as u16);
            let ip = Ipv6Addr::from(row.ucLocalAddr);
            let pid = row.dwOwningPid;
            let state = tcp_state_to_string(row.dwState);

            result.push(PortInfo {
                port,
                protocol: "TCP6".to_string(),
                pid,
                process_name: process_map.get(&pid).cloned().unwrap_or_else(|| "Unknown".to_string()),
                state,
                local_address: format!("[{}]:{}", ip, port),
            });
        }

        Ok(result)
    }
}

fn get_udp6_ports(process_map: &HashMap<u32, String>) -> Result<Vec<PortInfo>, Box<dyn std::error::Error>> {
    unsafe {
        let mut size: u32 = 0;
        let mut result = Vec::new();

        GetExtendedUdpTable(
            None,
            &mut size,
            false,
            AF_INET6,
            UDP_TABLE_CLASS(UDP_TABLE_OWNER_PID),
            0,
        );

        let mut buffer: Vec<u8> = vec![0; size as usize];
        let table_ptr = buffer.as_mut_ptr() as *mut MIB_UDP6TABLE_OWNER_PID;

        loop {
            let ret = GetExtendedUdpTable(
                Some(table_ptr as *mut _),
                &mut size,
                false,
                AF_INET6,
                UDP_TABLE_CLASS(UDP_TABLE_OWNER_PID),
                0,
            );

            if ret == 0 {
                break;
            } else if ret == ERROR_INSUFFICIENT_BUFFER.0 {
                buffer.resize(size as usize, 0);
                continue;
            } else {
                return Err(format!("GetExtendedUdpTable (IPv6) failed: {}", ret).into());
            }
        }

        let table = &*table_ptr;
        let row_count = table.dwNumEntries;

        let rows = table.table.as_ptr() as *const MIB_UDP6ROW_OWNER_PID;
        for i in 0..row_count {
            let row = &*rows.add(i as usize);
            let port = u16::from_be(row.dwLocalPort as u16);
            let ip = Ipv6Addr::from(row.ucLocalAddr);
            let pid = row.dwOwningPid;

            result.push(PortInfo {
                port,
                protocol: "UDP6".to_string(),
                pid,
                process_name: process_map.get(&pid).cloned().unwrap_or_else(|| "Unknown".to_string()),
                state: "LISTEN".to_string(),
                local_address: format!("[{}]:{}", ip, port),
            });
        }

        Ok(result)
    }
}

pub fn scan_ports() -> Result<Vec<PortInfo>, Box<dyn std::error::Error>> {
    let process_map = get_process_name_map();
    let mut ports = Vec::new();

    ports.extend(get_tcp_ports(&process_map)?);
    ports.extend(get_udp_ports(&process_map)?);
    ports.extend(get_tcp6_ports(&process_map)?);
    ports.extend(get_udp6_ports(&process_map)?);

    // 按端口号排序
    ports.sort_by(|a, b| a.port.cmp(&b.port));

    Ok(ports)
}