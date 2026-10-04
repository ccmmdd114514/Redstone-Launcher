mod args;
mod install;
mod java;
mod launch;
mod meta;
mod net;
mod paths;
mod ui;

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use std::io::{self, BufRead, Write};

#[derive(Parser, Debug)]
#[command(
    name = "redstone",
    about = "红石启动器 Redstone Launcher：Minecraft Java 版启动器",
    version,
    // 帮助文本全部由下面的 run_help 手写输出，故关闭 clap 自动生成的英文帮助。
    disable_help_flag = true,
    disable_help_subcommand = true,
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

/// 校验版本标识是否安全。
///
/// 版本号会直接参与目录拼接（实例目录、版本目录、日志与锁文件）。未校验时，
/// 绝对路径（`C:\Windows`）会让 `join` 整体替换原路径，`..` 则可穿越到实例区
/// 之外——`remove --yes` 会因此递归删除任意已存在的目录。这里只放行
/// 字母、数字与 `. _ - +`，并显式拒绝 `.` 与 `..`。
fn ensure_safe_version(version: &str) -> Result<()> {
    let ok = !version.is_empty()
        && version.len() <= 64
        && version != "."
        && version != ".."
        && version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '+'));
    if !ok {
        return Err(anyhow!(
            "版本名不合法：{version}（只允许字母、数字与 . _ - +，且不能为 . 或 ..）"
        ));
    }
    Ok(())
}

/// 全部子命令名。用于在交给 clap 之前拦下拼错的命令，给出中文提示
/// （clap 的报错模板是英文的，改不了）。
const KNOWN_COMMANDS: &[&str] = &[
    "list",
    "instances",
    "install",
    "launch",
    "remove",
    "logs",
    "doctor",
    "java",
];

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    ui::init();
    // 自己接住错误：默认的 `fn main() -> Result<()>` 会打印英文前缀「Error:」，
    // 与「全方位汉化」冲突，且退出码不好控制。
    if let Err(e) = run().await {
        eprintln!("{} {e:#}", ui::red("[错误]"));
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    // 帮助请求优先于一切：手写的中文帮助，不走 clap 的英文模板。
    if args.iter().skip(1).any(|a| a == "-h" || a == "--help") || args.get(1).map(|a| a.as_str()) == Some("help") {
        let topic = args
            .iter()
            .skip(1)
            .find(|a| !a.starts_with('-') && a.as_str() != "help")
            .map(|s| s.as_str());
        run_help(topic);
        return Ok(());
    }

    // 双击启动（没有任何参数）→ 进入数字键菜单；带参数的外部命令行照旧。
    if args.len() <= 1 {
        return run_menu().await;
    }

    // 拼错的子命令：给中文提示。以 `-` 开头的留给 clap（-V / --version 之类）。
    if let Some(cmd) = args.get(1) {
        if !cmd.starts_with('-') && !KNOWN_COMMANDS.contains(&cmd.as_str()) {
            return Err(anyhow!(
                "未知命令「{cmd}」。\n  可用命令：{}\n  查看用法：redstone --help",
                KNOWN_COMMANDS.join(" / ")
            ));
        }
    }

    let cli = Cli::parse();
    run_command(cli.command).await
}

