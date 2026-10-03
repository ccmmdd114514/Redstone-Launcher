"""把版本说明与预编译包发布到 GitHub Releases。

本机没有 gh CLI，本脚本走 GitHub REST API 完成两件事：
1. 以 v<版本号> 为 tag 创建 Release（Beta / Alpha 自动标记为预发布）
2. 若 dist\\redstone-launcher-<版本号>.zip 存在，一并上传为 Asset

用法（PowerShell）：

    $env:GH_TOKEN = "ghp_xxxxxxxx"
    python tools\\publish-github-release.py 0.9.0-beta.1
    python tools\\publish-github-release.py 0.9.0-beta.2

Token 需要 repo 权限（Contents: Read and write）。脚本不会把 token 写进任何文件，
只在当前进程的环境变量里读取。
"""

import json
import os
import sys
import urllib.error
import urllib.request
from pathlib import Path

OWNER = "ccmmdd114514"
REPO = "Redstone-Launcher"
ROOT = Path(__file__).resolve().parent.parent


def get_token() -> str:
    token = os.environ.get("GH_TOKEN", "").strip()
    if not token:
        raise SystemExit(
            "[中止] 未找到 GH_TOKEN 环境变量。\n"
            "  PowerShell 里先执行：$env:GH_TOKEN = \"ghp_你的token\""
        )
    return token


def api(url: str, token: str, payload=None, upload_file: Path | None = None):
    """统一处理 JSON 请求与二进制上传，返回解析后的响应体。"""
    if upload_file is not None:
        data = upload_file.read_bytes()
        target = f"{url}?name={upload_file.name}"
        headers = {
            "Authorization": f"Bearer {token}",
            "Accept": "application/vnd.github+json",
            "X-GitHub-Api-Version": "2022-11-28",
            "Content-Type": "application/octet-stream",
        }
    else:
        data = json.dumps(payload).encode("utf-8")
        target = url
        headers = {
            "Authorization": f"Bearer {token}",
            "Accept": "application/vnd.github+json",
            "X-GitHub-Api-Version": "2022-11-28",
            "Content-Type": "application/json",
        }

    req = urllib.request.Request(target, data=data, headers=headers, method="POST")
    try:
        with urllib.request.urlopen(req, timeout=60) as resp:
            return json.loads(resp.read().decode("utf-8"))
    except urllib.error.HTTPError as exc:
        detail = exc.read().decode("utf-8", errors="replace")
        raise SystemExit(f"[失败] GitHub API 返回 {exc.code}\n{detail}") from exc
    except urllib.error.URLError as exc:
        raise SystemExit(f"[失败] 网络不通：{exc.reason}") from exc


def main() -> None:
    if len(sys.argv) < 2:
        raise SystemExit("[中止] 请指定版本号，例如：python tools\\publish-github-release.py 0.9.0-beta.1")

    version = sys.argv[1].strip()
    token = get_token()

    notes = ROOT / "release-notes" / f"{version}.md"
    if not notes.is_file():
        raise SystemExit(f"[中止] 找不到版本说明：{notes}")
    body = notes.read_text(encoding="utf-8")

    package = ROOT / "dist" / f"redstone-launcher-{version}.zip"
    prerelease = any(tag in version for tag in ("-beta", "-alpha"))

    print(f"[准备] 版本 {version}（{'预发布' if prerelease else '正式版'}）")
    print(f"[准备] 说明 {notes.relative_to(ROOT)}（{len(body)} 字符）")
    if package.is_file():
        print(f"[准备] 预编译包 {package.relative_to(ROOT)}（{package.stat().st_size / 1048576:.2f} MB）")
    else:
        print(f"[注意] 未找到 {package.name}，本次只发源码说明，不带附件")

    release = api(
        f"https://api.github.com/repos/{OWNER}/{REPO}/releases",
        token,
        payload={
            "tag_name": f"v{version}",
            "name": f"红石启动器 {version}（{'Beta 测试版' if '-beta' in version else 'Alpha 测试版' if '-alpha' in version else '正式版'}）",
            "body": body,
            "target_commitish": "main",
            "prerelease": prerelease,
            "draft": False,
        },
    )
    print(f"[完成] Release 已创建：{release['html_url']}")

    if package.is_file():
        uploaded = api(
            f"https://uploads.github.com/repos/{OWNER}/{REPO}/releases/{release['id']}/assets",
            token,
            upload_file=package,
        )
        print(f"[完成] 附件已上传：{uploaded['browser_download_url']}")


if __name__ == "__main__":
    main()
