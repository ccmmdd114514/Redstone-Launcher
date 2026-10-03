# 红石启动器 Redstone Launcher

<p align="center">
  <img src="assets/icon.png" width="128" alt="红石启动器图标" />
</p>

**🌐 Looking for the English version? [Click here →](README.en.md)**

> **版本：0.9.0-beta.3（Beta 测试版）** · [![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE) · [![Platform](https://img.shields.io/badge/Platform-Windows%2010%2F11-blue)](https://www.microsoft.com/windows) · Rust

一个用 Rust 从零自研的 **Minecraft Java 版启动器内核**。当前形态为命令行工具，
已完整跑通「拉取清单 → 下载游戏 → 启动游戏」主线，可实际进入游戏。

| | |
| --- | --- |
| 中文名 | 红石启动器 |
| 英文名 | Redstone Launcher |
| 命令行命令 | `redstone` |
| 技术栈 | Rust（tokio + reqwest），内核自写，不依赖任何 GPL 库 |
| 当前阶段 | 命令行内核（M1），图形界面阶段（M2）尚未开始 |

---

## 这是什么

一个**从零写、没有基于任何现成启动器**的 Minecraft 启动器。HMCL / Prism / PCL2 /
Axolotl 等项目的思路可作参考，但本项目不复制其代码，因此不受 GPL 传染性约束，
可以以 MIT 协议开源分发。

启动器负责四件事：拉官方清单、解析版本元数据、下载并校验游戏文件、拼出正确的
Java 命令行把游戏拉起来。**它不破解、不绕过正版验证、不分发盗版客户端**，
游戏本体首次安装时由用户自行从官方源下载。

## 系统要求

| 项目 | 要求 |
| --- | --- |
| 操作系统 | Windows 10 / 11（当前仅验证 Windows x86_64） |
| Java | JDK 8 / 17 / 20 / 21 任一，或满足目标版本要求的更高版本 |
| 磁盘 | 单个版本完整安装约需 530 MB（库约 97 MB + 资源约 428 MB） |
| 网络 | 需能访问 `libraries.minecraft.net` 与 `resources.download.minecraft.net` |

启动器会扫描系统级安装目录、用户级 `%LOCALAPPDATA%\Programs\` 目录、
`JAVA_HOME` 与 `PATH`，并优先选用**系统级** Java；只有在系统级完全不满足时才会
退到随其他应用附带的 JDK，并给出明确警告。

## 下载

> **当前版本：0.9.0-beta.2（Beta 测试版）**，于 2026-10-03 开源发布。
>
> **Beta 阶段说明**：命令行内核已跑通「拉取清单 → 下载游戏 → 启动游戏」主线，
> 功能基本齐全，但此前未验证过 1.12.2 / 1.16.5 等老版本的兼容分支，
> 且目录结构、参数命名在 1.0.0 之前仍可能调整。
> **建议先用于尝鲜，不要作为唯一启动器依赖。**

### 源码

```bat
git clone https://github.com/ccmmdd114514/redstone-launcher.git
cd redstone-launcher
cargo build --release
```

### 预编译包

全部预编译包发布在 [Releases 页面](https://github.com/ccmmdd114514/redstone-launcher/releases)
的 Assets 区，文件名形如 `redstone-launcher-<版本号>.zip`。

| 版本 | 类型 | 下载 | 大小 | 说明 |
| --- | --- | --- | --- | --- |
| 0.9.0-beta.3 | Beta 测试版 | 按上方源码方式构建 | — | exe 已带红石方块图标，但本次尚未产出预编译包 |
| 0.9.0-beta.2 | Beta 测试版 | [Release 页面](https://github.com/ccmmdd114514/Redstone-Launcher/releases/tag/v0.9.0-beta.2) | — | 源码版：本次未提供预编译包，请按上方方式自行构建 |
| 0.9.0-beta.1 | Beta 测试版 | [redstone-launcher-0.9.0-beta.1.zip](https://github.com/ccmmdd114514/Redstone-Launcher/releases/download/v0.9.0-beta.1/redstone-launcher-0.9.0-beta.1.zip) | 3.19 MB | 首个 Beta 预编译包，Windows x86_64 |

下载后请先校验文件完整性，SHA-256 如下：

```
7424021a3cf89b60cedab6fa1d3fff8614c8fe75d53ea8cff51735e01eb3026a *redstone-launcher-0.9.0-beta.1.zip
```

PowerShell 校验方式：

```powershell
Get-FileHash .\redstone-launcher-0.9.0-beta.1.zip -Algorithm SHA256
```

解压后双击 `redstone.bat` 即可运行；也可以运行 `add-to-path.bat` 把目录加入 PATH。

## 快速开始

1. 从上方 Releases 页面下载 `redstone-launcher-<版本号>.zip` 并解压。
2. 可选：双击 `add-to-path.bat`，把目录加入用户 PATH（该脚本用 .NET API 写入，
   可避免 `setx` 截断长 PATH 的坑）。
3. 运行自检，确认 Java 与目录状态正常：

```bat
redstone doctor
```

4. 安装一个版本并启动：

```bat
redstone install 1.21.8
redstone launch 1.21.8 --name Player --memory 4G
```

双击 `redstone.bat` 可直接运行，无参数时会暂停等待输入，避免窗口一闪而过。

## 命令速查

无参数直接运行会打印完整帮助。

```bat
:: 列出最近 10 个正式版（--kind snapshot|all 可切换，--limit 调整条数）
redstone list

:: 查看已安装的版本与实例状态
redstone instances

:: 列出本机探测到的 Java（标注 [系统] 或 [附带]）
redstone java

:: 自检：Java、目录可写性、已装版本、官方清单连通性
redstone doctor

:: 安装 1.21.8（默认官方源；改用镜像加 --mirror bmclapi）
redstone install 1.21.8

:: 只打印 java 命令行，不真正启动（排错用）
redstone launch 1.21.8 --dry-run

:: 启动游戏
redstone launch 1.21.8 --name 阿强 --memory 4G

:: 查看最近一次启动日志（默认最后 40 行）
redstone logs --lines 100

:: 删除某个版本的实例（不带 --yes 只预览；共享的库与资源不会被删）
redstone remove 1.21.8
redstone remove 1.21.8 --yes
```

## 当前能力

| 能力 | 状态 |
| --- | --- |
| 拉取官方版本清单（`version_manifest_v2.json`） | 支持 |
| 解析 version.json 的 `arguments`（rules 求值 + 变量展开） | 支持 |
| 下载 client.jar 与依赖库，sha1 + size 双重校验、失败重试 | 支持 |
| 原生库（natives）解压，含 zip-slip 防护 | 支持 |
| 下载资源文件（assets） | 支持 |
| 本机 Java 探测与自动选版（系统级优先） | 支持 |
| 离线账号（昵称 → uuid5 派生稳定 UUID） | 支持 |
| 官方源 / BMCLAPI 镜像切换与失败兜底 | 支持 |
| 多版本实例隔离（各版本独立存档与模组） | 支持 |
| 启动日志落盘 | 支持 |
| 断点续传、JRE 自动下载、正版登录、模组加载器 | 未实现（后续阶段） |

## 数据放在哪

共享区与实例区是分离的，这是刻意设计：库与资源体积大且各版本通用，放共享区可避免重复下载；
存档与模组必须隔离，否则 1.21.8 与 1.12.2 会互相串档。

| 路径 | 用途 |
| --- | --- |
| `<开发根>\minecraft` | 共享区：versions / libraries / assets / logs（多版本共用） |
| `<开发根>\instances\<版本>` | 实例区：该版本的 saves / mods / options.txt |
| `<开发根>` | 由 `src\paths.rs` 中的 `DEV_ROOT` 常量决定，改为你的目标目录即可 |

开发期默认指向用户目录下一个独立的开发根，**不会触碰**系统里已有的
`%APPDATA%\.minecraft` 存档；正式切换只需改 `src\paths.rs` 的 `DEV_ROOT` 常量。

## 从源码构建

```bat
cargo build --release
copy target\release\redstone.exe dist\redstone.exe
```

**Windows -gnu 工具链的已知坑（务必先看）**：本机使用
`stable-x86_64-pc-windows-gnu` 工具链（无 MSVC）。若构建时出现

```
dlltool.exe: reopening ...\.lib: Permission denied
```

这是 `windows-sys`（raw-dylib）在链接期调用 `dlltool` 被系统策略拦截所致，
**与源码本身无关**。规避方式：把 `.cargo\config.toml` 里的 `target-dir`
指向不含中文与空格的纯 ASCII 路径，并从同路径的英文目录进入构建。
本仓库不附带 `.cargo\config.toml`（该文件含本机专属路径，已在 `.gitignore` 中排除）。

## 版本与发布制度

| 类型 | 版本号格式 | 说明 |
| --- | --- | --- |
| 测试版（Alpha） | `0.0.x-alpha.N` | 内部或早期验证，功能不完整，可能崩溃 |
| Beta 测试版 | `0.9.0-beta.N` | 功能基本齐全，面向公开测试，仍可能变动 |
| 正式版（Release） | `1.0.0` 及以上 | 功能冻结，通过完整验收，可作日常主力 |

版本号以 `Cargo.toml` 的 `version` 为唯一来源，程序内的版本号通过
`env!("CARGO_PKG_VERSION")` 自动读取，不需要两处维护。

**发布必须遵守 `RELEASE.md`**：每次打包统一执行 `python tools\release.py`，
该工具会强制校验发布说明是否齐备（缺说明会直接拒绝打包），
并把说明以 `RELEASE-NOTES.md` 打进包里。各版本说明存于 `release-notes\`。

Beta 版发布说明必须包含两块内容：

- **相较于上一版的改进**（用「此前……现在……」逐条对照）
- **本版本已知 bug 与未解决项**（分「本版本新增问题」与「上一版继承未修」两类）

## 项目结构

| 文件 | 职责 |
| --- | --- |
| `src/main.rs` | 命令行入口（list / instances / install / launch / remove / logs / doctor / java） |
| `src/paths.rs` | 路径常量，全项目唯一的绝对路径来源 |
| `src/meta.rs` | 官方元数据结构与原生库判定 |
| `src/net.rs` | 有序源下载引擎：并发 16、重试 3 次、sha1 + size 校验、`.part` 原子替换 |
| `src/args.rs` | arguments 规则求值器（rules 求值 + 变量展开） |
| `src/install.rs` | 客户端、依赖库、原生库、资源的下载与解压 |
| `src/java.rs` | 本机 Java 探测与选版（系统级 / 附带级分级） |
| `src/launch.rs` | 启动命令拼装、进程拉起、日志落盘 |

## 关键实现要点

1. **主类名不做硬编码**，一律读 `version.json` 的 `mainClass`。
2. **不自己攒命令行**，完整求值 `arguments.jvm` 与 `arguments.game`；
   `-cp ${classpath}` 由官方 jvm 参数自带，不重复添加。
3. **原生库靠库名 classifier 判定**（如 `:natives-windows`）；
   arm64 / x86 变体整个跳过，既不进 classpath 也不解压。
4. **rules 求值**：列表中存在 allow 规则时默认不应用，全为 disallow 时默认应用，
   最后一条命中的规则决定结果。
5. **无值变量整条丢弃**：离线模式下 `${clientid}`、`${auth_xuid}` 无值，
   连同前面的 `--clientId` 标志一起删除，避免参数错位。
6. **中文不乱码**：参数按列表传递，并追加 `-Dfile.encoding=UTF-8` 等三个编码参数。
7. **多版本零重复下载**：库与资源进共享区，存档与模组进实例区。

## 路线图

| 阶段 | 内容 | 状态 |
| --- | --- | --- |
| M1 | 命令行内核（下载、安装、启动、日志） | 已完成 |
| M2 | 图形界面（Tauri v2 + Vue 3） | 规划中 |
| M3 | 模组生态（Fabric / NeoForge / Forge） | 规划中 |

## 合规说明与免责声明

- **本项目不做破解、不绕过正版验证、不分发盗版客户端**；离线账号仅用于单机与自有服务器。
- 微软登录（后续阶段）只走官方 OAuth 窗口，不收集明文密码。
- 参考 HMCL / Prism / PCL2 / Axolotl 等开源项目的思路，但**不复制其代码**；
  本项目自写内核，不受 GPL 传染。
- 分发包只含启动器本身，游戏资源由用户首次安装时自行下载。
- Minecraft 是 Mojang Studios 的商标，本项目与 Mojang Studios 无隶属关系，
  亦未获得其官方授权。本项目的存在不侵犯 Minecraft 商标权。

本项目按「原样」提供，不附带任何明示或暗示的担保。使用者自行承担使用风险。

## 许可证

[MIT License](LICENSE) — 允许闭源二次分发与商用，仅需保留版权声明。
详见 [LICENSE](LICENSE) 文末对协议选型的说明。
