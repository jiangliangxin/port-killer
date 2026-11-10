// 构建后处理脚本：把 Tauri 产物复制到 release/ 目录并生成校验和。
// 跨平台：Windows 生成便携版 exe，macOS 收集 DMG。
//
// 版本号统一由 scripts/set-version.mjs 在构建前写入，脚本这里只负责收集产物。
import { createHash } from "node:crypto";
import {
  cpSync,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const releaseDir = join(repoRoot, "release");
const targetDir = join(repoRoot, "src-tauri", "target");

function sha256(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

function writeHashFile(targetPath, hashFilename) {
  const hash = sha256(targetPath);
  writeFileSync(join(releaseDir, hashFilename), `${hash}  ${targetPath.split(/[\\/]/).pop()}\n`, "utf8");
  console.log(`SHA256: ${hash}`);
}

/// 清空 release/，避免上一次构建的旧产物混进本次发布。
function resetReleaseDir() {
  if (existsSync(releaseDir)) {
    for (const entry of readdirSync(releaseDir)) {
      rmSync(join(releaseDir, entry), { force: true, recursive: true });
    }
  }
  mkdirSync(releaseDir, { recursive: true });
}

/// 按需构建方式（默认 / --target universal-apple-darwin / --target aarch64-...）
/// 产物目录不同，这里把所有可能的位置都找一遍。
function findDmgDir() {
  const candidates = [
    join(targetDir, "release", "bundle", "dmg"),
    join(targetDir, "universal-apple-darwin", "release", "bundle", "dmg"),
  ];

  if (existsSync(targetDir)) {
    for (const entry of readdirSync(targetDir)) {
      candidates.push(join(targetDir, entry, "release", "bundle", "dmg"));
    }
  }

  return candidates.find((dir) => existsSync(dir));
}

function windowsPortable() {
  const sourceExe = join(targetDir, "release", "kill-port.exe");
  if (!statSync(sourceExe, { throwIfNoEntry: false })) {
    console.error("Release exe not found. Run `pnpm tauri build` first.");
    process.exit(1);
  }

  resetReleaseDir();
  const targetExe = join(releaseDir, "port-killer.exe");
  cpSync(sourceExe, targetExe);
  writeHashFile(targetExe, "port-killer.sha256.txt");
  console.log(`Portable exe: ${targetExe}`);
}

function macosDmg() {
  const dmgDir = findDmgDir();
  if (!dmgDir) {
    console.error("DMG not found. Ensure bundle.targets includes \"dmg\" in tauri.conf.json.");
    process.exit(1);
  }

  const dmgs = readdirSync(dmgDir).filter((file) => file.endsWith(".dmg"));
  if (dmgs.length === 0) {
    console.error(`No .dmg under ${dmgDir}`);
    process.exit(1);
  }

  const latest = dmgs
    .map((file) => ({ file, mtime: statSync(join(dmgDir, file)).mtimeMs }))
    .sort((a, b) => a.mtime - b.mtime)
    .pop().file;

  resetReleaseDir();
  const targetDmg = join(releaseDir, latest);
  cpSync(join(dmgDir, latest), targetDmg);
  writeHashFile(targetDmg, `${latest.replace(/\.dmg$/, "")}.sha256.txt`);
  console.log(`DMG: ${targetDmg}`);
}

if (process.platform === "win32") {
  windowsPortable();
} else {
  macosDmg();
}
