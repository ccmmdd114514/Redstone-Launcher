# 自己写一个 Minecraft 启动器：技术路线与坑位清单

> 本文按"先跑通最小可用版本"的目标组织，适用于 Windows 平台、Python 技术栈起步。
> 撰写时间：2026-10-01。

## 一、启动器到底是什么

启动器本质只有三件事，拆开看就不神秘了：

1. **下载器**：把游戏需要的版本文件、依赖库（libraries）、资源文件（assets）下载到本地。
2. **命令行拼装器**：按 Minecraft 官方规定的 JVM 参数，拼出一条 `java -cp ... com.minecraft.client.Main` 命令启动游戏。
3. **账号与目录管理器**：管登录态、管每个版本的存档与模组目录互不串味。

**启动器不修改游戏本体，不绕过正版验证。** 只要走合法授权，官方是允许第三方启动器的。

## 二、建议的技术栈（Python 起步最省事）

| 层 | 推荐 | 理由 |
| --- | --- | --- |
| 界面 | PySide6（Qt for Python） | 与你已有的桌面宠物项目同一套，Windows 打包成熟 |
| 网络 | requests（简单场景）+ aiohttp（并发下载） | 多线程下载 assets/libraries 时 aiohttp 更好控速与断点 |
| 配置 | JSON（结构）+ TOML（用户设置） | 与官方 ecosystem 的 JSON 格式天然一致 |
| 打包 | PyInstaller | 与桌面宠物同款，产出独立 exe |
| 并发 | concurrent.futures.ThreadPoolExecutor | 下载数万个小文件必备，先限 8–16 线程，否则被限速 |

不建议一上来就上 Tauri / Rust / Electron（美西螈启动器那条 Tauri v2 + Rust + Vue 3 路线性能确实好，但工程量与调试成本对零基础不友好，且 GUI 调试反馈慢）。Python 版本先跑通逻辑，性能瓶颈（下载）用并发解决即可。

## 三、最小可用版本（MVP）的五个步骤

### Step 0：先定目录结构

沿用官方 `.minecraft` 结构，别自创目录，否则存档和模组都白跑：

```
%APPDATA%\.minecraft\
├─ versions\<版本名>\<版本名>.json   # 版本配置（来自官方）
├─ versions\<版本名>\<版本名>.jar    # 客户端 jar（下载或自动安装）
├─ libraries\                        # 依赖库与 natives
├─ assets\indexes\<索引hash>         # 资源索引
├─ assets\objects\<hash前2位>\<完整hash>  # 实际资源文件
└─ assets\skins                      # 皮肤
```

### Step 1：拉版本清单

`https://launchermeta.mojang.com/mc/game/version_manifest.json`
（旧地址 `launcher.mojang.com` 仍在部分文档里出现，新地址优先）

里面是一个 `{ latest: {...}, versions: [{ id, type, url, time }] }` 列表，按 id（如 `1.21.8`）取 `url` 就能拿到该版本的 `version.json`。

### Step 2：解析 version.json

`version.json` 是整套流程的核心，四个字段要分别处理：

| 字段 | 作用 | 处理要点 |
| --- | --- | --- |
| `assetIndex` | 资源索引地址 | 里面列了资源清单与 hash，用于 Step 3 |
| `libraries` | 依赖库列表 | 每条含 `name`/`url`/`size`/`sha1`；**带 `natives` 字段的条目是平台专属的，Windows 只取 `natives.windows`** |
| `downloads.client` | 客户端 jar 地址 | 老版本是 `client.jar`，新版本是 `client` 对象 |
| `javaVersion` | 需要的 Java 版本 | 1.16.5 及以前多为 Java 8；1.17+ 多为 Java 21/25，**照着 `javaVersion` 自动下载对应 JRE，能省掉 80% 的启动失败** |

### Step 3：下载资源与依赖库

- 资源索引里的每个条目是一个 hash，真实文件地址是
  `https://resources.download.minecraft.net/<hash 前两位>/<完整 hash>`，
  保存路径则是 `assets/objects/<hash 前两位>/<完整 hash>`（注意：**URL 与本地路径的前缀规则相同，但不要按文件名猜，一律按 hash 存**）。
- 库文件地址在 `libraries[].url` 里（默认 `https://libraries.minecraft.net/`），保存路径按 `name` 里的 group/artifact 拆成目录，最后一位是版本号。
- 每个文件都带 `sha1`，**下完必须校验**，不匹配就重下一次，这一步能挡掉 90% 的诡异崩溃。
- 用本地缓存（记录已下载文件的 hash + 大小），重复安装版本时直接秒过。

### Step 4：拼启动命令行

启动时照抄 `version.json` 里 `libraries` 拼出的 classpath，加上：

```
java ^
 -Xmx4G ^
 -Djava.library.path=<natives解压目录> ^
 -cp "<客户端jar>;<依赖库路径1>;...;<natives目录>" ^
 com.minecraft.client.Main ^
 --username <玩家名> --uuid <UUID> --accessToken <令牌> ^
 --assetIndex <索引hash> --assetsRoot <assets目录> ^
 --gameDirectory <版本目录> --version <版本号>
```

坑位提醒：

- Windows 的 classpath 分隔符是 `;`，Linux/macOS 是 `:`。
- natives（ lwjgl 的 .dll）要先解压到临时目录，再用 `-Djava.library.path` 指过去，用完可清理。
- **不要自己发明 JVM 参数**，官方 `version.json` 里能给出就照抄，手动加 `--add-opens` 之类只会在小版本升级时翻车。
- 内存分配先给物理内存的 50%–70%，模组再多往上加。

### Step 5：账号登录

