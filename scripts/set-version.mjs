// 把 tag 版本写进三处版本号，避免产物版本和 tag 脱节。
//
// 之前只有 DMG 文件名靠 RELEASE_VERSION 改了一下，App 内置版本、Windows exe
// 名称都还停在 tauri.conf.json 里写死的 0.1.0。
//
// 用法：
//   RELEASE_VERSION=v0.1.5 node scripts/set-version.mjs
//   RELEASE_VERSION=v0.1.5 node scripts/set-version.mjs --dry-run
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const dryRun = process.argv.includes("--dry-run");

const raw = (process.env.RELEASE_VERSION ?? "").trim();
if (!raw) {
  console.log("RELEASE_VERSION 未设置，保持现有版本号不变");
  process.exit(0);
}

const version = raw.replace(/^v/, "");
if (!/^\d+\.\d+\.\d+$/.test(version)) {
  console.error(`无法识别的版本号: ${raw}（期望 v1.2.3 或 1.2.3）`);
  process.exit(1);
}

function updateFile(relPath, transform) {
  const path = join(repoRoot, relPath);
  const before = readFileSync(path, "utf8");
  const after = transform(before);
  if (before === after) {
    console.log(`= ${relPath} 已是 ${version}`);
    return;
  }
  if (!dryRun) writeFileSync(path, after, "utf8");
  console.log(`${dryRun ? "~" : "✓"} ${relPath} -> ${version}`);
}

function replaceJsonVersion(text) {
  const data = JSON.parse(text);
  data.version = version;
  return `${JSON.stringify(data, null, 2)}\n`;
}

function replaceCargoVersion(text) {
  return text.replace(
    /(\[package\][\s\S]*?\nversion\s*=\s*")[^"]+(")/,
    `$1${version}$2`,
  );
}

function replaceLockVersion(text) {
  return text.replace(
    /(\[\[package\]\]\nname = "kill-port"\nversion = ")[^"]+(")/,
    `$1${version}$2`,
  );
}

updateFile("package.json", replaceJsonVersion);
updateFile("src-tauri/Cargo.toml", replaceCargoVersion);
updateFile("src-tauri/Cargo.lock", replaceLockVersion);
updateFile("src-tauri/tauri.conf.json", replaceJsonVersion);

console.log(`版本号已同步为 ${version}${dryRun ? "（dry-run，未写盘）" : ""}`);
