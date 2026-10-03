# Redstone Launcher

<p align="center">
  <img src="assets/icon.png" width="128" alt="Redstone Launcher icon" />
</p>

> **Version: 0.9.0-beta.3 (Beta)** · [![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE) · [![Platform](https://img.shields.io/badge/Platform-Windows%2010%2F11-blue)](https://www.microsoft.com/windows) · Rust

[English](README.en.md) | [简体中文](README.md)

A **Minecraft Java Edition launcher core written from scratch in Rust**. It currently
ships as a command-line tool and has a working end-to-end path: fetch the official
manifest → download the game → launch it into an actual playable client.

| | |
| --- | --- |
| Chinese name | 红石启动器 (Hongshi Launcher) |
| English name | Redstone Launcher |
| Command | `redstone` |
| Tech stack | Rust (tokio + reqwest), self-written core, no GPL libraries |
| Current stage | M1 — command-line core. The GUI stage (M2) has not started. |

---

## What is this

A Minecraft launcher built **from zero, not based on any existing launcher**.
HMCL / Prism / PCL2 / Axolotl were referenced for ideas only — no code was copied,
so the project carries no GPL contagion and can be redistributed under MIT.

The launcher does four things: fetch the official version manifest, parse the
version metadata, download and verify the game files, then assemble a correct
Java command line and start the game. **It does not crack anything, does not
bypass authentication, and does not ship any proprietary client** — the game
assets are downloaded by the user from official sources on first install.

## System requirements

| Item | Requirement |
| --- | --- |
| OS | Windows 10 / 11 (only Windows x86_64 has been verified so far) |
| Java | Any of JDK 8 / 17 / 20 / 21, or a newer version satisfying the target |
| Disk | ~530 MB per version for a full install (~97 MB libraries + ~428 MB assets) |
| Network | Access to `libraries.minecraft.net` and `resources.download.minecraft.net` |

The launcher scans system-wide install directories, the per-user
`%LOCALAPPDATA%\Programs\` tree, `JAVA_HOME` and `PATH`, and prefers a
**system-level** JDK. It only falls back to a JDK bundled with another application
when no system-level one satisfies the requirement — and it says so loudly
instead of silently degrading.

## Download

> **Current version: 0.9.0-beta.2 (Beta)**, open-sourced on 2026-10-03.
>
> **About the Beta stage:** the command-line core has a working download-and-launch
> path and the feature set is essentially complete, but compatibility with older
> versions (1.12.2 / 1.16.5) has **not** been verified, and both the directory
> layout and the command names may still change before 1.0.0.
> **Use it to try things out — not as your only launcher.**

### Source

```bat
git clone https://github.com/ccmmdd114514/Redstone-Launcher.git
cd Redstone-Launcher
cargo build --release
```

### Prebuilt packages

All prebuilt packages are published on the
[Releases page](https://github.com/ccmmdd114514/Redstone-Launcher/releases) under
Assets, named `redstone-launcher-<version>.zip`.

| Version | Type | Download | Size | Notes |
| --- | --- | --- | --- | --- |
| 0.9.0-beta.3 | Beta | Build from source as shown above | — | The exe now carries the redstone-block icon, but no prebuilt package was produced for this build |
| 0.9.0-beta.2 | Beta | [Release page](https://github.com/ccmmdd114514/Redstone-Launcher/releases/tag/v0.9.0-beta.2) | — | Source only: no prebuilt package for this build, build it yourself as shown above |
| 0.9.0-beta.1 | Beta | [redstone-launcher-0.9.0-beta.1.zip](https://github.com/ccmmdd114514/Redstone-Launcher/releases/download/v0.9.0-beta.1/redstone-launcher-0.9.0-beta.1.zip) | 3.19 MB | First prebuilt Beta package, Windows x86_64 |

Verify the download before running it — SHA-256:

```
7424021a3cf89b60cedab6fa1d3fff8614c8fe75d53ea8cff51735e01eb3026a *redstone-launcher-0.9.0-beta.1.zip
```

```powershell
Get-FileHash .\redstone-launcher-0.9.0-beta.1.zip -Algorithm SHA256
```

After extracting, just double-click `redstone.bat`. You can also run
`add-to-path.bat` to add the folder to your PATH.

## Quick start

1. Download `redstone-launcher-<version>.zip` from the Releases page above and extract it.
2. Optional: double-click `add-to-path.bat` to add the folder to your user PATH
   (it writes the registry through the .NET API, which avoids the silent PATH
   truncation that `setx` is known to cause).
3. Run the self-check to confirm Java and directory health:

```bat
redstone doctor
```

4. Install a version and launch it:

```bat
redstone install 1.21.8
redstone launch 1.21.8 --name Player --memory 4G
```

Running `redstone.bat` with no arguments prints the full help and then pauses, so
the window will not flash and vanish.

## Command reference

| Command | Purpose |
| --- | --- |
| `redstone list` | List recent releases (`--kind snapshot\|all`, `--limit N`) |
| `redstone instances` | Show installed versions and instance status |
| `redstone java` | List detected Java installations, tagged `[System]` or `[Bundled]` |
| `redstone doctor` | Self-check: Java, directory writability, installed versions, manifest reachability |
| `redstone install <ver>` | Install a version (official source by default, `--mirror bmclapi` to switch) |
| `redstone launch <ver>` | Launch the game (`--name`, `--memory`, `--dry-run` to print the command line only) |
| `redstone logs` | Show the tail of the last launch log (`--lines N`) |
| `redstone remove <ver>` | Delete a version's instance directory (`--yes` to actually delete) |

## Current capabilities

| Capability | Status |
| --- | --- |
| Fetch the official version manifest (`version_manifest_v2.json`) | Supported |
| Parse `arguments` from version.json (rule evaluation + variable expansion) | Supported |
| Download client.jar and libraries with sha1 + size verification and retries | Supported |
| Extract native libraries with zip-slip protection | Supported |
| Download game assets | Supported |
| Detect and select a local Java runtime (system-level preferred) | Supported |
| Offline mode (nickname → stable UUID via uuid5) | Supported |
| Official source / BMCLAPI mirror switching with fallback | Supported |
| Per-version instance isolation (separate saves and mods) | Supported |
| Launch log written to disk | Supported |
| Resume, JRE auto-download, official account login, mod loaders | Not implemented (later stages) |

## Where your data lives

The shared area and the instance area are deliberately separated: libraries and
assets are large and shared across versions, so they live in one place to avoid
downloading them repeatedly; while saves and mods must be isolated, otherwise
1.21.8 and 1.12.2 would corrupt each other's worlds.

| Path | Contents |
| --- | --- |
| `<root>\minecraft` | Shared: versions / libraries / assets / logs |
| `<root>\instances\<version>` | Per-version: saves / mods / options.txt |
| `<root>` | Determined by `dev_root()` in `src\paths.rs` (derived from `%LOCALAPPDATA%\MCLauncherDev`) |

The launcher deliberately **never touches** an existing official installation in
`%APPDATA%\.minecraft`; redirecting the data directory is a one-function change
in `src\paths.rs`.

## Building from source

```bat
cargo build --release
copy target\release\redstone.exe dist\redstone.exe
```

**Known pitfall on the Windows `-gnu` toolchain (read this first).** This project
targets `stable-x86_64-pc-windows-gnu` (no MSVC). If your build fails with

```
dlltool.exe: reopening ...\.lib: Permission denied
```

that is `windows-sys` (raw-dylib) being blocked while invoking `dlltool` during
linking — it is **not** a problem with the source code. Work around it by
pointing `target-dir` in `.cargo\config.toml` at an ASCII-only path with no
spaces, and enter the build from an English-named directory. This repository
deliberately does not ship `.cargo\config.toml`, because that file contains a
machine-specific path and is listed in `.gitignore`.

## Versioning and release policy

| Type | Version format | Meaning |
| --- | --- | --- |
| Alpha | `0.0.x-alpha.N` | Early/internal testing, incomplete, may crash |
| Beta | `0.9.0-beta.N` | Feature set essentially complete, open to public testing, still subject to change |
| Release | `1.0.0` and above | Feature-frozen, fully validated, suitable for daily use |

`Cargo.toml`'s `version` field is the single source of truth for the version
number; the binary reads the same value via `env!("CARGO_PKG_VERSION")`, so there
is no second place to keep in sync.

**Every release must follow `RELEASE.md`.** Packaging is done exclusively through
`python tools\release.py`, which validates that the release notes exist and are
complete (it refuses to package otherwise) and embeds them into the archive as
`RELEASE-NOTES.md`. Per-version notes live in `release-notes\`.

Beta release notes must contain two sections:

- **Improvements over the previous version**, written as explicit before/after pairs
- **Known bugs and unresolved issues in this version**, split into issues
  introduced by this build and issues inherited from the previous one

To publish a GitHub Release from a machine without the `gh` CLI:

```bat
set GH_TOKEN=ghp_your_token
python tools\publish-github-release.py 0.9.0-beta.3
```

## Project structure

| File | Responsibility |
| --- | --- |
| `src/main.rs` | CLI entry point (list / instances / install / launch / remove / logs / doctor / java) |
| `src/paths.rs` | Path helpers, the single source of every filesystem location |
| `src/meta.rs` | Official manifest structures and native-library classification |
| `src/net.rs` | Ordered download engine: 16 concurrent, 3 retries, sha1 + size verification, atomic `.part` replacement |
| `src/args.rs` | `arguments` rule evaluator (rule evaluation + variable expansion) |
| `src/install.rs` | Download and extraction of client, libraries, natives and assets |
| `src/java.rs` | Local Java discovery and selection (system vs. bundled tiering) |
| `src/launch.rs` | Command assembly, process spawning, log persistence |

## Key implementation notes

1. **The main class is never hardcoded** — it is always read from `mainClass` in
   `version.json`.
2. **The command line is not hand-assembled**; `arguments.jvm` and
   `arguments.game` are evaluated in full. `-cp ${classpath}` already comes from
   the official JVM arguments and is not added twice.
3. **Native libraries are identified by their classifier** (e.g.
   `:natives-windows`). arm64 / x86 variants are skipped entirely — they are
   neither added to the classpath nor extracted.
4. **Rule evaluation**: when a list contains an `allow` rule the default is
   "not applied"; when all entries are `disallow` the default is "applied"; the
   last matching rule wins.
5. **Valueless variables are dropped along with their flags**: in offline mode
   `${clientid}` and `${auth_xuid}` have no value, so the preceding `--clientId`
   flag is removed as well, preventing argument misalignment.
6. **No garbled text**: arguments are passed as a list, and three encoding flags
   including `-Dfile.encoding=UTF-8` are appended.
7. **Zero duplicate downloads across versions**: libraries and assets go to the
   shared area, saves and mods go to the per-version instance.

## Roadmap

| Stage | Scope | Status |
| --- | --- | --- |
| M1 | Command-line core (download, install, launch, logs) | Done |
| M2 | Graphical interface (Tauri v2 + Vue 3) | Planned |
| M3 | Modding ecosystem (Fabric / NeoForge / Forge) | Planned |

## Compliance and disclaimer

- This project **does not crack, does not bypass authentication, and does not
  distribute proprietary clients**. Offline mode is for single-player and your own
  servers only.
- Official account login (a later stage) goes exclusively through the official
  OAuth window and never collects plaintext passwords.
- Ideas were referenced from HMCL / Prism / PCL2 / Axolotl, but **no code was
  copied**; the core is self-written and therefore not affected by GPL.
- Distribution archives contain the launcher only. Game assets are downloaded by
  the user from official sources on first install.
- Minecraft is a trademark of Mojang Studios. This project is not affiliated with
  or endorsed by Mojang Studios, and the project does not infringe the Minecraft
  trademark.

This project is provided "as is", without warranty of any kind, express or
implied. You assume all risks associated with using it.

## License

[MIT License](LICENSE) — closed-source redistribution and commercial use are
permitted; only the copyright notice must be retained. See `LICENSE` for the
rationale behind this choice.
