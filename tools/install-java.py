"""把下载好的 Temurin JDK 解压到安装位置。

来源：清华大学 Adoptium 镜像。
目标：%LOCALAPPDATA%\\Programs\\Eclipse Adoptium\\<jdk 目录>

用用户级目录而不是 C:\\Program Files，是因为往 Program Files 写需要管理员
提权（UAC），命令行静默安装会被 WinError 5 直接拒绝。用户级目录一样是独立
安装、不依赖任何第三方应用，且卸载时直接删目录即可。
"""

import os
import subprocess
import zipfile

TARGET_ROOT = os.path.join(
    os.environ.get("LOCALAPPDATA", ""),
    "Programs",
    "Eclipse Adoptium",
)
CACHE = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), ".cache")

PACKAGES = [
    ("jdk21.zip", None),
    ("jdk8.zip", None),
]


def top_level(zip_path: str) -> str:
    with zipfile.ZipFile(zip_path) as zf:
        names = zf.namelist()
    roots = {n.split("/")[0] for n in names if n}
    if len(roots) != 1:
        raise SystemExit(f"[中止] {zip_path} 顶层目录不止一个：{sorted(roots)}")
    return roots.pop()


def extract(zip_path: str, top: str) -> str:
    dest_dir = os.path.join(TARGET_ROOT, top)
    print(f"解压 {os.path.basename(zip_path)}")
    print(f"  → {dest_dir}")
    with zipfile.ZipFile(zip_path) as zf:
        zf.extractall(TARGET_ROOT)
    return dest_dir


def verify(home: str) -> None:
    exe = os.path.join(home, "bin", "java.exe")
    if not os.path.isfile(exe):
        print(f"  [失败] 没找到 {exe}")
        return
    out = subprocess.run([exe, "-version"], capture_output=True, timeout=30)
    text = (out.stderr or out.stdout).decode("utf-8", "replace")
    for line in text.splitlines():
        if "version" in line:
            print(f"  版本：{line.strip()}")
            break
    print(f"  可用：{exe}")


def main() -> None:
    print(f"目标根目录：{TARGET_ROOT}")
    os.makedirs(TARGET_ROOT, exist_ok=True)

    for filename, _ in PACKAGES:
        zip_path = os.path.join(CACHE, filename)
        if not os.path.isfile(zip_path):
            print(f"[跳过] 下载包不存在：{zip_path}")
            continue
        top = top_level(zip_path)
        home = extract(zip_path, top)
        verify(home)
        print()

    print("--- 安装结果 ---")
    if os.path.isdir(TARGET_ROOT):
        for name in sorted(os.listdir(TARGET_ROOT)):
            print(f"  {name}")


if __name__ == "__main__":
    main()
