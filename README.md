# Kill Port

用 AI 写前端的时候，每次启动 dev server 都会开一个端口。任务一多，端口越积越多，不是忘了关就是端口冲突 — 烦了，所以写了这个。

Windows 端口占用管理工具，扫描端口占用，一键终止进程。仅支持 Windows。

## 功能

- 扫描 TCP/UDP（IPv4/IPv6）端口占用
- 按端口、PID、进程名等排序和搜索
- 显示进程路径和命令行，便于区分多个同名进程
- 按进程分组选中并批量关闭，关闭前复查 PID 和端口归属
- 识别普通/管理员模式，权限不足时给出明确提示
- 关闭后复查端口是否释放，能识别端口被新 PID 重新占用的情况
- 自动刷新

## 使用

从 [Releases](https://github.com/jiangxin/kill-port/releases) 下载 `端口占用管理工具.exe` 后双击运行，或自行构建：

```bash
pnpm install
pnpm release
```

便携版产物为 `release/端口占用管理工具.exe`，可直接双击运行；同时会生成 `release/端口占用管理工具.sha256.txt` 便于校验。  
如果构建时报“拒绝访问”，先关闭正在运行的 exe 再重新构建。

## 注意

关闭进程会影响该进程占用的所有端口。工具会在关闭前展示进程名、PID 和端口列表，并在执行前重新确认端口归属，但仍建议先确认进程路径和命令行。

普通模式可以扫描端口；关闭系统进程或高权限进程时，可能需要以管理员身份运行。

## 开发

```bash
pnpm install       # 安装前端依赖
pnpm tauri dev     # 启动开发模式（热重载 + DevTools）
pnpm release       # 生成点击即用的便携版 exe
```

## 技术栈

- **前端**: React 18 + TypeScript + Vite
- **后端**: Rust + Tauri 2.0
- **Windows API**: `GetExtendedTcpTable` / `GetExtendedUdpTable` / `CreateToolhelp32Snapshot`

## 发布

创建并推送 `v*` tag 后，GitHub Actions 会自动构建便携版 exe 并上传到 GitHub Release：

```bash
git tag v0.1.0
git push origin v0.1.0
```

## 许可证

[MIT](LICENSE)
