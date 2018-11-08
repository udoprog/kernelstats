//! list of old kernel versions.

use crate::error::Error;
use log::{info, warn};
use std::fmt;
use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};

pub const URL_BASE: &'static str = "https://mirrors.kernel.org/pub/linux/kernel";

macro_rules! versions {
    ($($version:expr,)*) => {
        vec![$(KernelVersion::new($version),)*];
    }
}

/// Get all kernel versions.
pub fn versions() -> Vec<KernelVersion> {
    versions![
        "!1.0", "!1.1.0", "1.1.13", // "1.1.23",
        "1.1.29", "1.1.33", "1.1.35", "1.1.45", "1.1.52", "1.1.59", "1.1.63", "1.1.64", "1.1.67",
        "1.1.70", "1.1.71", // "1.1.73",
        "1.1.74", "1.1.75", "1.1.76", "1.1.78", "1.1.79", "1.1.80", "1.1.81", "1.1.82", "1.1.83",
        "1.1.84", "1.1.85", "1.1.86", "1.1.87", "1.1.88", "1.1.89", "1.1.90", "1.1.91", "1.1.92",
        "1.1.93", "1.1.94", "1.1.95", "!1.2.0", "1.2.1", "1.2.2", "1.2.3", "1.2.4", "1.2.5",
        "1.2.6", "1.2.7", "1.2.8", "1.2.9", "1.2.10", "1.2.11", "1.2.12", "1.2.13", "!1.3.0",
        "1.3.2", "1.3.3", "1.3.4", "1.3.5", "1.3.6", "1.3.7", "1.3.8", "1.3.9", "1.3.10", "1.3.11",
        "1.3.12", "1.3.13", "1.3.14", "1.3.15", "1.3.16", "1.3.17", "1.3.18", "1.3.19", "1.3.20",
        "1.3.21", "1.3.22", "1.3.23", "1.3.24", "1.3.25", "1.3.26", "1.3.27", "1.3.28", "1.3.29",
        "1.3.30", "1.3.31", "1.3.32", "1.3.33", "1.3.34", "1.3.35", "1.3.36",
        // "1.3.37",
        "1.3.38", "1.3.39", "1.3.40", "1.3.41", "1.3.42", "1.3.43", "1.3.44", "1.3.45", "1.3.46",
        "1.3.47", "1.3.48", "1.3.49", "1.3.50", "1.3.51", "1.3.52", "1.3.53", "1.3.54", "1.3.55",
        "1.3.56", "1.3.57", "1.3.58", "1.3.59", "1.3.60", "1.3.61", "1.3.63", "1.3.64", "1.3.65",
        "1.3.66", "1.3.67", // "1.3.68",
        "1.3.69", "1.3.70", "1.3.71", "1.3.72", "1.3.73", "1.3.74", "1.3.75", "1.3.76", "1.3.77",
        "1.3.78", "1.3.79", "1.3.80", "1.3.81", // "1.3.82",
        "1.3.83", "1.3.84", "1.3.85", "1.3.86", "1.3.87", "1.3.88", "1.3.89", "1.3.90", "1.3.91",
        "1.3.92", "1.3.93", "1.3.94", "1.3.95", "1.3.96", "1.3.97", "1.3.98", "1.3.99", "1.3.100",
        "!2.0", "2.0.1", "2.0.2", "2.0.3", "2.0.4", "2.0.5", "2.0.6", "2.0.7", "2.0.8", "2.0.9",
        "2.0.10", "2.0.11", // "2.0.12",
        "2.0.13", "2.0.14", "2.0.15", "2.0.16", "2.0.17", "2.0.18", "2.0.19", "2.0.20", "2.0.21",
        // "2.0.22",
        "2.0.23", "2.0.24", "2.0.25", "2.0.26", "2.0.27", "2.0.28", "2.0.29", "2.0.30",
        // "2.0.31",
        "2.0.32", "2.0.33", // "2.0.34",
        "2.0.35", "2.0.36", "2.0.37", "2.0.38", "2.0.39", "2.0.40", "!2.1.0", "!2.2.0", "2.2.1",
        "2.2.2", "2.2.3", "2.2.4", "2.2.5", "2.2.6", "2.2.7", "2.2.8", "2.2.9", "2.2.10", "2.2.11",
        "2.2.12", "2.2.13", "2.2.14", "2.2.15", "2.2.16", "2.2.17", "2.2.18", "2.2.19", "2.2.20",
        "2.2.21", "2.2.22", "2.2.23", "2.2.24", "2.2.25", "2.2.26", "!2.3.0", "2.3.1", "2.3.2",
        "2.3.3", "2.3.4", "2.3.5", "2.3.6", "2.3.7", "2.3.8", "2.3.9", "2.3.10", "2.3.11",
        "2.3.12", "2.3.13", "2.3.14", "2.3.15", "2.3.16", "2.3.17", "2.3.18", "2.3.19", "2.3.20",
        "2.3.21", "2.3.22", "2.3.23", "2.3.24", "2.3.25", "2.3.26", "2.3.27", "2.3.28", "2.3.29",
        "2.3.30", "2.3.31", "2.3.32", "2.3.33", "2.3.34", "2.3.35", "2.3.36", "2.3.37", "2.3.38",
        "2.3.39", "2.3.40", "2.3.41", "2.3.42", "2.3.43", "2.3.44", "2.3.45", "2.3.46", "2.3.47",
        "2.3.48", "2.3.49", "2.3.50", "!2.4.0", "2.4.1", "2.4.2", "2.4.3", "2.4.4", "2.4.5",
        "2.4.6", "2.4.7", "2.4.8", "2.4.9", "2.4.10", // "2.4.11-dontuse",
        "2.4.12", "2.4.13", "2.4.14", "2.4.15", "2.4.16", "2.4.17", "2.4.18", "2.4.19", "2.4.20",
        "2.4.21", "2.4.22", "2.4.23", "2.4.24", "2.4.25", "2.4.26", "2.4.27", "2.4.28", "2.4.29",
        "2.4.30", "2.4.31", "2.4.32", "2.4.33", "2.4.34", "2.4.35", "2.4.36", "2.4.37", "!2.5.0",
        "2.5.1", "2.5.2", "2.5.3", "2.5.4", "2.5.5", "2.5.6", "2.5.7", "2.5.8", "2.5.9", "2.5.10",
        "2.5.11", "2.5.12", "2.5.13", "2.5.14", "2.5.15", "2.5.16", "2.5.17", "2.5.18", "2.5.19",
        "2.5.20", "2.5.21", "2.5.22", "2.5.23", "2.5.24", "2.5.25", "2.5.26", "2.5.27", "2.5.28",
        "2.5.29", "2.5.30", "2.5.31", "2.5.32", "2.5.33", "2.5.34", "2.5.35", "2.5.36", "2.5.37",
        "2.5.38", "2.5.39", "2.5.40", "2.5.41", "2.5.42", "2.5.43", "2.5.44", "2.5.45", "2.5.46",
        "2.5.47", "2.5.48", "2.5.49", "2.5.50", "2.5.51", "2.5.52", "2.5.53", "2.5.54", "2.5.55",
        "2.5.56", "2.5.57", "2.5.58", "2.5.59", "2.5.60", "2.5.61", "2.5.62", "2.5.63", "2.5.64",
        "2.5.65", "2.5.66", "2.5.67", "2.5.68", "2.5.69", "2.5.70", "2.5.71", "2.5.72", "2.5.73",
        "2.5.74", "2.5.75", "!2.6.0", "!2.6.1", "!2.6.2", "!2.6.3", "!2.6.4", "!2.6.5", "!2.6.6",
        "!2.6.7", "!2.6.8", "!2.6.9", "!2.6.10", "!2.6.11",
    ]
}

