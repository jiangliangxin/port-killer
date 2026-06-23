# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 环境要求

- Node.js 18+
- pnpm
- Rust (通过 rustup 安装)

## Build Commands

```bash
pnpm install          # 安装依赖
pnpm tauri dev        # 开发模式（热重载，自动打开 DevTools）
pnpm tauri build      # 生产构建
```

构建产物：
- 可执行文件：`src-tauri/target/release/kill-port.exe`

当前发布目标是点击即用的单文件 exe，不生成安装包。

## Architecture

Tauri 2.0 桌面应用，前端 React + 后端 Rust。**仅支持 Windows**。

```
src/                    # React 前端
├── App.tsx             # 主组件，状态管理
├── components/         # UI 组件
├── hooks/usePorts.ts   # 端口扫描 hook
└── types/port.ts       # TypeScript 类型定义

src-tauri/src/          # Rust 后端
├── lib.rs              # Tauri 命令入口
├── port_scanner.rs     # Windows API 端口扫描（TCP/UDP, IPv4/IPv6）
└── process_killer.rs   # 进程终止（taskkill）
```

## Tauri IPC

前端通过 `invoke<T>(command, args)` 调用 Rust 后端：

| 命令 | 参数 | 返回 |
|------|------|------|
| `scan_ports` | - | `PortInfo[]` |
| `kill_process` | `pid: number` | `KillResult` |
| `kill_processes` | `pids: number[]` | `KillResult[]` |

## 类型同步

Rust struct 和 TypeScript interface 必须保持一致：

| Rust (`src-tauri/src/*.rs`) | TypeScript (`src/types/port.ts`) |
|----------------------------|----------------------------------|
| `PortInfo` | `PortInfo` |
| `KillResult` | `KillResult` |

Rust 使用 `#[serde(rename_all = "camelCase")]`，TypeScript 使用 camelCase。

**修改 Rust struct 时，必须同步更新 TypeScript 类型。**

## Windows API

端口扫描使用 `GetExtendedTcpTable` / `GetExtendedUdpTable`：
- TCP: `TCP_TABLE_OWNER_PID_ALL` (5)
- UDP: `UDP_TABLE_OWNER_PID` (1)
- 地址族: `AF_INET` (2, IPv4), `AF_INET6` (23, IPv6)

进程名通过 `CreateToolhelp32Snapshot` 获取。
