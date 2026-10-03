//! 下载引擎：有序源列表（官方优先 / 镜像兜底）、并发、sha1 + size 双重校验、
//! 重试 3 次、.part 原子替换。

use anyhow::{anyhow, Context, Result};
use futures::StreamExt;
use reqwest::Client;
use serde::de::DeserializeOwned;
use sha1::{Digest, Sha1};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::sync::Semaphore;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mirror {
    Official,
    Bmclapi,
}

impl Mirror {
    pub fn parse(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "official" | "官方" => Ok(Mirror::Official),
            "bmclapi" | "bmcl" | "镜像" => Ok(Mirror::Bmclapi),
            other => Err(anyhow!("未知镜像源：{other}（可选 official / bmclapi）")),
        }
    }
}

/// 把官方 URL 改写为 BMCLAPI（v2 端点）。改不了就返回 None。
fn to_bmclapi(url: &str, version: &str) -> Option<String> {
    if url.starts_with("https://piston-data.mojang.com") && url.ends_with("/client.jar") {
        return Some(format!(
            "https://bmclapi2.bangbang93.com/version/{version}/client"
        ));
    }
    if url.contains("/v1/packages/") && url.starts_with("https://piston-meta.mojang.com") {
        return Some(format!(
            "https://bmclapi2.bangbang93.com/version/{version}/json"
        ));
    }
    if let Some(rest) = url.strip_prefix("https://libraries.minecraft.net/") {
        return Some(format!("https://bmclapi2.bangbang93.com/maven/{rest}"));
    }
    if let Some(rest) = url.strip_prefix("https://repo1.maven.org/maven2/") {
        return Some(format!("https://bmclapi2.bangbang93.com/maven/{rest}"));
    }
    if let Some(rest) = url.strip_prefix("https://resources.download.minecraft.net/") {
        return Some(format!("https://bmclapi2.bangbang93.com/assets/{rest}"));
    }
    None
}

#[derive(Clone)]
pub struct Http {
    client: Client,
    pub mirror: Mirror,
}

impl Http {
    pub fn new(mirror: Mirror) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(180))
            .connect_timeout(Duration::from_secs(15))
            .user_agent(concat!("MCLauncher/", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self { client, mirror })
    }

    /// 返回按优先级排序的候选地址：首选 + 兜底。
    pub fn candidates(&self, url: &str, version: &str) -> Vec<String> {
        let alt = to_bmclapi(url, version);
        match (self.mirror, alt) {
            (Mirror::Official, Some(a)) => vec![url.to_string(), a],
            (Mirror::Bmclapi, Some(a)) => vec![a, url.to_string()],
            _ => vec![url.to_string()],
        }
    }

    pub async fn json<T: DeserializeOwned>(&self, url: &str, version: &str) -> Result<T> {
        let mut last = None;
        for u in self.candidates(url, version) {
            for attempt in 0..3 {
                match self.client.get(&u).send().await {
                    Ok(resp) if resp.status().is_success() => {
                        return resp.json::<T>().await.context("解析 JSON 失败");
                    }
                    Ok(resp) => last = Some(anyhow!("HTTP {}", resp.status())),
                    Err(e) => last = Some(anyhow!("{e}")),
                }
                tokio::time::sleep(Duration::from_millis(300 * (attempt + 1))).await;
            }
        }
        Err(last.unwrap_or_else(|| anyhow!("拉取失败：{url}")))
    }

    /// 下载单个文件。已存在且校验通过则跳过（返回 0）。
    pub async fn download(
        &self,
        cands: &[String],
        dest: &Path,
        sha1: Option<&str>,
        size: Option<u64>,
    ) -> Result<u64> {
        if let Ok(meta) = std::fs::metadata(dest) {
            let size_ok = size.map(|s| meta.len() == s).unwrap_or(true);
            if size_ok {
                match sha1 {
                    Some(h) if file_sha1(dest)? == h => return Ok(0),
                    Some(_) => {}
                    None => return Ok(0),
                }
            }
        }
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = part_path(dest);
        let mut last = None;
        for url in cands {
            for attempt in 0..3 {
                match self.try_download(url, &tmp, sha1, size).await {
                    Ok(n) => {
                        std::fs::rename(&tmp, dest)
                            .with_context(|| format!("重命名失败：{tmp:?} -> {dest:?}"))?;
                        return Ok(n);
                    }
                    Err(e) => {
                        last = Some(e);
                        tokio::time::sleep(Duration::from_millis(300 * (attempt + 1))).await;
                    }
                }
            }
        }
        let _ = std::fs::remove_file(&tmp);
        Err(last.unwrap_or_else(|| anyhow!("无可用下载地址")))
    }

    async fn try_download(
        &self,
        url: &str,
        tmp: &Path,
        sha1: Option<&str>,
        size: Option<u64>,
    ) -> Result<u64> {
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .with_context(|| format!("请求失败：{url}"))?;
        if !resp.status().is_success() {
            return Err(anyhow!("HTTP {} ({url})", resp.status()));
        }
        let expected = resp.content_length().or(size);
        if let (Some(e), Some(s)) = (expected, size) {
            if e != s {
                return Err(anyhow!("长度不符：声明 {e}，期望 {s} ({url})"));
            }
        }
        let mut file = tokio::fs::File::create(tmp).await?;
        let mut stream = resp.bytes_stream();
        let mut total = 0u64;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("下载中断")?;
            total += chunk.len() as u64;
            file.write_all(&chunk).await?;
        }
        file.flush().await?;
        drop(file);

        if let Some(s) = size {
            let actual = std::fs::metadata(tmp)?.len();
            if actual != s {
                return Err(anyhow!("大小不符：实际 {actual}，期望 {s}"));
            }
        }
        if let Some(h) = sha1 {
            let actual = file_sha1(tmp)?;
            if actual != h {
                return Err(anyhow!("校验失败：实际 {actual}，期望 {h}"));
            }
        }
        Ok(total)
    }
}

