"""一次性补丁：给 java.rs 的探测加入用户级安装目录。"""

import io
import os

PATH = r"D:\数据\Programming\Redstone-Launcher\src\java.rs"

OLD_SCAN = """    for base in VENDOR_DIRS {
        push_vendor_dir(base, &mut list, &mut seen);
    }
"""

NEW_SCAN = """    for base in scan_dirs() {
        push_vendor_dir(&base, &mut list, &mut seen);
    }
"""

HELPER = '''
/// 需要扫描的发行版根目录：系统级 Program Files + 用户级 `%LOCALAPPDATA%\\Programs`。
///
/// 用户级目录是免提权的安装位置（很多软件默认装这里），必须一起扫，
/// 否则装在用户目录里的 JDK 会被漏掉。
fn scan_dirs() -> Vec<String> {
    let mut dirs: Vec<String> = VENDOR_DIRS.iter().map(|s| s.to_string()).collect();
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        for name in [
            "Java",
            "Eclipse Adoptium",
            "Zulu",
            "Microsoft",
            "BellSoft",
            "OpenJDK",
        ] {
            dirs.push(
                PathBuf::from(&local)
                    .join("Programs")
                    .join(name)
                    .to_string_lossy()
                    .to_string(),
            );
        }
    }
    dirs
}

/// 扫描某个发行版根目录，它的每个子目录里可能有 `bin\\java.exe`。
'''


def main() -> None:
    with io.open(PATH, "r", encoding="utf-8") as f:
        text = f.read()

    if OLD_SCAN not in text:
        raise SystemExit("[中止] 没找到待替换的扫描循环，文件可能已改过")
    if "fn scan_dirs()" in text:
        print("[跳过] 补丁已打过")
        return

    text = text.replace(OLD_SCAN, NEW_SCAN)
    text = text.replace(
        "/// 扫描某个发行版根目录，它的每个子目录里可能有 `bin\\java.exe`。\n",
        HELPER,
        1,
    )

    with io.open(PATH, "w", encoding="utf-8", newline="") as f:
        f.write(text)
    print(f"[完成] 已更新 {PATH}")
    print(f"      文件大小 {os.path.getsize(PATH):,} 字节")


if __name__ == "__main__":
    main()