/// 手写的中文帮助。`topic` 为子命令名时输出该子命令的说明，否则输出总览。
fn run_help(topic: Option<&str>) {
    match topic {
        Some("list") => {
            println!("列出官方版本");
            println!();
            println!("用法: redstone list [选项]");
            println!("  --limit <N>              最多列出几个，默认 10");
            println!("  --kind <release|snapshot|all>  版本类型，默认 release（正式版）");
        }
        Some("instances") => {
            println!("查看已安装的版本与实例状态");
            println!();
            println!("用法: redstone instances");
        }
        Some("install") => {
            println!("下载并安装指定版本");
            println!();
            println!("用法: redstone install <版本> [选项]");
            println!("  <版本>                   版本号，例如 1.21.8、26.3");
            println!("  --mirror <official|bmclapi>   下载源，默认 official（官方，实测更快）");
        }
        Some("launch") => {
            println!("启动指定版本");
            println!();
            println!("用法: redstone launch <版本> [选项]");
            println!("  <版本>                   要启动的版本号");
            println!("  --name <玩家名>          离线模式下的玩家名，默认 Player");
            println!("  --memory <内存>          最大内存，默认 4G");
            println!("  --dry-run                只打印 java 命令行，不真正启动");
        }
        Some("remove") => {
            println!("删除某个版本的实例（共享的库与资源保留）");
            println!();
            println!("用法: redstone remove <版本> [选项]");
            println!("  <版本>                   要删除的版本号");
            println!("  --yes                    真正执行删除；不加此参数只预览");
        }
        Some("logs") => {
            println!("查看启动器日志");
            println!();
            println!("用法: redstone logs [选项]");
            println!("  --lines <N>              显示最后几行，默认 40");
        }
        Some("doctor") => {
            println!("自检：Java、目录、网络");
            println!();
            println!("用法: redstone doctor");
        }
        Some("java") => {
            println!("列出本机探测到的 Java");
            println!();
            println!("用法: redstone java");
        }
        _ => {
            println!("{}", ui::bold("红石启动器 Redstone Launcher"));
            println!("Minecraft Java 版启动器（命令行内核）");
            println!();
            println!("{}", ui::bold("用法"));
            println!("  redstone                     进入数字键菜单（双击 exe 的等效方式）");
            println!("  redstone <命令> [选项]        直接执行某个命令");
            println!();
            println!("{}", ui::bold("命令"));
            println!("  list       列出官方版本");
            println!("  instances  查看已安装的版本与实例状态");
            println!("  install    下载并安装指定版本");
            println!("  launch     启动指定版本");
            println!("  remove     删除某个版本的实例（共享的库与资源保留）");
            println!("  logs       查看启动器日志");
            println!("  doctor     自检：Java、目录、网络");
            println!("  java       列出本机探测到的 Java");
            println!();
            println!("{}", ui::bold("选项"));
            println!("  -h, --help     显示帮助（可加命令名，如 redstone install --help）");
            println!("  -V, --version  显示版本号");
            println!();
            println!("{}", ui::gray("提示: 不加任何参数直接运行（双击）会进入数字键菜单，按 1-9 即可操作。"));
        }
    }
}

