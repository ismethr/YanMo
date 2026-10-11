#!/usr/bin/env python3
"""根据 GitHub Release 中的更新包与 .sig 签名生成完整的 latest.json 并上传。

各平台的构建任务并行运行，各自上传 latest.json 会互相覆盖，所以在全部构建结束后统一生成。

用法：python3 scripts/updater-manifest.py v0.2.0 [--repo owner/name] [--dry-run]
需要已登录的 gh。
"""

import argparse
import json
import subprocess
import sys
import tempfile
from datetime import datetime, timezone
from pathlib import Path

# 更新包文件名 → 更新器查找的平台键（{os}-{arch} 为默认，{os}-{arch}-{安装方式} 为精确匹配）
TARGETS = [
    (lambda n: n.endswith("_aarch64.app.tar.gz"), ["darwin-aarch64", "darwin-aarch64-app"]),
    (lambda n: n.endswith("_x64.app.tar.gz"), ["darwin-x86_64", "darwin-x86_64-app"]),
    (lambda n: n.endswith("_amd64.AppImage"), ["linux-x86_64", "linux-x86_64-appimage"]),
    (lambda n: n.endswith("_amd64.deb"), ["linux-x86_64-deb"]),
    (lambda n: n.endswith(".x86_64.rpm"), ["linux-x86_64-rpm"]),
    (lambda n: n.endswith("_x64-setup.exe"), ["windows-x86_64", "windows-x86_64-nsis"]),
    (lambda n: n.endswith("_x64_en-US.msi"), ["windows-x86_64-msi"]),
]


def gh(*args: str) -> str:
    return subprocess.run(["gh", *args], check=True, capture_output=True, text=True).stdout


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("tag")
    ap.add_argument("--repo", default="ismethr/YanMo")
    ap.add_argument("--dry-run", action="store_true")
    a = ap.parse_args()

    release = json.loads(gh("release", "view", a.tag, "-R", a.repo, "--json", "assets,body"))
    names = {x["name"] for x in release["assets"]}
    platforms = {}
    with tempfile.TemporaryDirectory() as tmp:
        for name in sorted(names):
            keys = next((k for match, k in TARGETS if match(name)), None)
            if not keys:
                continue
            if f"{name}.sig" not in names:
                print(f"缺少签名：{name}.sig", file=sys.stderr)
                return 1
            gh("release", "download", a.tag, "-R", a.repo, "-p", f"{name}.sig", "-D", tmp, "--clobber")
            entry = {
                "signature": (Path(tmp) / f"{name}.sig").read_text().strip(),
                "url": f"https://github.com/{a.repo}/releases/download/{a.tag}/{name}",
            }
            for k in keys:
                platforms[k] = entry
        manifest = {
            "version": a.tag.lstrip("v"),
            "notes": release.get("body") or "",
            "pub_date": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
            "platforms": platforms,
        }
        print(json.dumps({k: v["url"].rsplit("/", 1)[-1] for k, v in platforms.items()}, indent=2))
        if a.dry_run:
            return 0
        out = Path(tmp) / "latest.json"
        out.write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + "\n")
        gh("release", "upload", a.tag, "-R", a.repo, str(out), "--clobber")
    return 0


if __name__ == "__main__":
    sys.exit(main())
