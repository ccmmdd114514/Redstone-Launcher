//! 本机 Java 探测与选版。
//!
//! 探测顺序（优先级从高到低）：
//! 1. 系统安装位置：`Program Files` 下的各家发行版目录
//! 2. `JAVA_HOME` 环境变量
//! 3. `PATH` 中能直接调用的 java
//! 4. 兜底：随其他应用附带的 Java（`paths::bundled_java_root()`）
//!
//! 兜底那一档标记为 `Bundled`：它属于别的应用，卸载该应用后就会消失，
//! 不能当作启动器的长期依赖。探测结果里有系统级 Java 时优先用系统级。

use crate::paths;
use anyhow::{anyhow, Context, Result};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Java 的来源。决定它能不能作为长期依赖。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JavaSource {
    /// 系统级安装：Program Files 下的发行版、JAVA_HOME、PATH
    System,
    /// 随其他应用附带，卸载该应用即失效
    Bundled,
}

impl JavaSource {
    fn label(&self) -> &'static str {
        match self {
            JavaSource::System => "系统",
            JavaSource::Bundled => "附带",
        }
    }
}

#[derive(Debug, Clone)]
pub struct JavaInstall {
    pub path: PathBuf,
    pub major: i32,
    pub version: String,
    pub source: JavaSource,
}

/// 各家 JDK 在 Windows 上的默认安装根目录。
const VENDOR_DIRS: &[&str] = &[
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
];

/// 出现在这些片段里的路径，一律算作「随其他应用附带」。
const BUNDLED_MARKERS: &[&str] = &[".workbuddy", "\\workbuddy\\"];

fn classify(exe: &Path) -> JavaSource {
    let lower = exe.to_string_lossy().to_lowercase();
    if BUNDLED_MARKERS.iter().any(|m| lower.contains(m)) {
        JavaSource::Bundled
    } else {
        JavaSource::System
    }
}

fn run_version(exe: &Path) -> Option<String> {
    let out = Command::new(exe).arg("-version").output().ok()?;
    let s = String::from_utf8_lossy(&out.stderr).to_string();
    if s.is_empty() {
        Some(String::from_utf8_lossy(&out.stdout).to_string())
    } else {
        Some(s)
    }
}

fn probe_version_string(exe: &Path) -> String {
    let s = run_version(exe).unwrap_or_default();
    for part in s.split('"') {
        if part.contains('.') && part.chars().next().map(|c| c.is_ascii_digit()) == Some(true) {
            return part.to_string();
        }
    }
    s.lines().next().unwrap_or("未知").to_string()
}

fn probe_major(exe: &Path) -> Option<i32> {
    let ver = probe_version_string(exe);
    // "1.8.0_504" → 8；"21.0.12.1" → 21
    let parts: Vec<&str> = ver.split('.').collect();
    if parts.first() == Some(&"1") {
        parts.get(1).and_then(|p| p.parse().ok())
    } else {
        parts.first().and_then(|p| p.parse().ok())
    }
}

/// 把一个候选路径收进列表，自动去重并判定来源。
fn push(list: &mut Vec<JavaInstall>, seen: &mut HashSet<String>, exe: PathBuf) {
    let key = exe.to_string_lossy().to_lowercase();
    if !exe.is_file() || !seen.insert(key) {
        return;
    }
    if let Some(major) = probe_major(&exe) {
        let version = probe_version_string(&exe);
        list.push(JavaInstall {
            source: classify(&exe),
            path: exe,
            major,
            version,
        });
    }
}


/// 需要扫描的发行版根目录：系统级 Program Files + 用户级 `%LOCALAPPDATA%\Programs`。
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

/// 扫描某个发行版根目录，它的每个子目录里可能有 `bin\java.exe`。
fn push_vendor_dir(base: &std::path::Path, list: &mut Vec<JavaInstall>, seen: &mut HashSet<String>) {
    let Ok(entries) = std::fs::read_dir(base) else {
        return;
    };
    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        // 常见两种布局：<root>\jdk-21\bin\java.exe 与 <root>\jdk-21\...\bin\java.exe
        let direct = dir.join("bin").join("java.exe");
        if direct.is_file() {
            push(list, seen, direct);
            continue;
        }
        if let Some(found) = find_java_one_level(&dir) {
            push(list, seen, found);
        }
    }
}