/// 所有子命令的派发逻辑。外部命令行与菜单模式共用，子命令实现本身一行不动。
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
            println!("{} {} 版本（最近 {} 个）：", ui::cyan("◆"), kind, picked.len());
            for v in picked {
                println!("  {:<20} {}", ui::bold(&v.id), ui::gray(&v.kind));
            }
        }
        Command::Instances => {
            let list = launch::installed_versions();
            if list.is_empty() {
                println!("{} 还没有安装任何版本。先安装一个：redstone install 1.21.8", ui::yellow("!"));
                return Ok(());
            }
            println!(
                "{}",
                ui::bold(&format!(
                    "{:<14} {:<6} {:<8} {:<6} {}",
                    "版本", "本体", "原生库", "存档", "实例目录"
                ))
            );
            for v in &list {
                println!(
                    "{:<14} {:<6} {:<8} {:<6} {}",
                    v.id,
                    flag(v.jar),
                    flag(v.natives),
                    v.saves,
                    ui::gray(&paths::instance_dir(&v.id).display().to_string())
                );
            }
        }
        Command::Remove { version, yes } => {
            ensure_safe_version(&version)?;
            let target = paths::instance_dir(&version);
            if !target.exists() {
                println!("{} 实例不存在：{}", ui::yellow("!"), target.display());
                return Ok(());
            }
            let saves = launch::installed_versions()
                .into_iter()
                .find(|v| v.id == version)
                .map(|v| v.saves)
                .unwrap_or(0);
            if !yes {
                println!("将要删除实例目录：{}", ui::yellow(&target.display().to_string()));
                println!("  内含存档：{} 个", saves);
                println!(
                    "  共享的库与资源不会删除，版本本体 {} 也会保留",
                    paths::version_dir(&version).display()
                );
                println!("确认执行请加参数：redstone remove {} --yes", version);
                return Ok(());
            }
            std::fs::remove_dir_all(&target)
                .with_context(|| format!("删除失败（游戏可能正在运行）：{}", target.display()))?;
            println!("{} 已删除实例：{}", ui::green("✓"), target.display());
        }
        Command::Logs { lines } => {
            launch::show_logs(lines)?;
        }
        Command::Doctor => {
            println!("{}", ui::bold("[Java]"));
            let (system_count, bundled_count) = java::summary();
            java::print_list();
            if system_count == 0 && bundled_count > 0 {
                println!(
                    "  {} 没有系统级 Java，当前只能用附带的那套（{}），宿主应用卸载后即失效",
                    ui::yellow("警告："),
                    paths::bundled_java_root().display()
                );
            }
            if system_count + bundled_count == 0 {
                println!("  未探测到任何 Java，请先安装 Eclipse Temurin");
            }
            println!("{}", ui::bold("[目录]"));
            for (name, dir) in [
                ("共享区", paths::game_dir()),
                ("实例区", paths::instances_root()),
                ("日志区", paths::logs_dir()),
            ] {
                let ok = dir.exists()
                    && std::fs::File::create(dir.join(".redstone-write-test"))
                        .and_then(|f| -> std::io::Result<()> {
                            f.sync_all()?;
                            Ok(())
                        })
                        .is_ok();
                let _ = std::fs::remove_file(dir.join(".redstone-write-test"));
                println!(
                    "  {:<6} {} {}",
                    name,
                    colored_flag(ok),
                    ui::gray(&dir.display().to_string())
                );
            }
            println!("{}", ui::bold("[已安装]"));
            let list = launch::installed_versions();
            println!(
                "  {} 个版本：{}",
                list.len(),
                list.iter().map(|v| v.id.as_str()).collect::<Vec<_>>().join(", ")
            );
            println!("{}", ui::bold("[网络]"));
            let started = std::time::Instant::now();
            match net::Http::new(net::Mirror::Official) {
                Ok(http) => match http.json::<serde_json::Value>(meta::MANIFEST_URL, "").await {
                    Ok(v) => {
                        let n = v["versions"].as_array().map(|a| a.len()).unwrap_or(0);
                        println!(
                            "  {} 官方清单可达，{} 个版本，耗时 {:?}",
                            ui::green("✓"),
                            n,
                            started.elapsed()
                        );
                    }
                    Err(e) => println!("  {} 官方清单不可达：{e}", ui::red("✗")),
                },
                Err(e) => println!("  {} 无法初始化网络：{e}", ui::red("✗")),
            }
        }
        Command::Install { version, mirror } => {
            ensure_safe_version(&version)?;
            let http = net::Http::new(net::Mirror::parse(&mirror)?)?;
            install::install(&version, &http).await?;
        }
        Command::Launch {
            version,
            name,
            memory,
            dry_run,
        } => {
            ensure_safe_version(&version)?;
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
                println!("{} 游戏已正常退出（退出码 0）", ui::green("✓"));
            } else {
                eprintln!("{} 游戏退出码 {code}", ui::red("✗"));
            }
        }
        Command::Java => {
            java::print_list();
        }
    }
    Ok(())
}

