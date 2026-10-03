"""一次性补丁：把 paths.rs 里的 JAVA_ROOT 改名为 BUNDLED_JAVA_ROOT。

旧名字暗示它是「本机的 Java」，实际那是别的应用附带的目录，卸载即失效，
改名后语义准确，避免以后又把它当成正式依赖。
"""

import io
import os

PATH = r"D:\数据\Programming\Redstone-Launcher\src\paths.rs"

OLD = 'pub const JAVA_ROOT: &str = r"<USER>\\.workbuddy\\binaries\\java";'
NEW = (
    "/// 随其他应用附带的 Java 根目录。\n"
    "///\n"
    "/// 属于那个应用而不是系统，卸载后即失效，只能作为探测的兜底，\n"
    "/// 不能当成启动器的长期依赖。\n"
    'pub const BUNDLED_JAVA_ROOT: &str = r"<USER>\\.workbuddy\\binaries\\java";'
)


def main() -> None:
    with io.open(PATH, "r", encoding="utf-8") as f:
        text = f.read()

    if "BUNDLED_JAVA_ROOT" in text:
        print("[跳过] 已经改过名")
        return
    if OLD not in text:
        raise SystemExit("[中止] 没找到 JAVA_ROOT 常量")

    text = text.replace(OLD, NEW)
    with io.open(PATH, "w", encoding="utf-8", newline="") as f:
        f.write(text)
    print(f"[完成] 已更新 {PATH}")


if __name__ == "__main__":
    main()
