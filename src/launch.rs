//! 启动命令拼装与进程拉起。

use crate::args::{self, Ctx};
use crate::meta::VersionJson;
use crate::paths;
use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

/// 离线账号：昵称 → 稳定 UUID（uuid5 派生，同名必得同一个 UUID）。
pub fn offline_profile(name: &str) -> (String, String) {
    let uuid = uuid::Uuid::new_v5(
        &uuid::Uuid::NAMESPACE_URL,
        format!("offline:{name}").as_bytes(),
    )
    .to_string();
    let token = format!("OffLineAuth_{}", &uuid.replace('-', "")[..16]);
    (uuid, token)
}

/// 拼出完整 classpath：客户端 jar + 全部非原生依赖库。
pub fn build_classpath(vj: &VersionJson, version: &str) -> String {
    let (libs, _) = crate::install::split_libraries(vj);
    let mut parts: Vec<String> = vec![paths::version_jar(version)
        .to_string_lossy()
        .to_string()];
    for l in libs {
        parts.push(l.to_string_lossy().to_string());
    }
    parts.join(";")
}

/// 拼出完整 java 命令行。
pub fn build_command(
    vj: &VersionJson,
    version: &str,
    java_exe: &Path,
    name: &str,
    memory: &str,
) -> Result<Vec<String>> {
    let (uuid, token) = offline_profile(name);
    let ctx = Ctx {
        natives_dir: paths::natives_dir(version).to_string_lossy().to_string(),
        classpath: build_classpath(vj, version),
        player_name: name.to_string(),
        uuid,
        access_token: token,
        // 每个版本独立实例目录，存档与模组互不串味
        game_dir: paths::instance_dir(version).to_string_lossy().to_string(),
        assets_root: paths::assets_dir().to_string_lossy().to_string(),
        assets_index_name: vj.asset_index.id.clone(),
        version_name: version.to_string(),
        version_type: vj.version_type.clone(),
    };
    let features: HashMap<String, bool> = HashMap::new();

    let mut cmd: Vec<String> = vec![java_exe.to_string_lossy().to_string()];

    match &vj.arguments {
        Some(args) => {
            cmd.push(format!("-Xmx{memory}"));
            cmd.extend(args::eval(&args.jvm, &ctx, &features));
        }
        None => {
            // 老版本：没有 arguments，只有 minecraftArguments
            cmd.push(format!("-Xmx{memory}"));
            cmd.push(format!(
                "-Djava.library.path={}",
                ctx.natives_dir
            ));
            cmd.push("-cp".to_string());
            cmd.push(ctx.classpath.clone());
        }
    }
    // 中文不乱码
    cmd.push("-Dfile.encoding=UTF-8".to_string());
    cmd.push("-Dsun.stdout.encoding=UTF-8".to_string());
    cmd.push("-Dsun.stderr.encoding=UTF-8".to_string());

    cmd.push(vj.main_class.clone());

    match &vj.arguments {
        Some(args) => cmd.extend(args::eval(&args.game, &ctx, &features)),
        None => {
            let legacy = vj
                .minecraft_arguments
                .clone()
                .ok_or_else(|| anyhow!("版本配置既无 arguments 也无 minecraftArguments"))?;
            for token in legacy.split(' ') {
                cmd.push(
                    args::eval(
                        &[crate::meta::ArgElement::Plain(token.to_string())],
                        &ctx,
                        &features,
                    )
                    .first()
                    .cloned()
                    .unwrap_or_default(),
                );
            }
        }
    }

    // 清理空参数（防御性）
    cmd.retain(|c| !c.is_empty());
    Ok(cmd)
}

pub fn print_command(cmd: &[String]) {
    println!("命令行（{} 个参数）：", cmd.len());
    for (i, c) in cmd.iter().enumerate() {
        println!("  [{i:02}] {c}");
    }
}

