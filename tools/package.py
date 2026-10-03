"""把构建产物打包为 Beta 分发压缩包，并做解压自检。

版本号从 Cargo.toml 读取，避免两处维护不同步。
"""

import os
import re
import shutil
import tempfile
import zipfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CARGO_TOML = os.path.join(ROOT, "Cargo.toml")
BUILD_DIR = os.path.join(ROOT, "build")
DIST_DIR = os.path.join(ROOT, "dist")


def read_version() -> str:
    with open(CARGO_TOML, encoding="utf-8") as f:
        content = f.read()
    match = re.search(r'^version\s*=\s*"([^"]+)"', content, re.MULTILINE)
    if not match:
        raise SystemExit("无法从 Cargo.toml 读取 version 字段")
    return match.group(1)


def main() -> None:
    version = read_version()
    folder_name = f"redstone-launcher-{version}"
    src = os.path.join(BUILD_DIR, folder_name)
    if not os.path.isdir(src):
        raise SystemExit(f"待打包目录不存在：{src}")

    os.makedirs(DIST_DIR, exist_ok=True)
    zip_path = os.path.join(DIST_DIR, f"{folder_name}.zip")
    if os.path.exists(zip_path):
        os.remove(zip_path)

    with zipfile.ZipFile(zip_path, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as z:
        for current, _dirs, files in os.walk(src):
            for name in sorted(files):
                full = os.path.join(current, name)
                arcname = os.path.relpath(full, BUILD_DIR)
                z.write(full, arcname)

    size = os.path.getsize(zip_path)
    print(f"版本号：{version}")
    print(f"压缩包：{zip_path}")
    print(f"大小：{size:,} 字节（{size / 1048576:.2f} MB）")
    print("包含文件：")
    with zipfile.ZipFile(zip_path) as z:
        for info in z.infolist():
            print(f"  {info.filename}  ({info.file_size:,} 字节)")

    # 解压自检：确认压缩包能正常解开且文件完好
    tmp = tempfile.mkdtemp(prefix="redstone-verify-")
    try:
        with zipfile.ZipFile(zip_path) as z:
            z.extractall(tmp)
        extracted = os.path.join(tmp, folder_name)
        names = sorted(os.listdir(extracted))
        print(f"解压自检：{extracted}")
        for name in names:
            full = os.path.join(extracted, name)
            print(f"  {name}  ({os.path.getsize(full):,} 字节)")
        required = {"redstone.exe", "redstone.bat", "add-to-path.bat", "README.md"}
        missing = required - set(names)
        if missing:
            raise SystemExit(f"缺少文件：{missing}")
        print("解压自检通过：四个必需文件齐全")
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


if __name__ == "__main__":
    main()
