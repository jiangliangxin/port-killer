# Kill Port

Windows 端口占用管理工具

## 环境要求

- Node.js 18+
- pnpm
- **Rust** (必需) - [安装 Rust](https://www.rust-lang.org/tools/install)

安装 Rust (Windows):
```powershell
# 下载并运行 rustup-init.exe
# 或使用 winget
winget install Rustlang.Rustup
```

## 开发

```bash
# 安装依赖
pnpm install

# 启动开发模式
pnpm tauri dev
```

## 构建

```bash
pnpm tauri build
```

构建产物位于 `src-tauri/target/release/bundle/` 目录。