/// 主菜单：数字键 1-9 直选，按下即执行，不需要敲命令、不需要回车。
async fn run_menu() -> Result<()> {
    println!("{}", ui::banner());
    loop {
        println!("  {}", ui::bold("—— 主菜单 ——"));
        let items = [
            ("1", "列出官方版本"),
            ("2", "查看已安装版本"),
            ("3", "安装版本"),
            ("4", "启动游戏"),
            ("5", "查看启动日志"),
            ("6", "自检（Java / 目录 / 网络）"),
            ("7", "查看本机 Java"),
            ("8", "删除实例"),
            ("9", "退出"),
        ];
        for (key, label) in items {
            println!("    {}  {}", ui::bold_red(key), label);
        }
        println!("    {}  {}", ui::gray("0"), ui::gray("命令行模式（高级用法）"));
        print!("\n  {} ", ui::bold("请按数字键选择（0-9）："));
        io::stdout().flush().ok();

        let key = match ui::read_key() {
            Some(c) => c,
            None => break,
        };
        println!("{}", ui::bold_red(&key.to_string()));
        println!();

        let result = match key {
            '1' => {
                run_command(Command::List {
                    limit: 10,
                    kind: "release".into(),
                })
                .await
            }
            '2' => run_command(Command::Instances).await,
            '3' => match pick_remote_version().await {
                Ok(Some(v)) => {
                    run_command(Command::Install {
                        version: v,
                        mirror: "official".into(),
                    })
                    .await
                }
                Ok(None) => Ok(()),
                Err(e) => Err(e),
            },
            '4' => match pick_installed_version() {
                Some(v) => match ask_player_name() {
                    Some(name) => {
                        run_command(Command::Launch {
                            version: v,
                            name,
                            memory: "4G".into(),
                            dry_run: false,
                        })
                        .await
                    }
                    None => Ok(()),
                },
                None => Ok(()),
            },
            '5' => run_command(Command::Logs { lines: 40 }).await,
            '6' => run_command(Command::Doctor).await,
            '7' => run_command(Command::Java).await,
            '8' => match pick_installed_version() {
                Some(v) => {
                    if confirm(&format!("确认删除实例 {v}？（实例目录内的存档会一并删除）")) {
                        run_command(Command::Remove { version: v, yes: true }).await
                    } else {
                        println!("  已取消。");
                        Ok(())
                    }
                }
                None => Ok(()),
            },
            '0' => return run_interactive().await,
            '9' | 'q' | 'Q' => break,
            _ => {
                println!("  {} 无效按键，请按 0-9。", ui::yellow("!"));
                Ok(())
            }
        };

        if let Err(e) = result {
            eprintln!("  {} {e:#}", ui::red("[错误]"));
        }
        println!();
        pause();
    }
    println!("{}", ui::gray("再见。"));
    Ok(())
}

/// 等一个按键，让用户看清输出后再回主菜单。
fn pause() {
    print!("  {}", ui::gray("按任意键返回主菜单..."));
    io::stdout().flush().ok();
    let _ = ui::read_key();
    println!();
}

/// 让用户在列表里按数字选一项，返回下标。按 0 或非法键返回 None。
fn choose(items: &[String], title: &str) -> Option<usize> {
    if items.is_empty() {
        println!("  {}", ui::yellow("（没有可选项）"));
        return None;
    }
    println!("  {}", ui::bold(title));
    for (i, item) in items.iter().take(9).enumerate() {
        println!("    {}  {}", ui::bold_red(&(i + 1).to_string()), item);
    }
    println!("    {}  {}", ui::gray("0"), ui::gray("返回"));
    print!("  请按数字键选择：");
    io::stdout().flush().ok();
    let key = ui::read_key()?;
    println!("{}", ui::bold_red(&key.to_string()));
    let n = key.to_digit(10)? as usize;
    if n == 0 || n > items.len().min(9) {
        None
    } else {
        Some(n - 1)
    }
}

/// 单键确认（y / n）。
fn confirm(question: &str) -> bool {
    print!("  {} {} ", ui::yellow("?"), question);
    print!("[y/N] ");
    io::stdout().flush().ok();
    match ui::read_key() {
        Some('y') | Some('Y') => {
            println!("y");
            true
        }
        _ => {
            println!("n");
            false
        }
    }
}

/// 询问玩家名。直接回车用默认值 Player。
fn ask_player_name() -> Option<String> {
    print!("  玩家名（直接回车用默认值 Player）：");
    io::stdout().flush().ok();
    let mut line = String::new();
    if io::stdin().lock().read_line(&mut line).is_err() {
        return None;
    }
    let name = line.trim();
    if name.is_empty() {
        Some("Player".to_string())
    } else {
        Some(name.to_string())
    }
}

