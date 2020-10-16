//! list of old kernel versions.

use anyhow::{anyhow, Result};
use log::{info, warn};
use serde_derive::Deserialize;
use std::fmt;
use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};

pub const URL_BASE: &'static str = "https://mirrors.kernel.org/pub/linux/kernel";
const KERNELS: &'static str = include_str!("kernels.yaml");

/// Get all kernel versions.
pub fn kernels() -> Result<Kernels> {
    serde_yaml::from_str(KERNELS).map_err(|e| anyhow!("failed to deserialize kernels: {}", e))
}

#[derive(Deserialize, Debug, Clone)]
pub struct Kernels {
    pub releases: Vec<KernelRelease>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct KernelRelease {
    /// If this version is important.
    #[serde(default)]
    pub important: bool,
    version: String,
    /// Custom path to download the kernel, relative to the mirror.
    pub path: Option<String>,
}

impl KernelRelease {
    fn path(&self) -> String {
        if let Some(path) = self.path.as_ref() {
            return path.to_string();
        }

        let version = self.version.as_str();

        let mut parts = version.split(".");
        let major = parts.next().unwrap_or_else(|| "expected major version");
        let minor = match parts.next() {
            Some(minor) => minor,
            None => "x",
        };

        let name = match version {
            "1.1.0" => format!("v{}", version),
            _ => format!("linux-{version}", version = version),
        };

        format!(
            "v{major}.{minor}/{name}.tar.gz",
            major = major,
            minor = minor,
            name = name,
        )
    }

    /// Get the downloadable URL for the given kernel version.
    pub fn tar_gz_url(&self) -> Result<String> {
        let path = self.path();
        Ok(format!("{base}/{path}", base = URL_BASE, path = path))
    }
}

impl fmt::Display for KernelRelease {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        self.version.fmt(fmt)
    }
}

#[derive(Debug, Clone)]
pub struct CachedKernel<'a> {
    pub version: &'a KernelRelease,
    pub path: PathBuf,
}

/// Download the archives of the listed versions in parallel.
pub async fn download_old_kernels<'a>(
    root: &Path,
    versions: &'a [KernelRelease],
    verify: bool,
    parallelism: usize,
) -> Result<Vec<CachedKernel<'a>>> {
    let total = versions.len();
    let mut results = Vec::new();

    let mut it = versions.iter().enumerate();
    let mut tasks = unicycle::FuturesUnordered::new();
    let mut count = 0;

    loop {
        if count < parallelism {
            if let Some((index, version)) = it.next() {
                count += 1;
                tasks.push(download_archive(index, total, root, version, verify));
                continue;
            }
        }

        if tasks.is_empty() {
            break;
        }

        results.push(tasks.next().await.unwrap()?);
        count -= 1;
    }

    return Ok(results);

    /// Download the specified archive.
    async fn download_archive<'a>(
        index: usize,
        total: usize,
        root: &Path,
        version: &'a KernelRelease,
        verify: bool,
    ) -> Result<CachedKernel<'a>> {
        let path = root.join(format!("linux-{}.tar.gz", version));

        // use existing path if it already exists.
        if path.is_file() {
            let ok = if verify {
                match test_archive(&path) {
                    Ok(()) => true,
                    Err(e) => {
                        warn!("ignoring bad archive: {}: {}", path.display(), e);
                        fs::remove_file(&path)
                            .map_err(|e| anyhow!("failed to remove: {}: {}", path.display(), e))?;
                        false
                    }
                }
            } else {
                true
            };

            if ok {
                info!("{}/{}: OK: {}", index, total, path.display());
                return Ok(CachedKernel { version, path });
            }
        }

        let url = version.tar_gz_url()?;

        info!(
            "{}/{}: downloading {} -> {}",
            index,
            total,
            url,
            path.display()
        );

        let res = reqwest::get(&url)
            .await
            .map_err(|e| anyhow!("failed to get url: {}: {}", url, e))?;

        if !res.status().is_success() {
            return Err(anyhow!("failed to download: {}: {}", url, res.status()).into());
        }

        let buf = res.bytes().await?;

        if let Err(e) = test_reader_archive(Cursor::new(&buf)) {
            return Err(anyhow!(
                "test on downloaded archive failed: {}: {}",
                path.display(),
                e
            ));
        }

        let mut out = fs::File::create(&path)
            .map_err(|e| anyhow!("failed to open file: {}: {}", path.display(), e))?;

        out.write_all(&buf)
            .map_err(|e| anyhow!("failed to write file: {}: {}", path.display(), e))?;
        out.sync_all()
            .map_err(|e| anyhow!("failed to sync: {}: {}", path.display(), e))?;

        Ok(CachedKernel { version, path })
    }

    /// Test that the given path is a proper archive.
    ///
    /// Returns a reason string describing what's wrong with the archive if it's not OK.
    /// Otherwise, returns `None`.
    fn test_archive(path: &Path) -> Result<()> {
        let f = match fs::File::open(path) {
            Err(e) => return Err(anyhow!("failed to open archive: {}", e)),
            Ok(f) => f,
        };

        test_reader_archive(f)
    }

    /// Test that the reader archive is OK.
    fn test_reader_archive(reader: impl Read) -> Result<()> {
        use flate2::read::GzDecoder;
        use tar::Archive;

        let mut a = Archive::new(GzDecoder::new(reader));

        let entries = match a.entries() {
            Err(e) => return Err(anyhow!("failed to list tar entries: {}", e)),
            Ok(entries) => entries,
        };

        for entry in entries {
            let entry = match entry {
                Err(e) => return Err(anyhow!("bad entry: {}", e)),
                Ok(entry) => entry,
            };

            if let Err(e) = entry.path() {
                return Err(anyhow!("bad entry: {}", e));
            }
        }

        Ok(())
    }
}