fn part_path(dest: &Path) -> PathBuf {
    let mut name = dest
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "download".to_string());
    name.push_str(".part");
    dest.with_file_name(name)
}

pub fn file_sha1(path: &Path) -> Result<String> {
    let mut f = File::open(path).with_context(|| format!("无法打开：{path:?}"))?;
    let mut hasher = Sha1::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

pub struct Item {
    pub label: String,
    pub urls: Vec<String>,
    pub dest: PathBuf,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}

pub struct Stats {
    pub ok: usize,
    pub skipped: usize,
    pub failed: usize,
    pub bytes: u64,
}

/// 并发下载，最多 16 路。
pub async fn download_many(http: &Http, items: Vec<Item>) -> Result<Stats> {
    let total = items.len();
    let http = Arc::new(http.clone());
    let sem = Arc::new(Semaphore::new(16));
    let mut handles = Vec::with_capacity(total);

    for item in items {
        let h = http.clone();
        let permit = sem.clone();
        handles.push(tokio::spawn(async move {
            let _p = permit.acquire_owned().await;
            let r = h
                .download(&item.urls, &item.dest, item.sha1.as_deref(), item.size)
                .await;
            (item.label, r)
        }));
    }

    let mut stats = Stats {
        ok: 0,
        skipped: 0,
        failed: 0,
        bytes: 0,
    };
    let mut done = 0usize;
    for h in handles {
        done += 1;
        match h.await {
            Ok((_, Ok(n))) => {
                if n == 0 {
                    stats.skipped += 1;
                } else {
                    stats.ok += 1;
                    stats.bytes += n;
                }
            }
            Ok((label, Err(e))) => {
                stats.failed += 1;
                eprintln!("  [失败] {label}: {e}");
            }
            Err(e) => {
                stats.failed += 1;
                eprintln!("  [失败] 任务异常：{e}");
            }
        }
        if done % 250 == 0 || done == total {
            println!(
                "  进度 {done}/{total}，新增 {:.1} MB，跳过 {}，失败 {}",
                stats.bytes as f64 / 1_048_576.0,
                stats.skipped,
                stats.failed
            );
        }
    }
    Ok(stats)
}
