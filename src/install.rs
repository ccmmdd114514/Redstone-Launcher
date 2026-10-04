//! 安装器：client.jar + libraries + natives 解压 + assets。

use crate::meta::{self, AssetIndex, VersionJson};
use crate::net::{self, Http, Item};
use crate::paths;
use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};

/// 拉版本清单并取指定版本的 version.json。
pub async fn fetch_version_json(http: &Http, version: &str) -> Result<VersionJson> {
    let manifest: meta::VersionManifest = http
        .json(meta::MANIFEST_URL, version)
        .await
        .context("拉取版本清单失败")?;
    let entry = manifest
        .versions
        .iter()
        .find(|v| v.id == version)
        .ok_or_else(|| anyhow!("版本清单里找不到 {version}"))?;
    let vj: VersionJson = http
        .json(&entry.url, version)
        .await
        .context("拉取版本配置失败")?;
    Ok(vj)
}

/// 拆出 classpath 与 natives jar 列表（绝对路径）。
pub fn split_libraries(vj: &VersionJson) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let features = HashMap::new();
    let mut classpath = Vec::new();
    let mut natives = Vec::new();

    for lib in &vj.libraries {
        if let Some(rules) = &lib.rules {
            if !crate::args::rules_allow(rules, &features) {
                continue;
            }
        }
        if lib.is_native() {
            if let Some(art) = lib.native_artifact() {
                if art.url.is_empty() {
                    // 该原生库没有下载地址，跳过它本身，不能中断整个循环——
                    // 否则它之后的库与原生库全部丢失，装出来能过校验却起不来。
                    continue;
                }
                natives.push(paths::libraries_dir().join(lib.relative_path(art)));
            }
            // 其他架构的原生库直接跳过，不进 classpath
            continue;
        }
        if let Some(art) = lib.artifact() {
            if art.url.is_empty() {
                continue;
            }
            let rel = lib.relative_path(art);
            classpath.push(paths::libraries_dir().join(rel));
        }
    }
    (classpath, natives)
}

/// libraries 的下载清单（供 install 复用）。
pub fn library_items(http: &Http, vj: &VersionJson) -> Vec<Item> {
    let features = HashMap::new();
    let mut items = Vec::new();
    for lib in &vj.libraries {
        if let Some(rules) = &lib.rules {
            if !crate::args::rules_allow(rules, &features) {
                continue;
            }
        }
        if lib.is_native() {
            // 只下载当前平台要用的那一个原生库
            if let Some(art) = lib.native_artifact() {
                if !art.url.is_empty() {
                    items.push(Item {
                        label: lib.name.clone(),
                        urls: http.candidates(&art.url, &vj.id),
                        dest: paths::libraries_dir().join(lib.relative_path(art)),
                        sha1: art.sha1.clone(),
                        size: art.size,
                    });
                }
            }
            continue;
        }
        let art = match lib.artifact() {
            Some(a) if !a.url.is_empty() => a,
            _ => continue,
        };
        let rel = lib.relative_path(art);
        items.push(Item {
            label: lib.name.clone(),
            urls: http.candidates(&art.url, &vj.id),
            dest: paths::libraries_dir().join(rel),
            sha1: art.sha1.clone(),
            size: art.size,
        });
    }
    items
}

pub fn client_item(http: &Http, vj: &VersionJson) -> Result<Item> {
    let art = vj
        .downloads
        .get("client")
        .ok_or_else(|| anyhow!("版本配置里缺少 downloads.client"))?;
    Ok(Item {
        label: format!("{} client.jar", vj.id),
        urls: http.candidates(&art.url, &vj.id),
        dest: paths::version_jar(&vj.id),
        sha1: art.sha1.clone(),
        size: art.size,
    })
}

/// assets 的下载清单（索引 + 全部对象）。
pub async fn asset_items(http: &Http, vj: &VersionJson) -> Result<Vec<Item>> {
    let index_path = paths::indexes_dir().join(format!("{}.json", vj.asset_index.id));
    std::fs::create_dir_all(paths::indexes_dir())?;

    // 索引本身必须校验。Mojang 在 version.json 的 assetIndex 里给了 sha1 与 size，
    // 以前这里走 http.json() 直连拉取，两者全丢：索引被截断、被镜像塞了私货，
    // 都要一路错到几千个资源文件上才暴露。改走带校验的 download()。
    let urls = http.candidates(&vj.asset_index.url, &vj.id);
    http.download(
        &urls,
        &index_path,
        Some(&vj.asset_index.sha1),
        Some(vj.asset_index.size),
    )
    .await
    .context("下载资源索引失败（sha1 或大小校验未通过）")?;

    let raw = std::fs::read(&index_path).context("读取资源索引失败")?;
    let index: AssetIndex =
        serde_json::from_slice(&raw).context("解析资源索引失败：内容不是合法 JSON")?;

    let mut items = Vec::with_capacity(index.objects.len());
    for (name, obj) in &index.objects {
        let h = &obj.hash;
        let url = format!(
            "https://resources.download.minecraft.net/{}/{h}",
            &h[..2]
        );
        items.push(Item {
            label: name.clone(),
            urls: http.candidates(&url, &vj.id),
            dest: paths::objects_dir().join(&h[..2]).join(h),
            sha1: Some(h.clone()),
            size: Some(obj.size),
        });
    }
    Ok(items)
}

