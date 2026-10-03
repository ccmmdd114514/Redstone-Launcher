mod args;
mod install;
mod java;
mod launch;
mod meta;
mod net;
mod paths;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::io::{self, BufRead, Write};

#[derive(Parser, Debug)]
#[command(
    name = "redstone",
    about = "红石启动器 Redstone Launcher：Minecraft Java 版启动器（命令行版）",
    version,
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// 列出官方版本
    List {
        #[arg(short, long, default_value_t = 10)]
        limit: usize,
        /// release / snapshot / all
        #[arg(short, long, default_value = "release")]
        kind: String,
    },
    /// 查看已安装的版本与实例状态
    Instances,
    /// 下载并安装指定版本
    Install {
        version: String,
        /// official（默认，实测更快）或 bmclapi
        #[arg(long, default_value = "official")]
        mirror: String,
    },
    /// 启动指定版本
    Launch {
        version: String,
        #[arg(long, default_value = "Player")]
        name: String,
        #[arg(long, default_value = "4G")]
        memory: String,
        /// 只打印 java 命令行，不真正启动
        #[arg(long)]
        dry_run: bool,
    },
    /// 删除某个版本的实例（共享的库与资源保留）
    Remove {
        version: String,
        /// 真正执行删除；不加则只预览
        #[arg(long)]
        yes: bool,
    },
    /// 查看启动器日志
    Logs {
        #[arg(long, default_value_t = 40)]
        lines: usize,
    },
    /// 自检：Java、目录、网络
    Doctor,
    /// 列出本机探测到的 Java
    Java,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    // 双击启动（没有任何参数）→ 进入交互模式；带参数的外部命令行照旧。
    if args.len() <= 1 {
        return run_interactive().await;
    }
    let cli = Cli::parse();
    run_command(cli.command).await
}

/// 所有子命令的派发逻辑。外部命令行与交互模式共用，子命令实现本身一行不动。
async fn run_command(command: Command) -> Result<()> {
    match command {
        Command::List { limit, kind } => {
            let http = net::Http::new(net::Mirror::Official)?;
            let manifest: meta::VersionManifest = http
                .json(meta::MANIFEST_URL, "")
                .await
                .context("拉取版本清单失败")?;
            let picked: Vec<&meta::ManifestVersion> = manifest
                .versions
                .iter()
                .filter(|v| kind == "all" || v.kind == kind)
                .take(limit)
                .collect();
            println!("{} 版本（最近 {} 个）：", kind, picked.len());
            for v in picked {
                println!("  {:<20} {}", v.id, v.kind);
            }
        }
        Command::Instances => {
            let list = launch::installed_versions();
            if list.is_empty() {
                println!("还没有安装任何版本。先运行：redstone install 1.21.8");
                return Ok(());
            }
            println!("{:<14} {:<6} {:<8} {:<6} {}", "版本", "本体", "原生库", "存档", "实例目录");
            for v in &list {
                println!(
                    "{:<14} {:<6} {:<8} {:<6} {}",
                    v.id,
                    flag(v.jar),
                    flag(v.natives),
                    v.saves,
                    paths::instance_dir(&v.id).display()
                );
            }
        }
        Command::Remove { version, yes } => {
            let target = paths::instance_dir(&version);
            if !target.exists() {
                println!("实例不存在：{}", target.display());
                return Ok(());
            }
            let saves = launch::installed_versions()
                .into_iter()
                .find(|v| v.id == version)
                .map(|v| v.saves)
                .unwrap_or(0);
            if !yes {
                println!("将要删除实例目录：{}", target.display());
                println!("  内含存档：{} 个", saves);
                println!("  共享的库与资源不会删除，版本本体 {} 也会保留", paths::version_dir(&version).display());
                println!("确认执行请加参数：redstone remove {} --yes", version);
                return Ok(());
            }
            std::fs::remove_dir_all(&target)
                .with_context(|| format!("删除失败（游戏可能正在运行）：{}", target.display()))?;
            println!("已删除实例：{}", target.display());
        }
        Command::Logs { lines } => {
            launch::show_logs(lines)?;
        }
        Command::Doctor => {
            println!("[Java]");
            let (system_count, bundled_count) = java::summary();
            java::print_list();
            if system_count == 0 && bundled_count > 0 {
                println!(
                    "  警告：没有系统级 Java，当前只能用附带的那套（{}），宿主应用卸载后即失效",
                    paths::bundled_java_root().display()
                );
            }
            if system_count + bundled_count == 0 {
                println!("  未探测到任何 Java，请先安装 Eclipse Temurin");
            }
            println!("[目录]");
            for (name, dir) in [
                ("共享区", paths::game_dir()),
                ("实例区", paths::instances_root()),
                ("日志区", paths::logs_dir()),
            ] {
                let ok = dir.exists()
                    && std::fs::File::create(dir.join(".mcl-write-test"))
                        .and_then(|f| -> std::io::Result<()> {
                            f.sync_all()?;
                            Ok(())
                        })
                        .is_ok();
                let _ = std::fs::remove_file(dir.join(".mcl-write-test"));
                println!("  {:<6} {} {}", name, flag(ok), dir.display());
            }
            println!("[已安装]");
            let list = launch::installed_versions();
            println!("  {} 个版本：{}", list.len(), list.iter().map(|v| v.id.as_str()).collect::<Vec<_>>().join(", "));
            println!("[网络]");
            let started = std::time::Instant::now();
            match net::Http::new(net::Mirror::Official) {
                Ok(http) => match http
                    .json::<serde_json::Value>(meta::MANIFEST_URL, "")
                    .await
                {
                    Ok(v) => {
                        let n = v["versions"].as_array().map(|a| a.len()).unwrap_or(0);
                        println!(
                            "  官方清单可达，{} 个版本，耗时 {:?}",
                            n,
                            started.elapsed()
                        );
                    }
                    Err(e) => println!("  官方清单不可达：{e}"),
                },
                Err(e) => println!("  无法初始化网络：{e}"),
            }
        }
        Command::Install { version, mirror } => {
            let http = net::Http::new(net::Mirror::parse(&mirror)?)?;
            install::install(&version, &http).await?;
        }
        Command::Launch {
            version,
            name,
            memory,
            dry_run,
        } => {
            let vj = launch::load_version_json(&version)?;
            launch::precheck(&vj, &version)?;
            let needed = vj
                .java_version
                .as_ref()
                .map(|j| j.major_version)
                .unwrap_or(8);
            let jvm = java::require(needed)?;
            println!("使用 Java {}：{}", jvm.major, jvm.path.display());
            let cmd = launch::build_command(&vj, &version, &jvm.path, &name, &memory)?;
            if dry_run {
                launch::print_command(&cmd);
                return Ok(());
            }
            println!("启动 {}（玩家 {name}）...", version);
            let code = launch::launch(&cmd, &version)?;
            if code == 0 {
                println!("游戏已正常退出（退出码 0）");
            } else {
                eprintln!("游戏退出码 {code}");
            }
        }
        Command::Java => {
            java::print_list();
        }
    }
    Ok(())
}

