//! 本机 Java 探测探针：把 `src/java.rs` + `src/paths.rs` 单独编译成一个
//! 独立小程序，用来在「产不出 exe」的机器上直接验证 Java 扫描与选版逻辑。
//!
//! 存在的理由：本机 `cargo build` / `cargo test` 会卡在 `windows-sys` 需要的
//! dlltool（见 release-notes\0.9.0-beta.5.md 的已知问题），而 `java.rs` 只用
//! std + anyhow + crate::paths，不碰 Windows API，因此可以绕开依赖链单独编译。
//!
//! 不要用 rustc 直接调，请走 `python tools\probe-java-scan.py`——
//! 它会自动找 anyhow 的 rlib、读版本号、清理产物。
//!
//! 输出内容：探测到的全部 Java（含来源分级）、总数，以及对指定大版本的选版结果。

#[path = "../src/paths.rs"]
mod paths;

#[path = "../src/java.rs"]
mod java;

fn main() {
    println!("=== 本机 Java 探测结果 ===");
    java::print_list();

    let list = java::discover();
    println!();
    println!("共探测到 {} 套 Java", list.len());
    for j in &list {
        let kind = if j.source == java::JavaSource::System {
            "系统级"
        } else {
            "附带级"
        };
        println!("  - Java {:<3} [{kind}] {}", j.major, j.path.display());
    }

    let wanted: i32 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(25);

    println!();
    println!("=== 选版测试：需要 Java {wanted} ===");
    match java::select(wanted) {
        Ok(j) => println!(
            "选中 → Java {}（{}）{}",
            j.major,
            j.version,
            j.path.display()
        ),
        Err(e) => println!("选版失败 → {e:#}"),
    }
}
