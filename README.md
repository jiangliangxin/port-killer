# Kill Port

用 AI 写前端的时候，每次启动 dev server 都会开一个端口。任务一多，端口越积越多，不是忘了关就是端口冲突 — 烦了，所以写了这个。

Windows 端口占用管理工具，扫描端口占用，一键终止进程。

## 功能

- 扫描 TCP/UDP（IPv4/IPv6）端口占用
- 按端口、PID、进程名等排序和搜索
- 勾选进程并批量终止（`taskkill /F`）
- 自动刷新

## 使用

从 [Releases](https://github.com/jiangxin/kill-port/releases) 下载 `kill-port.exe` 后双击运行，或自行构建：

```bash
pnpm install
pnpm tauri build
```

构建产物为 `src-tauri/target/release/kill-port.exe`。如果构建时报“拒绝访问”，先关闭正在运行的 `kill-port.exe` 再重新构建。

## 开发

```bash
pnpm install       # 安装前端依赖
pnpm tauri dev     # 启动开发模式（热重载 + DevTools）
```

## 技术栈

- **前端**: React 18 + TypeScript + Vite
- **后端**: Rust + Tauri 2.0
- **Windows API**: `GetExtendedTcpTable` / `GetExtendedUdpTable` / `CreateToolhelp32Snapshot`

## 许可证

[MIT](LICENSE)
