#!/bin/sh
# 用示例论文生成浏览器预览模式（不在 Tauri 中运行时）使用的假数据：
# 中文界面用 examples/sample-paper，英文界面用 examples/sample-paper-en（放在 fixtures/en/）。
# 数据来自真实的 ym 命令，保证与桌面端返回结构一致。编译结果（PDF）两种语言共用中文示例的。
set -e
cd "$(dirname "$0")/../../../.."
OUT=apps/desktop/ui/src/lib/fixtures

gen() { # gen <示例目录> <输出目录> <语言> <命令…>
  sample=$1; out=$2; lang=$3; shift 3
  mkdir -p "$out"
  for id in "$@"; do
    cargo run -q -p ym-cli -- -C "$sample" --lang "$lang" exec "$id" > "$out/$id.json"
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
' "$out" "$sample"
}

gen examples/sample-paper "$OUT" zh project.info project.tree project.index check.run compile.run compile.pdf
gen examples/sample-paper-en "$OUT/en" en project.info project.tree project.index check.run
rm -f "$OUT/check.run.en.json"
echo "fixtures 已更新：$OUT"