/// 交互模式：双击 redstone.exe（无参数）进入。黑窗口常驻，键盘敲命令循环执行。
async fn run_interactive() -> Result<()> {
    println!("红石启动器 Redstone Launcher {}", env!("CARGO_PKG_VERSION"));
    println!("交互模式：直接输入命令，例如 `install 1.21.8`、`launch 1.21.8`、`doctor`。");
    println!("输入 `exit` / `quit` / `q` 退出；输入 `help` 查看命令列表。");
    println!("（传统命令行 `redstone <命令>` 依旧可用，详见 README —— 作为彩蛋保留到正式版。）");
    println!();

    let stdin = io::stdin();
    let mut line = String::new();
    loop {
        print!("redstone> ");
        io::stdout().flush().ok();
        line.clear();
        // 读到 EOF（Ctrl+Z / Ctrl+D）退出交互模式
        if stdin.lock().read_line(&mut line)? == 0 {
            println!();
            break;
        }
        let input = line.trim();
        if input.is_empty() {
            continue;
        }
        let tokens = split_args(input);
        match tokens.first().map(String::as_str) {
            Some("exit") | Some("quit") | Some("q") => break,
            Some("help") => {
                println!("可用命令：");
                println!("  list [--limit N] [--kind release|snapshot|all]   列出官方版本");
                println!("  instances                                         查看已安装版本");
                println!("  install <版本> [--mirror official|bmclapi]         下载并安装");
                println!("  launch <版本> [--name 玩家] [--memory 4G] [--dry-run]  启动");
                println!("  remove <版本> [--yes]                              删除实例");
                println!("  logs [--lines N]                                   查看日志");
                println!("  doctor                                            自检 Java / 目录 / 网络");
                println!("  java                                              列出本机探测到的 Java");
                println!("  exit / quit / q                                   退出交互模式");
            }
            _ => {
                let argv = std::iter::once("redstone".to_string()).chain(tokens);
                match Cli::try_parse_from(argv) {
                    Ok(cli) => {
                        if let Err(e) = run_command(cli.command).await {
                            eprintln!("错误：{e:#}");
                        }
                    }
                    // clap 报错自带用法说明，直接打印；不退出交互模式
                    Err(e) => eprintln!("{e}"),
                }
            }
        }
    }
    println!("再见。");
    Ok(())
}

/// 引号感知的参数切分（零依赖）：支持双引号 / 单引号包裹含空格的参数。
/// 例：`launch 1.21.8 --name "My Player"` → ["launch","1.21.8","--name","My Player"]
fn split_args(line: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    for c in line.chars() {
        match c {
            '"' | '\'' if quote.is_none() => quote = Some(c),
            q if Some(q) == quote => quote = None,
            ' ' | '\t' if quote.is_none() => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            _ => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn flag(ok: bool) -> &'static str {
    if ok {
        "是"
    } else {
        "否"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_args_basic() {
        assert_eq!(split_args("install 1.21.8"), vec!["install", "1.21.8"]);
    }

    #[test]
    fn split_args_double_quote() {
        assert_eq!(
            split_args("launch 1.21.8 --name \"My Player\""),
            vec!["launch", "1.21.8", "--name", "My Player"]
        );
    }

    #[test]
    fn split_args_single_quote_groups_spaces() {
        assert_eq!(
            split_args("launch 1.21.8 --name 'Cool Name'"),
            vec!["launch", "1.21.8", "--name", "Cool Name"]
        );
    }

    #[test]
    fn split_args_double_quote_keeps_apostrophe() {
        assert_eq!(
            split_args("launch 1.21.8 --name \"Steve's World\""),
            vec!["launch", "1.21.8", "--name", "Steve's World"]
        );
    }

    #[test]
    fn split_args_collapses_extra_spaces() {
        assert_eq!(split_args("  doctor   "), vec!["doctor"]);
    }

    #[test]
    fn split_args_leading_dash_flag() {
        assert_eq!(
            split_args("install 1.12.2 --mirror bmclapi"),
            vec!["install", "1.12.2", "--mirror", "bmclapi"]
        );
    }
}