#[derive(Debug, Clone, Copy)]
pub struct KernelVersion {
    /// If this version is important.
    pub important: bool,
    raw: &'static str,
}

impl KernelVersion {
    pub fn new(mut raw: &'static str) -> KernelVersion {
        let mut important = false;

        if raw.starts_with('!') {
            important = true;
            raw = &raw[1..];
        }

        KernelVersion { important, raw }
    }

    /// Get the downloadable URL for the given kernel version.
    pub fn tar_gz_url(&self) -> Result<String, Error> {
        let version = self.raw.trim_start_matches('!');

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

        Ok(format!(
            "{base}/v{major}.{minor}/{name}.tar.gz",
            base = URL_BASE,
            major = major,
            minor = minor,
            name = name,
        ))
    }
}

impl fmt::Display for KernelVersion {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        self.raw.fmt(fmt)
    }
}

#[derive(Debug, Clone)]
pub struct CachedKernel {
    pub version: KernelVersion,
    pub path: PathBuf,
}

/// Download the archives of the listed versions in parallel.
pub fn download_old_kernels(
    root: &Path,
    versions: &[KernelVersion],
    verify: bool,
) -> Result<Vec<CachedKernel>, Error> {
    use rayon::prelude::*;
    use rayon::ThreadPoolBuilder;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let pool = ThreadPoolBuilder::new()
        .num_threads(8)
        .build()
        .map_err(|e| format!("failed to build thread pool: {}", e))?;

    let total = versions.len();
    let index_atomic = AtomicUsize::new(0);

    let results = pool.install(|| {
        let results = versions.par_iter().map(|version| {
            download_archive(
                index_atomic.fetch_add(1, Ordering::SeqCst),
                total,
                root,
                *version,
                verify,
            )
        });

        results.collect::<Result<Vec<CachedKernel>, Error>>()
    });

    return Ok(results?);

    /// Download the specified archive.
    fn download_archive(
        index: usize,
        total: usize,
        root: &Path,
        version: KernelVersion,
        verify: bool,
    ) -> Result<CachedKernel, Error> {
        let path = root.join(format!("linux-{}.tar.gz", version));

        // use existing path if it already exists.
        if path.is_file() {
            let ok = if verify {
                match test_archive(&path) {
                    None => true,
                    Some(e) => {
                        warn!("ignoring bad archive: {}: {}", path.display(), e);
                        fs::remove_file(&path)
                            .map_err(|e| format!("failed to remove: {}: {}", path.display(), e))?;
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

        let mut res =
            reqwest::get(&url).map_err(|e| format!("failed to get url: {}: {}", url, e))?;

        if !res.status().is_success() {
            return Err(format!("failed to download: {}: {}", url, res.status()).into());
        }

        let mut buf = Vec::new();

        res.copy_to(&mut buf)
            .map_err(|e| format!("failed to copy tarball to memory: {}", e))?;

        if let Some(e) = test_reader_archive(Cursor::new(&buf)) {
            return Err(format!(
                "test on downloaded archive failed: {}: {}",
                path.display(),
                e
            ).into());
        }

        let mut out = fs::File::create(&path)
            .map_err(|e| format!("failed to open file: {}: {}", path.display(), e))?;

        out.write_all(&buf)
            .map_err(|e| format!("failed to write file: {}: {}", path.display(), e))?;
        out.sync_all()
            .map_err(|e| format!("failed to sync: {}: {}", path.display(), e))?;

        Ok(CachedKernel { version, path })
    }

    /// Test that the given path is a proper archive.
    ///
    /// Returns a reason string describing what's wrong with the archive if it's not OK.
    /// Otherwise, returns `None`.
    fn test_archive(path: &Path) -> Option<String> {
        let f = match fs::File::open(path) {
            Err(e) => return Some(format!("failed to open archive: {}", e)),
            Ok(f) => f,
        };

        test_reader_archive(f)
    }

    /// Test that the reader archive is OK.
    fn test_reader_archive(reader: impl Read) -> Option<String> {
        use flate2::read::GzDecoder;
        use tar::Archive;

        let mut a = Archive::new(GzDecoder::new(reader));

        let entries = match a.entries() {
            Err(e) => return Some(format!("failed to list tar entries: {}", e)),
            Ok(entries) => entries,
        };

        for entry in entries {
            let entry = match entry {
                Err(e) => return Some(format!("bad entry: {}", e)),
                Ok(entry) => entry,
            };

            if let Err(e) = entry.path() {
                return Some(format!("bad entry: {}", e));
            }
        }

        None
    }
}