/// 拉取官方版本清单，列出最新的 9 个正式版供选择。
async fn pick_remote_version() -> Result<Option<String>> {
    let http = net::Http::new(net::Mirror::Official)?;
    let manifest: meta::VersionManifest = http
        .json(meta::MANIFEST_URL, "")
        .await
        .context("拉取版本清单失败")?;
    let list: Vec<String> = manifest
        .versions
        .iter()
        .filter(|v| v.kind == "release")
        .take(9)
        .map(|v| v.id.clone())
        .collect();
    if list.is_empty() {
        println!("  {}", ui::yellow("清单里没有正式版。"));
        return Ok(None);
    }
    Ok(choose(&list, "可安装的正式版（最新 9 个）").map(|i| list[i].clone()))
}

/// 列出已安装的版本供选择。
fn pick_installed_version() -> Option<String> {
    let list: Vec<String> = launch::installed_versions()
        .into_iter()
        .map(|v| v.id)
        .collect();
    if list.is_empty() {
        println!(
            "  {} 还没有安装任何版本，先用主菜单的「3 安装版本」。",
            ui::yellow("!")
        );
        return None;
    }
    choose(&list, "已安装的版本").map(|i| list[i].clone())
}

/// 命令行模式：手敲命令循环执行（主菜单按 0 进入）。双击场景下的高级用法。
async fn run_interactive() -> Result<()> {
    println!("{}", ui::bold("命令行模式"));
    println!("直接输入命令，例如 install 1.21.8、launch 1.21.8、doctor。");
    println!("输入 {} / {} / {} 退出并回到主菜单；输入 {} 查看命令列表。",
        ui::bold("exit"), ui::bold("quit"), ui::bold("q"), ui::bold("help"));
    println!();

    let stdin = io::stdin();
    let mut line = String::new();
    loop {
        print!("{}", ui::bold_red("redstone> "));
        io::stdout().flush().ok();
        line.clear();
        // 读到 EOF（Ctrl+Z / Ctrl+D）退出命令行模式
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
            Some("help") => run_help(tokens.get(1).map(String::as_str)),
            Some(cmd) if !KNOWN_COMMANDS.contains(&cmd) => {
                eprintln!(
                    "{} 未知命令「{cmd}」。可用命令：{}",
                    ui::red("[错误]"),
                    KNOWN_COMMANDS.join(" / ")
                );
            }
            _ => {
                let argv = std::iter::once("redstone".to_string()).chain(tokens);
                match Cli::try_parse_from(argv) {
                    Ok(cli) => {
                        if let Err(e) = run_command(cli.command).await {
                            eprintln!("{} {e:#}", ui::red("[错误]"));
                        }
                    }
                    // clap 报错自带用法说明（英文），先给一行中文，再附原始详情
                    Err(e) => {
                        eprintln!("{} 参数不对。用法可查 redstone help <命令>。", ui::red("[错误]"));
                        eprintln!("{e}");
                    }
                }
            }
        }
    }
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

/// 表格里的「是 / 否」标记，保持定宽（不加色，避免破坏对齐）。
fn flag(ok: bool) -> &'static str {
    if ok {
        "是"
    } else {
        "否"
    }
}

/// 单点状态的「是 / 否」标记，带上颜色。
fn colored_flag(ok: bool) -> String {
    if ok {
        ui::green("是")
    } else {
        ui::red("否")
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

    #[test]
    fn safe_version_accepts_normal_ids() {
        for v in ["1.21.8", "26.3", "24w14a", "1.12.2", "1.20.1-forge-47.2.0"] {
            assert!(ensure_safe_version(v).is_ok(), "{v} 应被接受");
        }
    }

    #[test]
    fn safe_version_rejects_traversal() {
        for v in [
            "",
            ".",
            "..",
            "../..",
            "C:\\Windows",
            "..\\..\\Windows",
            "a/b",
            "a\\b",
            "a:b",
        ] {
            assert!(ensure_safe_version(v).is_err(), "{v} 应被拒绝");
        }
    }
}