pub fn launch(cmd: &[String], version: &str) -> Result<i32> {
    if cmd.is_empty() {
        return Err(anyhow!("命令行为空"));
    }
    std::fs::create_dir_all(paths::logs_dir())?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let log_path = paths::logs_dir().join(format!("mcl-{stamp}.log"));

    let mut child = std::process::Command::new(&cmd[0])
        .args(&cmd[1..])
        .current_dir(paths::instance_dir(version))
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .with_context(|| format!("启动失败：{}", cmd[0]))?;

    let out = child.stdout.take().context("无法捕获标准输出")?;
    let err = child.stderr.take().context("无法捕获错误输出")?;
    let log = std::sync::Arc::new(std::sync::Mutex::new(std::fs::File::create(&log_path)?));

    let h_out = pump(out, log.clone(), false);
    let h_err = pump(err, log.clone(), true);
    let _ = h_out.join();
    let _ = h_err.join();

    let status = child.wait()?;
    if let Ok(mut f) = log.lock() {
        let _ = f.flush();
    }
    println!("本次日志已写入：{}", log_path.display());
    Ok(status.code().unwrap_or(-1))
}

/// 把子进程输出同时打到控制台并写入日志文件。
fn pump<R: std::io::Read + Send + 'static>(
    reader: R,
    log: std::sync::Arc<std::sync::Mutex<std::fs::File>>,
    to_stderr: bool,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut r = std::io::BufReader::new(reader);
        let mut line = String::new();
        loop {
            line.clear();
            match r.read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    if to_stderr {
                        eprint!("{line}");
                    } else {
                        print!("{line}");
                    }
                    let _ = std::io::stdout().flush();
                    if let Ok(mut f) = log.lock() {
                        let _ = f.write_all(line.as_bytes());
                    }
                }
            }
        }
    })
}

/// 列出已安装版本及其状态。
pub fn installed_versions() -> Vec<InstalledVersion> {
    let mut list = Vec::new();
    let Ok(entries) = std::fs::read_dir(paths::versions_dir()) else {
        return list;
    };
    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let id = match dir.file_name().and_then(|n| n.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        list.push(InstalledVersion {
            jar: paths::version_jar(&id).exists(),
            natives: paths::natives_dir(&id).exists(),
            instance: paths::instance_dir(&id).exists(),
            saves: count_saves(&id),
            id,
        });
    }
    list.sort_by(|a, b| a.id.cmp(&b.id));
    list
}

pub struct InstalledVersion {
    pub id: String,
    pub jar: bool,
    pub natives: bool,
    pub instance: bool,
    pub saves: usize,
}

fn count_saves(id: &str) -> usize {
    let dir = paths::instance_dir(id).join("saves");
    std::fs::read_dir(dir)
        .map(|e| e.flatten().filter(|p| p.path().is_dir()).count())
        .unwrap_or(0)
}

/// 查看启动器日志（默认最后 40 行）。
pub fn show_logs(lines: usize) -> Result<()> {
    let dir = paths::logs_dir();
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .with_context(|| format!("无法读取日志目录：{}", dir.display()))?
        .flatten()
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("mcl-")
        })
        .map(|e| e.path())
        .collect();
    if files.is_empty() {
        println!("还没有启动器日志（先运行一次 mcl launch）");
        return Ok(());
    }
    files.sort();
    let latest = files.pop().unwrap();
    println!("最新日志：{}", latest.display());
    let content = std::fs::read_to_string(&latest)?;
    let all: Vec<&str> = content.lines().collect();
    let start = all.len().saturating_sub(lines);
    for line in &all[start..] {
        println!("{line}");
    }
    Ok(())
}

/// 校验启动前置条件。
pub fn precheck(vj: &VersionJson, version: &str) -> Result<()> {
    let missing = crate::install::verify(vj, version);
    if !missing.is_empty() {
        return Err(anyhow!(
            "缺少 {} 个文件，请先执行：mcl install {version}",
            missing.len()
        ));
    }
    let natives = paths::natives_dir(version);
    if !natives.exists() {
        return Err(anyhow!("未解压原生库，请先执行：mcl install {version}"));
    }
    std::fs::create_dir_all(paths::instance_dir(version))
        .with_context(|| format!("无法创建实例目录：{}", paths::instance_dir(version).display()))?;
    Ok(())
}

pub fn load_version_json(version: &str) -> Result<VersionJson> {
    let p: PathBuf = paths::version_json(version);
    if !p.exists() {
        return Err(anyhow!(
            "本地没有 {version} 的版本配置，请先执行：mcl install {version}"
        ));
    }
    let s = std::fs::read_to_string(&p)?;
    Ok(serde_json::from_str(&s).context("解析本地版本配置失败")?)
}
