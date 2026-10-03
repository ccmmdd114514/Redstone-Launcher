"""一次性补丁：doctor 命令改用带来源标记的 Java 列表，并在只剩附带级时报警。"""

import io

PATH = r"D:\数据\Programming\Redstone-Launcher\src\main.rs"

OLD = """            println!("[Java]");
            let javas = java::discover();
            if javas.is_empty() {
                println!("  未探测到 Java");
            } else {
                for j in &javas {
                    println!("  Java {:<3} {}", j.major, j.path.display());
                }
            }
"""

NEW = """            println!("[Java]");
            let (system_count, bundled_count) = java::summary();
            java::print_list();
            if system_count == 0 && bundled_count > 0 {
                println!(
                    "  警告：没有系统级 Java，当前只能用附带的那套（{}），宿主应用卸载后即失效",
                    paths::BUNDLED_JAVA_ROOT
                );
            }
            if system_count + bundled_count == 0 {
                println!("  未探测到任何 Java，请先安装 Eclipse Temurin");
            }
"""


def main() -> None:
    with io.open(PATH, "r", encoding="utf-8") as f:
        text = f.read()

    if "java::summary()" in text:
        print("[跳过] 已经改过")
        return
    if OLD not in text:
        raise SystemExit("[中止] 没找到 doctor 里的 Java 段落")

    text = text.replace(OLD, NEW)
    with io.open(PATH, "w", encoding="utf-8", newline="") as f:
        f.write(text)
    print("[完成] 已更新 main.rs 的 doctor 输出")


if __name__ == "__main__":
    main()
