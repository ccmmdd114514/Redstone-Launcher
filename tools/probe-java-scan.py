"""Java 探测探针的编译与运行包装。

用途：本机 `cargo build` / `cargo test` 会卡在 windows-sys 需要的 dlltool
（详见 release-notes\\0.9.0-beta.5.md 的已知问题），导致改了 java.rs 也没法
在本机验证。`java.rs` 只用 std + anyhow + crate::paths，不碰 Windows API，
所以可以脱离依赖链单独编译成一个探针跑起来。

用法：
    python tools\\probe-java-scan.py            # 默认测「需要 Java 25」
    python tools\\probe-java-scan.py 8          # 测「需要 Java 8」
    python tools\\probe-java-scan.py 30         # 测选不到时的报错文案

脚本做的事：
1. 从 Cargo.toml 读版本号（rustc 直接编译时没有 CARGO_PKG_VERSION，得自己喂）
2. 从 .cargo\\config.toml 读 target-dir，在里面找 anyhow 的 rlib
3. rustc 编译 tools\\probe-java-scan.rs，产物落在系统临时目录，跑完即删
"""

from __future__ import annotations

import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PROBE_RS = ROOT / "tools" / "probe-java-scan.rs"
CARGO_TOML = ROOT / "Cargo.toml"
CARGO_CONFIG = ROOT / ".cargo" / "config.toml"


def die(msg: str) -> None:
    print(f"[中止] {msg}")
    sys.exit(1)


def read_version() -> str:
    text = CARGO_TOML.read_text(encoding="utf-8")
    m = re.search(r'^version\s*=\s*"([^"]+)"', text, re.MULTILINE)
    if not m:
        die("无法从 Cargo.toml 读取 version 字段")
    return m.group(1)


def find_target_dir() -> Path:
    """优先读 .cargo\\config.toml 里的 target-dir，取不到就用默认的 target\\。"""
    if CARGO_CONFIG.is_file():
        text = CARGO_CONFIG.read_text(encoding="utf-8")
        m = re.search(r"^\s*target-dir\s*=\s*['\"](.+?)['\"]", text, re.MULTILINE)
        if m:
            return Path(m.group(1).replace("\\\\", "\\"))
    return ROOT / "target"


def find_anyhow_rlib(target_dir: Path) -> Path:
    """在 target-dir 里找 libanyhow-*.rlib。没编译过就先 cargo check 一下。"""
    patterns = ["debug/deps/libanyhow-*.rlib", "release/deps/libanyhow-*.rlib"]
    for pat in patterns:
        hits = sorted(target_dir.glob(pat))
        if hits:
            return hits[-1]
    die(
        f"在 {target_dir} 里找不到 anyhow 的 rlib。\n"
        "  先让 cargo 把依赖拉下来，例如：cargo check --all-targets"
    )


def find_rustc() -> str:
    rustc = shutil.which("rustc")
    if rustc:
        return rustc
    fallback = Path.home() / ".cargo" / "bin" / "rustc.exe"
    if fallback.is_file():
        return str(fallback)
    die("找不到 rustc。把 %USERPROFILE%\\.cargo\\bin 加进 PATH，或用完整路径调用本脚本。")


def main() -> int:
    wanted = sys.argv[1] if len(sys.argv) > 1 else "25"

    version = read_version()
    target_dir = find_target_dir()
    rlib = find_anyhow_rlib(target_dir)
    rustc = find_rustc()

    print(f"版本号：{version}")
    print(f"target-dir：{target_dir}")
    print(f"anyhow：{rlib.name}")
    print(f"rustc：{rustc}")
    print("-" * 60)

    env = dict(os.environ, CARGO_PKG_VERSION=version)
    # 子进程直接写控制台，比 Python 的块缓冲先到；先把上面的信息刷出去，否则顺序错乱
    sys.stdout.flush()
    with tempfile.TemporaryDirectory(prefix="redstone-probe-") as tmp:
        exe = Path(tmp) / "probe-java-scan.exe"
        build = subprocess.run(
            [
                rustc,
                "--edition",
                "2021",
                "--extern",
                f"anyhow={rlib}",
                "-L",
                f"dependency={rlib.parent}",
                "-o",
                str(exe),
                str(PROBE_RS),
            ],
            env=env,
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
        )
        if build.returncode != 0:
            print(build.stderr or build.stdout)
            die("探针编译失败。")
        # 探针里有些函数只在主程序用到，单独编译会报 never used，属正常，静音掉
        errors = [ln for ln in (build.stderr or "").splitlines() if ln.startswith("error")]
        if errors:
            print("\n".join(errors))
            die("探针编译有错误。")

        run = subprocess.run([str(exe), wanted], env=env)
    return run.returncode


if __name__ == "__main__":
    sys.exit(main())
