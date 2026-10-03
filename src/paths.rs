//! 路径常量：整个项目唯一的绝对路径来源。
//! 严禁触碰真实游戏目录 %APPDATA%\.minecraft（用户的官方存档）。

use std::path::PathBuf;

/// 从环境变量取一个系统目录根取不到时返回空路径，
/// 而不是在代码里写死某个用户名——这样任何人 clone 后都不必改这一行。
fn system_root(var: &str) -> PathBuf {
    std::env::var(var).map(PathBuf::from).unwrap_or_default()
}

/// 开发根目录：`%LOCALAPPDATA%\MCLauncherDev`。
///
/// 共享区（库、资源、版本本体）与实例区都挂在这里。若想换位置，
/// 改这个函数即可，或让环境变量 `LOCALAPPDATA` 指向别的目录。
pub fn dev_root() -> PathBuf {
    system_root("LOCALAPPDATA").join("MCLauncherDev")
}

/// 随其他应用附带的 Java 根目录：`%USERPROFILE%\.workbuddy\binaries\java`。
///
/// 属于那个应用而不是系统，卸载后即失效，只能作为探测的兜底，
/// 不能当成启动器的长期依赖。
pub fn bundled_java_root() -> PathBuf {
    system_root("USERPROFILE")
        .join(".workbuddy")
        .join("binaries")
        .join("java")
}

pub const LAUNCHER_NAME: &str = "Redstone Launcher";
/// 版本号直接取自 Cargo.toml，避免两处维护不同步
pub const LAUNCHER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// 全局共享区：库、资源、版本本体（多版本共用，避免重复下载）
pub fn game_dir() -> PathBuf {
    dev_root().join("minecraft")
}

/// 实例区根目录：每个版本一个独立实例，存档与模组互不串味
pub fn instances_root() -> PathBuf {
    dev_root().join("instances")
}

/// 某个版本的实例目录，作为 --gameDir 传给游戏
pub fn instance_dir(id: &str) -> PathBuf {
    instances_root().join(id)
}

pub fn versions_dir() -> PathBuf {
    game_dir().join("versions")
}

pub fn version_dir(id: &str) -> PathBuf {
    versions_dir().join(id)
}

pub fn version_json(id: &str) -> PathBuf {
    version_dir(id).join(format!("{id}.json"))
}

pub fn version_jar(id: &str) -> PathBuf {
    version_dir(id).join(format!("{id}.jar"))
}

pub fn natives_dir(id: &str) -> PathBuf {
    version_dir(id).join("natives")
}

pub fn libraries_dir() -> PathBuf {
    game_dir().join("libraries")
}

pub fn assets_dir() -> PathBuf {
    game_dir().join("assets")
}

pub fn indexes_dir() -> PathBuf {
    assets_dir().join("indexes")
}

pub fn objects_dir() -> PathBuf {
    assets_dir().join("objects")
}

pub fn logs_dir() -> PathBuf {
    game_dir().join("logs")
}

pub fn ensure_dirs(id: &str) -> std::io::Result<()> {
    for d in [
        game_dir(),
        versions_dir(),
        version_dir(id),
        libraries_dir(),
        assets_dir(),
        indexes_dir(),
        objects_dir(),
        logs_dir(),
        instances_root(),
        instance_dir(id),
    ] {
        std::fs::create_dir_all(d)?;
    }
    Ok(())
}
