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
const BUNDLED_MARKERS: &[&str] = &[
    ".workbuddy",
    "\\workbuddy\\",
    "\\labymod\\",
    "\\.hmcl\\",
    "\\pcl\\",
    "\\bakaxl\\",
    "\\prismlauncher\\",
    "\\multimc\\",
];

/// 第三方 Minecraft 启动器/工具自带的 Java 运行时，位于 `%APPDATA%` 之下。
///
/// 这几套 Java 随宿主应用安装，卸载宿主即失效，一律按附带级处理。
/// 之所以单独列出：新版 Minecraft（26.x 起）要求 Java 25，而玩家机器上
/// 最常见的 Java 25 来源恰恰是这些启动器自带的 runtime，漏扫就直接不可用。
///
/// 注意：**不扫描 `%APPDATA%\.minecraft`**，该目录是玩家的真实游戏数据，
/// 项目硬约束禁止触碰。
const APP_RUNTIME_SUBDIRS: &[&str] = &[
    r"LabyMod\runtime",
    r".hmcl\runtime",
    r"PCL\runtime",
    r"BakaXL\runtime",
    r"PrismLauncher\runtime",
    r"MultiMC\runtime",
];

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
    // 去掉 "-ea" / "-beta" 之类的后缀："25-ea" → "25"
    let core = ver.split('-').next().unwrap_or(&ver);
    // "1.8.0_504" → 8；"21.0.12.1" → 21
    let parts: Vec<&str> = core.split('.').collect();
    if parts.first() == Some(&"1") {
        parts.get(1).and_then(|p| p.parse().ok())
    } else {
        parts.first().and_then(|p| p.parse().ok())
    }
}

/// 把一个候选路径收进列表，自动去重并判定来源。
/// `forced` 用于强制指定来源（第三方启动器运行时一律按附带级处理）。
fn push(
    list: &mut Vec<JavaInstall>,
    seen: &mut HashSet<String>,
    exe: PathBuf,
    forced: Option<JavaSource>,
) {
    let key = exe.to_string_lossy().to_lowercase();
    if !exe.is_file() || !seen.insert(key) {
        return;
    }
    if let Some(major) = probe_major(&exe) {
        let version = probe_version_string(&exe);
        list.push(JavaInstall {
            source: forced.unwrap_or_else(|| classify(&exe)),
            path: exe,
            major,
            version,
        });
    }
}


/// 系统级安装目录：Program Files 下各发行版、用户级免提权安装位置、工具下载的 JDK。
///
/// 用户级目录是免提权的安装位置（很多软件默认装这里），必须一起扫，
/// 否则装在用户目录里的 JDK 会被漏掉。
fn system_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = VENDOR_DIRS.iter().map(PathBuf::from).collect();
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        for name in [
            "Java",
            "Eclipse Adoptium",
            "Zulu",
            "Microsoft",
            "BellSoft",
            "OpenJDK",
        ] {
            dirs.push(PathBuf::from(&local).join("Programs").join(name));
        }
    }
    if let Ok(profile) = std::env::var("USERPROFILE") {
        // IntelliJ 等 IDE 下载的 JDK
        dirs.push(PathBuf::from(&profile).join(".jdks"));
        // scoop 安装的 JDK
        dirs.push(PathBuf::from(&profile).join("scoop").join("apps"));
    }
    dirs
}

/// 第三方启动器自带的运行时根目录（位于 `%APPDATA%`，按附带级处理）。
fn app_runtime_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(roaming) = std::env::var("APPDATA") {
        for sub in APP_RUNTIME_SUBDIRS {
            dirs.push(PathBuf::from(&roaming).join(sub));
        }
    }
    dirs
}

/// 用户自定义的额外扫描目录：环境变量 `REDSTONE_JAVA_DIRS`，多个用分号分隔。
///
/// 用于覆盖扫描白名单之外的安装位置（自定义盘符、绿色版 JDK、其他启动器的
/// 运行时目录等）。
fn custom_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(extra) = std::env::var("REDSTONE_JAVA_DIRS") {
        for p in extra.split(';') {
            let p = p.trim();
            if !p.is_empty() {
                dirs.push(PathBuf::from(p));
            }
        }
    }
    dirs
}

/// 扫描某个发行版根目录，它的每个子目录里可能有 `bin\java.exe`。
fn push_vendor_dir(
    base: &std::path::Path,
    list: &mut Vec<JavaInstall>,
    seen: &mut HashSet<String>,
    forced: Option<JavaSource>,
) {
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
            push(list, seen, direct, forced);
            continue;
        }
        if let Some(found) = find_java_one_level(&dir) {
            push(list, seen, found, forced);
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

    // 1. 系统级安装位置：Program Files 各发行版、用户级安装目录、IDE / scoop 下载的 JDK
    for base in system_dirs() {
        push_vendor_dir(&base, &mut list, &mut seen, None);
    }

    // 2. JAVA_HOME 与 PATH
    if let Ok(home) = std::env::var("JAVA_HOME") {
        push(
            &mut list,
            &mut seen,
            PathBuf::from(home).join("bin").join("java.exe"),
            None,
        );
    }
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            push(&mut list, &mut seen, dir.join("java.exe"), None);
        }
    }

    // 3. 第三方启动器自带的运行时（附带级）。新版 Minecraft（26.x）要求 Java 25，
    //    而玩家机器上最常见的 Java 25 来源正是这类 runtime。
    for base in app_runtime_dirs() {
        push_vendor_dir(&base, &mut list, &mut seen, Some(JavaSource::Bundled));
    }

    // 4. 用户通过 REDSTONE_JAVA_DIRS 指定的额外目录
    for base in custom_dirs() {
        push_vendor_dir(&base, &mut list, &mut seen, None);
    }

    // 5. 兜底：随本工具附带的 Java，卸载宿主后即失效
    push_vendor_dir(
        &paths::bundled_java_root(),
        &mut list,
        &mut seen,
        Some(JavaSource::Bundled),
    );

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