| 方式 | 难度 | 说明 |
| --- | --- | --- |
| 离线账号 | ★ | 直接生成本地 UUID 与令牌，仅能进非正版验证的服务器，适合先跑通流程 |
| Yggdrasil / LittleSkin | ★★ | 自建或第三方认证服务器，社区方案，代码可抄的参考多 |
| 微软正版账号 | ★★★★ | 走 OAuth2 设备码流程 + 客户端密钥，需要自己申请 Azure 应用证书，还要处理令牌刷新与皮肤；想长期做就用这个，别跳过 |

MVP 建议顺序：离线登录 → 跑通启动 → 再补微软登录。

## 四、必须记住的 API 地址清单

| 用途 | 地址 | 备注 |
| --- | --- | --- |
| 版本清单 | `https://launchermeta.mojang.com/mc/game/version_manifest.json` | 入口，第一站 |
| 库文件仓库 | `https://libraries.minecraft.net/` | libraries 里的 url 默认是这里 |
| 资源文件仓库 | `https://resources.download.minecraft.net/` | 按 hash 取，见 Step 3 |
| 资源索引 | version.json 里 `assetIndex.url` 给出 | 内含所有资源条目 |
| 模组/整合包（首选） | `https://api.modrinth.com/v2` | 无需密钥，匿名限流，优先用它 |
| 整合包（次选） | CurseForge API | **2026 年 7 月起强制 API Key 认证**，没 key 会下载失败 |
| JDK 下载 | `https://api.adoptium.org/v3/binary/latest/<版本>/windows/x64/jre/hotspot/normal/eclipse` | Temurin/Adoptium 官方，可指定版本 |
| Maven 依赖 | `https://repo1.maven.org/maven2/` | 部分库从这里下 |

另有一条实用注解：**MCIM（社区镜像站）与 BMCLAPI 一类的第三方镜像**能显著提升中国大陆下载速度，PCL2 与 HMCL 都内置了这些源；自己做的话，可以在设置里预留"镜像源"下拉框，收益极大。

## 五、按坑密度排序的真实难点

1. **资源 hash 与目录规则**：看似简单，遇到 `assets/indexes` 与 `assets/objects` 两套路径容易写混，且部分资源需要转存到 `skins/` 目录。
2. **natives 判定**：`natives.windows` 与 `natives.linux` 的 classifier 字段不同，32/64 位、Windows ARM 也要分支。
3. **Java 版本联动**：老版本（1.12.2 等）跑 Java 17 会直接挂，必须按 `javaVersion` 精确到 8/17/21/25 分别管理。
4. **登录态生命周期**：微软令牌有刷新窗口，过期后要静默续期；离线账号改名的缓存也要处理。
5. **整合包生态**：`.mrpack` / CurseForge zip 的解析、`overrides/` 目录合并、依赖与可选模组勾选——这部分工作量常常超过核心启动器本身。
6. **崩溃排查**：日志采集与分析看着不起眼，实际是留住用户的关键功能，成熟项目（HMCL 的 Baka-Crash-Report、Prism 的日志上传）都是单独子项目。

## 六、里程碑与验收标准

| 阶段 | 产出 | 验收标准 |
| --- | --- | --- |
| M1（跑通原版） | 能下载 versions/libraries/assets 并启动 1.21.x 原版 | 正版账号能进单人世界 |
| M2（多版本） | 支持切换任意历史版本，Java 按需自动下载 | 1.12.2 与 1.21.x 共存互不干扰 |
| M3（模组加载器） | 支持 Fabric / NeoForge / Forge 安装按钮 | 装完 Sodium 能进游戏且帧率提升 |
| M4（整合包） | 支持 .mrpack 与 CurseForge 导入 | 一个大型整合包能一键装完启动 |
| M5（体验） | 崩溃日志、备份、截图、多账号切换 | 自媒体化传播所需的功能补齐 |

建议严格按 M1→M2 顺序推进，**M1 卡住就先别碰 Python GUI，直接用命令行脚本（`.py` + `subprocess`）验证逻辑**，界面最后再套上去，能少掉一半调试时间。

## 七、开源协议红线（很重要）

- HMCL（GPLv3）、Prism（GPL-3）、PCL2（自定义 GPL 附加条款）、Axolotl（GPL-3.0）：**代码可看，但直接复制进自己的闭源项目会构成侵权**。研究思路可以，抄代码不行。
- 若你打算闭源或商用，优先参考 MIT / Apache-2.0 许可的项目，例如 X Minecraft Launcher（MIT，Electron/TypeScript）。
- 启动器界面素材、图标、字体另算一套授权，别顺手把别人的主题打包进来。

## 八、可参考的现成实现

| 项目 | 语言/栈 | 许可 | 适合参考的部分 |
| --- | --- | --- | --- |
| HMCL | Java + JavaFX | GPLv3 | 多实例管理、加载器安装、崩溃诊断（逻辑清晰，文档全） |
| PCL2 | C# + WPF | GPL 附加条款 | 国内镜像源、社区资源整合、版本管理体验 |
| Prism Launcher | C++ + Qt | GPL-3 | 实例隔离模型、依赖图、可维护性最标准 |
| Modrinth App | TypeScript / Tauri | 开源 | 内容浏览与安装流程的产品设计 |
| Axolotl（美西螈） | Rust + Tauri + Vue 3 | GPL-3.0 | 现代化 UI 与"实验室"类附加工具的玩法设计 |

## 九、一句话建议

先写一个**命令行版 Python 脚本**把"拉清单 → 下文件 → 拼命令 → 起游戏"跑通（这一步能验证你对整套流程的理解），确认能进世界后，再套 PySide6 界面做成品。别一上来就画 UI，那是最容易半途而废的开工方式。
