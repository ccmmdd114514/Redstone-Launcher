"""诊断：这台机器上到底装了哪些 Java，分别在哪。

只做只读探测，不改任何东西。用于确认启动器该去哪里找 Java。
"""

import os
import shutil
import subprocess
import winreg

SCAN_DIRS = [
    r"C:\Program Files\Java",
    r"C:\Program Files\Eclipse Adoptium",
    r"C:\Program Files\Microsoft",
    r"C:\Program Files\BellSoft",
    r"C:\Program Files\Zulu",
    r"C:\Program Files\Amazon Corretto",
    r"C:\Program Files\GraalVM",
    r"C:\Program Files\OpenJDK",
    r"C:\Program Files (x86)\Java",
    r"C:\Program Files (x86)\Eclipse Adoptium",
    r"C:\Program Files (x86)\Zulu",
]

REG_PATHS = [
    (winreg.HKEY_LOCAL_MACHINE, r"SOFTWARE\JavaSoft\JDK"),
    (winreg.HKEY_LOCAL_MACHINE, r"SOFTWARE\JavaSoft\JRE"),
    (winreg.HKEY_LOCAL_MACHINE, r"SOFTWARE\WOW6432Node\JavaSoft\JDK"),
    (winreg.HKEY_LOCAL_MACHINE, r"SOFTWARE\WOW6432Node\JavaSoft\JRE"),
]


def version_of(exe: str) -> str:
    try:
        out = subprocess.run(
            [exe, "-version"], capture_output=True, timeout=15
        )
        text = (out.stderr or out.stdout).decode("utf-8", "replace")
        for line in text.splitlines():
            if "version" in line:
                return line.strip()
    except Exception as exc:  # noqa: BLE001
        return f"<读取失败 {exc}>"
    return "<未知>"


def from_registry() -> list:
    found = []
    for hive, path in REG_PATHS:
        try:
            key = winreg.OpenKey(hive, path)
        except OSError:
            continue
        i = 0
        while True:
            try:
                sub = winreg.EnumKey(key, i)
            except OSError:
                break
            i += 1
            try:
                with winreg.OpenKey(key, sub) as k:
                    home = winreg.QueryValueEx(k, "JavaHome")[0]
            except OSError:
                continue
            exe = os.path.join(home, "bin", "java.exe")
            found.append((exe, f"注册表 {path}\\{sub}"))
    return found


def from_dirs() -> list:
    found = []
    for base in SCAN_DIRS:
        if not os.path.isdir(base):
            continue
        for name in sorted(os.listdir(base)):
            exe = os.path.join(base, name, "bin", "java.exe")
            if os.path.isfile(exe):
                found.append((exe, f"目录扫描 {base}"))
            # 有些发行版多一层：jdk-21.0.2\bin\java.exe 或 jdk-21\bin\java.exe
    return found


def from_env() -> list:
    found = []
    home = os.environ.get("JAVA_HOME")
    if home:
        exe = os.path.join(home, "bin", "java.exe")
        found.append((exe, "环境变量 JAVA_HOME"))
    which = shutil.which("java")
    if which:
        found.append((which, "PATH 中找到"))
    return found


def main() -> None:
    print("=" * 72)
    print("系统级 Java 探测")
    print("=" * 72)

    buckets = [
        ("注册表", from_registry()),
        ("Program Files 目录扫描", from_dirs()),
        ("环境变量 / PATH", from_env()),
    ]

    seen = set()
    total = 0
    for title, items in buckets:
        print(f"\n--- {title} ---")
        if not items:
            print("  （无）")
            continue
        for exe, origin in items:
            exe = os.path.normpath(exe)
            exists = os.path.isfile(exe)
            mark = "存在" if exists else "缺失"
            print(f"  [{mark}] {exe}")
            print(f"         来源：{origin}")
            if exists:
                print(f"         版本：{version_of(exe)}")
                seen.add(exe.lower())
                total += 1

    print()
    print("=" * 72)
    print(f"系统级 Java 合计：{total} 个（去重后 {len(seen)} 个）")

    wb = os.path.join(os.environ.get("USERPROFILE", ""), ".workbuddy", "binaries", "java")
    if os.path.isdir(wb):
        names = sorted(os.listdir(wb))
        print(f"\nWorkBuddy 自带（卸载即失效）：{wb}")
        print(f"  共 {len(names)} 个：{', '.join(names)}")


if __name__ == "__main__":
    main()
