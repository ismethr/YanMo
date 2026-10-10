#!/bin/sh
# 用 examples/sample-paper 生成浏览器预览模式（不在 Tauri 中运行时）使用的假数据。
# 数据来自真实的 ym 命令，保证与桌面端返回结构一致。
set -e
cd "$(dirname "$0")/../../../.."
OUT=apps/desktop/ui/src/lib/fixtures
SAMPLE=examples/sample-paper
mkdir -p "$OUT"
for id in project.info project.tree project.index check.run; do
  cargo run -q -p ym-cli -- -C "$SAMPLE" exec "$id" > "$OUT/$id.json"
done
node -e '
const fs = require("fs"), path = require("path");
const [out, sample] = process.argv.slice(1);
// 不把本机绝对路径写进仓库
const infoPath = path.join(out, "project.info.json");
const info = JSON.parse(fs.readFileSync(infoPath, "utf8"));
info.root = sample;
fs.writeFileSync(infoPath, JSON.stringify(info, null, 2) + "\n");
const files = {};
const walk = (rel) => {
  for (const e of fs.readdirSync(path.join(sample, rel), { withFileTypes: true })) {
    if (e.name.startsWith(".")) continue;
    const p = rel ? rel + "/" + e.name : e.name;
    if (e.isDirectory()) walk(p);
    else if (/\.(tex|bib)$/.test(e.name)) files[p] = fs.readFileSync(path.join(sample, p), "utf8");
  }
};
walk("");
fs.writeFileSync(path.join(out, "files.json"), JSON.stringify(files, null, 2) + "\n");
' "$OUT" "$SAMPLE"
echo "fixtures 已更新：$OUT"
