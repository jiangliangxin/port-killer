# Kill Port

用 AI 写前端的时候，每次启动 dev server 都会开一个端口。任务一多，端口越积越多，不是忘了关就是端口冲突 — 烦了，所以写了这个。

跨平台端口占用管理工具：扫描端口占用，一键终止进程。支持 Windows 和 macOS。

## 功能

- 扫描 TCP/UDP（IPv4/IPv6）端口占用
- 按端口、PID、进程名等排序和搜索
- 显示进程路径和命令行，便于区分多个同名进程
- 按进程分组选中并批量关闭，关闭前复查 PID 和端口归属
- 识别普通/管理员模式，权限不足时给出明确提示
- 拒绝系统关键进程（launchd / ControlCenter / svchost / lsass 等），避免"全选"误伤整机
- 关闭后复查端口是否释放，能识别端口被新 PID 重新占用的情况
- 自动刷新

## 使用

### Windows

从 [Releases](https://github.com/jiangliangxin/port-killer/releases) 下载 `port-killer.exe` 后双击运行，或自行构建。

便携版依赖系统自带的 WebView2 运行时（Windows 11 与已更新的 Windows 10 默认具备）。

```bash
pnpm install
pnpm release
```

便携版产物为 `release/port-killer.exe`，可直接双击运行；同时会生成 `release/port-killer.sha256.txt` 便于校验。  
如果构建时报"拒绝访问"，先关闭正在运行的 exe 再重新构建。

### macOS

从 [Releases](https://github.com/jiangliangxin/port-killer/releases) 下载 `.dmg` 文件：

1. 双击打开 DMG
2. 将 `Port Killer.app` 拖到 Applications 文件夹
3. 首次打开时，右键点击 App > **打开**，在弹出的对话框中点击"打开"

因为应用未签名（个人/开源项目未购买 Apple Developer 账号），macOS 的 Gatekeeper 会拦截未签名应用。也可以用命令行绕过：

```bash
xattr -cr "/Applications/Port Killer.app"
```

自行构建：

```bash
pnpm install
pnpm release
```

产物为 `release/*.dmg` 及对应 `sha256.txt`。

## 注意

关闭进程会影响该进程占用的所有端口。工具会在关闭前展示进程名、PID 和端口列表，并在执行前重新确认端口归属，但仍建议先确认进程路径和命令行。

普通模式可以扫描端口；关闭系统进程或高权限进程时，可能需要管理员权限（Windows 以管理员运行，macOS 以 root/sudo 运行）。

macOS 上非 root 的 `lsof` 只能看到当前用户的 socket，因此普通模式下列表是**不完整**的：别的用户的端口不会出现，需要以 root 运行才能看到全部。

系统关键进程（macOS 的 launchd、ControlCenter、mDNSResponder 等，Windows 的 svchost、lsass、csrss 等）无论权限如何都会被拒绝关闭。

## 开发

```bash
pnpm install       # 安装前端依赖
pnpm tauri dev     # 启动开发模式（热重载 + DevTools）
pnpm release       # 生成点击即用的安装包（Windows exe / macOS dmg）
```

## 技术栈

- **前端**: React 18 + TypeScript + Vite
- **后端**: Rust + Tauri 2.0
- **Windows**: `GetExtendedTcpTable` / `GetExtendedUdpTable` / `CreateToolhelp32Snapshot` / `taskkill`
- **macOS**: `lsof` / `ps` / `kill` / `proc_pidpath`

## 发布

创建并推送 `v*` tag 后，GitHub Actions 会自动构建 Windows 便携版 exe 和 macOS 通用 DMG（Apple Silicon + Intel）并上传到 GitHub Release：

```bash
git tag v0.1.6
git push origin v0.1.6
```

tag 里的版本号会被 `scripts/set-version.mjs` 写进 `package.json`、`Cargo.toml`、`tauri.conf.json`，
所以产物内置版本、DMG 文件名与 tag 始终一致——不需要手动改三处版本号。

## 许可证

[MIT](LICENSE)