/// 再往下找一层，兼容多套一层目录的发行版。
fn find_java_one_level(dir: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let sub = entry.path();
        if !sub.is_dir() {
            continue;
        }
        let exe = sub.join("bin").join("java.exe");
        if exe.is_file() {
            return Some(exe);
        }
    }
    None
}

/// 探测本机全部 Java。返回的列表按 major 升序，系统级与附带级混在一起，
/// 用 `source` 字段区分。
pub fn discover() -> Vec<JavaInstall> {
    let mut list = Vec::new();
    let mut seen = HashSet::new();

    for base in scan_dirs() {
        push_vendor_dir(std::path::Path::new(&base), &mut list, &mut seen);
    }

    if let Ok(home) = std::env::var("JAVA_HOME") {
        push(&mut list, &mut seen, PathBuf::from(home).join("bin").join("java.exe"));
    }

    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            push(&mut list, &mut seen, dir.join("java.exe"));
        }
    }

    // 兜底：随其他应用附带的 Java，卸载该应用即失效
    push_vendor_dir(&paths::bundled_java_root(), &mut list, &mut seen);

    list.sort_by_key(|j| j.major);
    list
}

fn system_list(list: &[JavaInstall]) -> Vec<&JavaInstall> {
    list.iter().filter(|j| j.source == JavaSource::System).collect()
}

fn bundled_list(list: &[JavaInstall]) -> Vec<&JavaInstall> {
    list.iter().filter(|j| j.source == JavaSource::Bundled).collect()
}

/// 按需要的 majorVersion 选版：系统级精确 → 系统级就近向上 → 附带级（带警告）。
pub fn select(needed: i32) -> Result<JavaInstall> {
    let list = discover();
    if list.is_empty() {
        return Err(anyhow!(
            "本机没有找到任何 Java。请安装 Eclipse Temurin {} 或更高版本后重试。",
            needed
        ));
    }

    let sys = system_list(&list);
    if let Some(j) = sys.iter().find(|j| j.major == needed) {
        return Ok((*j).clone());
    }
    if let Some(j) = sys.iter().find(|j| j.major > needed) {
        println!(
            "提示：未找到 Java {needed}，改用系统级 Java {}（{}）",
            j.major,
            j.path.display()
        );
        return Ok((*j).clone());
    }

    let bundled = bundled_list(&list);
    if let Some(j) = bundled.iter().find(|j| j.major >= needed) {
        warn_bundled(&bundled);
        return Ok((*j).clone());
    }

    let max = list.last().unwrap();
    Err(anyhow!(
        "需要 Java {needed}，本机最高只有 Java {}（{}）。请安装对应版本后再启动。",
        max.major,
        max.path.display()
    ))
}

/// 只剩下附带级 Java 时给出明确警告，避免用户以为一切正常。
fn warn_bundled(bundled: &[&JavaInstall]) {
    println!();
    println!("警告：系统里没有独立的 Java，当前只能用附带的那套（{}）", paths::bundled_java_root().display());
    for j in bundled {
        println!("        Java {}  {}", j.major, j.path.display());
    }
    println!("        这套由其他应用提供，卸载那个应用后启动器会立刻失效。");
    println!("        建议安装 Eclipse Temurin 到 C:\\Program Files\\Eclipse Adoptium。");
    println!();
}

/// 自检用：系统级 Java 的数量，以及是否只能靠附带级。
pub fn summary() -> (usize, usize) {
    let list = discover();
    (system_list(&list).len(), bundled_list(&list).len())
}

pub fn print_list() {
    let list = discover();
    if list.is_empty() {
        println!("未探测到 Java");
        return;
    }
    for j in &list {
        println!(
            "  Java {:<3} [{}] {}  ({})",
            j.major,
            j.source.label(),
            j.path.display(),
            j.version
        );
    }
}

pub fn require(needed: i32) -> Result<JavaInstall> {
    select(needed).context("Java 探测失败")
}
