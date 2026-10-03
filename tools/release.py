"""发布打包工具（红石启动器）。

按 RELEASE.md 的发布规范做强制校验，校验通过后才打包，
并把发布说明一并打进包里。任何一项不满足都会拒绝打包并说明原因。

用法：
    python tools/release.py
"""

import os
import re
import shutil
import sys
import tempfile
import zipfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CARGO_TOML = os.path.join(ROOT, "Cargo.toml")
CHANGELOG = os.path.join(ROOT, "CHANGELOG.md")
NOTES_DIR = os.path.join(ROOT, "release-notes")
DIST_DIR = os.path.join(ROOT, "dist")

# 包内必须包含的文件
REQUIRED_FILES = [
    "redstone.exe",
    "redstone.bat",
    "add-to-path.bat",
    "README.md",
    "CHANGELOG.md",
    "RELEASE-NOTES.md",
]

# 版本类型：semver 后缀 -> 中文标记
VERSION_KINDS = [
    ("-alpha", "测试版"),
    ("-beta", "Beta 测试版"),
]


def read_version() -> str:
    with open(CARGO_TOML, encoding="utf-8") as f:
        content = f.read()
    match = re.search(r'^version\s*=\s*"([^"]+)"', content, re.MULTILINE)
    if not match:
        raise SystemExit("无法从 Cargo.toml 读取 version 字段")
    return match.group(1)


def classify(version: str) -> str:
    for suffix, label in VERSION_KINDS:
        if suffix in version:
            return label
    return "正式版"


def fail(reason: str) -> None:
    print(f"[拒绝打包] {reason}")
    print("请参考 RELEASE.md 补齐条件后重新执行。")
    sys.exit(1)


def main() -> None:
    version = read_version()
    kind = classify(version)
    package_name = f"redstone-launcher-{version}"

    print("=" * 60)
    print(f"版本号：{version}")
    print(f"版本类型：{kind}")
    print("=" * 60)

    # 检查 1：主程序是否已构建
    exe = os.path.join(DIST_DIR, "redstone.exe")
    if not os.path.isfile(exe):
        fail(f"缺少主程序 {exe}，请先执行 cargo build --release 并复制到 dist\\")

    # 检查 2：发布说明是否存在且完整
    os.makedirs(NOTES_DIR, exist_ok=True)
    note_src = os.path.join(NOTES_DIR, f"{version}.md")
    if not os.path.isfile(note_src):
        fail(
            f"缺少发布说明：{note_src}\n"
            f"请按 RELEASE.md 第七节的模板创建，内容需包含：版本类型、本次变更、\n"
            f"验证情况、已知问题与局限。"
        )
    with open(note_src, encoding="utf-8") as f:
        note_text = f.read()
    if kind not in note_text:
        fail(f"发布说明中未标注本次的版本类型「{kind}」")
    if "已知问题" not in note_text:
        fail("发布说明缺少「已知问题与局限」章节")
    if "验证情况" not in note_text:
        fail("发布说明缺少「验证情况」章节")

    # 检查 3：更新日志是否同步
    if os.path.isfile(CHANGELOG):
        with open(CHANGELOG, encoding="utf-8") as f:
            changelog_text = f.read()
        if version not in changelog_text:
            fail(f"CHANGELOG.md 中没有 {version} 的条目，请先同步更新日志")
    else:
        fail("缺少 CHANGELOG.md")

    # 收集待打包文件
    files = {
        "redstone.exe": exe,
        "redstone.bat": os.path.join(ROOT, "redstone.bat"),
        "add-to-path.bat": os.path.join(ROOT, "add-to-path.bat"),
        "README.md": os.path.join(ROOT, "README.md"),
        "CHANGELOG.md": CHANGELOG,
        "RELEASE-NOTES.md": note_src,
    }
    for name, path in files.items():
        if not os.path.isfile(path):
            fail(f"缺少待打包文件：{name}（{path}）")

    # 打包
    os.makedirs(DIST_DIR, exist_ok=True)
    zip_path = os.path.join(DIST_DIR, f"{package_name}.zip")
    if os.path.exists(zip_path):
        os.remove(zip_path)
    with zipfile.ZipFile(zip_path, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as z:
        for name, path in files.items():
            z.write(path, os.path.join(package_name, name))

    size = os.path.getsize(zip_path)
    print(f"压缩包：{zip_path}")
    print(f"大小：{size:,} 字节（{size / 1048576:.2f} MB）")
    print("包内文件：")
    with zipfile.ZipFile(zip_path) as z:
        for info in z.infolist():
            print(f"  {info.filename}  ({info.file_size:,} 字节)")

    # 解压自检
    tmp = tempfile.mkdtemp(prefix="redstone-release-")
    try:
        with zipfile.ZipFile(zip_path) as z:
            z.extractall(tmp)
        extracted = os.path.join(tmp, package_name)
        names = sorted(os.listdir(extracted))
        missing = [n for n in REQUIRED_FILES if n not in names]
        if missing:
            fail(f"解压后缺少文件：{missing}")
        print("解压自检通过，必需文件齐全：")
        for name in names:
            full = os.path.join(extracted, name)
            print(f"  {name}  ({os.path.getsize(full):,} 字节)")
    finally:
        shutil.rmtree(tmp, ignore_errors=True)

    print("=" * 60)
    print(f"发布完成：{package_name}（{kind}）")
    print("提示：本包已包含发布说明 RELEASE-NOTES.md，请勿删除 dist 下的历史包。")
    print("=" * 60)


if __name__ == "__main__":
    main()
