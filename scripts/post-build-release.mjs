// 构建后处理脚本：把 Tauri 产物复制到 release/ 目录并生成校验和。
// 跨平台：Windows 生成便携版 exe，macOS 收集 DMG。
import { createHash } from "node:crypto";
import { cpSync, mkdirSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const releaseDir = join(repoRoot, "release");

function sha256(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

function writeHashFile(targetPath, hashFilename) {
  const hash = sha256(targetPath);
  writeFileSync(join(releaseDir, hashFilename), `${hash}  ${targetPath.split("/").pop()}\n`, "utf8");
  console.log(`SHA256: ${hash}`);
}

function windowsPortable() {
  const sourceExe = join(repoRoot, "src-tauri", "target", "release", "kill-port.exe");
  if (!statSync(sourceExe, { throwIfNoEntry: false })) {
    console.error("Release exe not found. Run `pnpm tauri build` first.");
    process.exit(1);
  }
  mkdirSync(releaseDir, { recursive: true });
  // release 目录只保留最新便携文件
  for (const entry of readdirSync(releaseDir)) {
    rmSync(join(releaseDir, entry), { force: true, recursive: true });
  }
  const targetExe = join(releaseDir, "port-killer.exe");
  cpSync(sourceExe, targetExe);
  writeHashFile(targetExe, "port-killer.sha256.txt");
  console.log(`Portable exe: ${targetExe}`);
}

function macosDmg() {
  const bundleDir = join(repoRoot, "src-tauri", "target", "release", "bundle");
  const dmgDir = join(bundleDir, "dmg");
  if (!statSync(dmgDir, { throwIfNoEntry: false })) {
    // Tauri 可能在 bundle/macos 下直接产出 .app（当未配置 dmg 时）；这里要求 dmg
    console.error("DMG not found. Ensure bundle.targets includes \"dmg\" in tauri.conf.json.");
    process.exit(1);
  }
  mkdirSync(releaseDir, { recursive: true });
  // 找到最新的 .dmg
  const dmgs = readdirSync(dmgDir).filter((f) => f.endsWith(".dmg")).sort();
  if (dmgs.length === 0) {
    console.error(`No .dmg under ${dmgDir}`);
    process.exit(1);
  }
  const latest = dmgs[dmgs.length - 1];
  const targetDmg = join(releaseDir, latest);
  cpSync(join(dmgDir, latest), targetDmg);
  const base = latest.replace(/\.dmg$/, "");
  writeHashFile(targetDmg, `${base}.sha256.txt`);
  console.log(`DMG: ${targetDmg}`);
}

if (process.platform === "win32") {
  windowsPortable();
} else {
  macosDmg();
}