/// 执行完整安装。
pub async fn install(version: &str, http: &Http) -> Result<()> {
    paths::ensure_dirs(version)?;
    println!("[1/5] 拉取版本配置 {version} ...");
    let vj = fetch_version_json(http, version).await?;
    std::fs::create_dir_all(paths::version_dir(version))?;
    std::fs::write(
        paths::version_json(version),
        serde_json::to_string_pretty(&vj)?,
    )?;
    println!(
        "      启动主类：{}   所需 Java：{}",
        vj.main_class,
        vj.java_version
            .as_ref()
            .map(|j| j.major_version.to_string())
            .unwrap_or_else(|| "未知".into())
    );

    println!("[2/5] 下载客户端与依赖库 ...");
    let mut items = vec![client_item(http, &vj)?];
    items.extend(library_items(http, &vj));
    let total = items.len();
    let stats = net::download_many(http, items, net::AbortPolicy::default()).await?;
    println!(
        "      客户端+依赖库：新增 {} 个，跳过 {} 个，失败 {} 个（共 {total}）",
        stats.ok, stats.skipped, stats.failed
    );

    println!("[3/5] 解压原生库 ...");
    let (_, natives) = split_libraries(&vj);
    let count = extract_natives(&natives, &paths::natives_dir(version))?;
    println!("      解压 {count} 个文件 → {}", paths::natives_dir(version).display());

    println!("[4/5] 下载资源文件 ...");
    let assets = asset_items(http, &vj).await?;
    let asset_total = assets.len();
    let astats = net::download_many(http, assets, net::AbortPolicy::default()).await?;
    println!(
        "      资源：新增 {} 个，跳过 {} 个，失败 {} 个（共 {asset_total}）",
        astats.ok, astats.skipped, astats.failed
    );

    println!("[5/5] 校验 ...");
    let missing = verify(&vj, version);
    if stats.failed > 0 || astats.failed > 0 || !missing.is_empty() {
        eprintln!("安装未完全成功，缺失文件 {} 个", missing.len());
        for m in missing.iter().take(10) {
            eprintln!("  缺失：{}", m.display());
        }
        return Err(anyhow!("安装未完成"));
    }
    println!("安装完成 ✅");
    Ok(())
}

/// 检查启动必需文件是否齐全，返回缺失列表。
pub fn verify(vj: &VersionJson, version: &str) -> Vec<PathBuf> {
    let mut missing = Vec::new();
    if !paths::version_jar(version).exists() {
        missing.push(paths::version_jar(version));
    }
    let (cp, _) = split_libraries(vj);
    for p in cp {
        if !p.exists() {
            missing.push(p);
        }
    }
    missing
}

/// 解压 natives jar 到目标目录。解压前清空目录，保留 jar 内子目录结构，
/// 并对每个条目做路径穿越检查（zip-slip）。
pub fn extract_natives(jars: &[PathBuf], target: &Path) -> Result<usize> {
    if target.exists() {
        std::fs::remove_dir_all(target)
            .with_context(|| format!("清空目录失败（游戏可能正在运行）：{target:?}"))?;
    }
    std::fs::create_dir_all(target)?;
    let root = target
        .canonicalize()
        .with_context(|| format!("无法解析目录：{target:?}"))?;

    let mut count = 0usize;
    for jar in jars {
        if !jar.exists() {
            return Err(anyhow!("原生库缺失：{}", jar.display()));
        }
        let f = File::open(jar)?;
        let mut archive = zip::ZipArchive::new(BufReader::new(f))
            .with_context(|| format!("打开 jar 失败：{jar:?}"))?;
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)?;
            if !entry.is_file() {
                continue;
            }
            let name = match entry.enclosed_name() {
                Some(p) => p,
                None => continue,
            };
            let name_str = name.to_string_lossy().replace('\\', "/");
            if name_str.starts_with("META-INF/") {
                continue;
            }
            let out = root.join(name);
            let parent = out.parent().unwrap_or(&root);
            std::fs::create_dir_all(parent)?;
            let canon_parent = parent.canonicalize().unwrap_or_else(|_| root.clone());
            if !canon_parent.starts_with(&root) {
                return Err(anyhow!("检测到路径穿越，已中止：{name_str}"));
            }
            let mut of = File::create(&out)?;
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf)?;
            of.write_all(&buf)?;
            count += 1;
        }
    }
    Ok(count)
}
