//! Calculate code statistics for the linux kernel.
#![deny(missing_docs)]

use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::Write;
use std::ops;
use std::path::{Path, PathBuf};
use std::process;
use std::str;

use anyhow::{anyhow, Context as _, Result};
use clap::Parser;
use kernelstats::git::Git;
use kernelstats::kernels::{self, Kernels};
use log::info;
use serde_derive::{Deserialize, Serialize};

/// Call tokei on the given path and get statistics.
fn tokei(dir: &Path) -> Result<HashMap<String, LanguageStats>> {
    let out = process::Command::new("tokei")
        .current_dir(dir)
        .args(&["-o", "json"])
        .output()?;

    if !out.status.success() {
        let out = str::from_utf8(&out.stderr)?;
        return Err(anyhow!("git error: {}", out).into());
    }

    let stdout = str::from_utf8(&out.stdout)?;
    Ok(serde_json::from_str(&stdout)?)
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Stats {
    blanks: u64,
    code: u64,
    comments: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Report {
    name: PathBuf,
    stats: Stats,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct LanguageStats {
    blanks: u64,
    code: u64,
    comments: u64,
    reports: Vec<Report>,
}

impl ops::AddAssign for LanguageStats {
    fn add_assign(&mut self, other: LanguageStats) {
        self.blanks += other.blanks;
        self.code += other.code;
        self.comments += other.comments;
        self.reports.extend(other.reports);
    }
}

/// The output of analyzing a single kernel.
#[derive(Debug, Serialize)]
pub struct Output {
    /// The tag that we build for.
    tag: String,
    /// Statistics for all languages.
    all: HashMap<String, LanguageStats>,
}

impl Output {
    /// Construct a new kernel output.
    pub fn new(tag: String) -> Output {
        Output {
            tag,
            all: Default::default(),
        }
    }
}

/// A kernel to build, the path it's
#[derive(Debug, Clone)]
pub enum Kernel<'a> {
    /// A kernel tar.gz that needs to be unpacked.
    Cached {
        /// The version of the cached kernel, in `v{major}.{minor}` format.
        version: String,
        /// Path to the cached kernel.
        path: &'a Path,
    },
    /// A git directory tag.
    Git {
        /// The tag of the kernel.
        tag: String,
        /// The git handle for the kernel.
        git: Git<'a>,
    },
}

impl<'a> Kernel<'a> {
    /// Get the version of the kernel.
    pub fn version(&self) -> &str {
        match *self {
            Kernel::Cached { ref version, .. } => version.as_str(),
            Kernel::Git { ref tag, .. } => tag.as_str(),
        }
    }

    /// Analyze the given kernel.
    pub fn analyze(self, work_dir: &Path) -> Result<Output> {
        match self {
            Kernel::Cached { version, path } => {
                use flate2::read::GzDecoder;
                use tar::Archive;

                let work_dir = work_dir.join(format!("linux-{}", version));

                if !work_dir.is_dir() {
                    let f = fs::File::open(path).map_err(|e| {
                        anyhow!("failed to open cached archive: {}: {}", path.display(), e)
                    })?;
                    let mut a = Archive::new(GzDecoder::new(f));

                    a.unpack(&work_dir)
                        .with_context(|| anyhow!("failed to unpack archive: {}", path.display()))?;
                }

                let e = fs::read_dir(&work_dir)
                    .with_context(|| anyhow!("failed to read directory: {}", work_dir.display()))?
                    .next()
                    .ok_or_else(|| anyhow!("no sub-directory in: {}", work_dir.display()))?;

                let output_dir = e
                    .with_context(|| anyhow!("no entry: {}", work_dir.display()))?
                    .path();

                if !output_dir.is_dir() {
                    return Err(anyhow!("missing linux directory: {}", output_dir.display()).into());
                }

                let mut output = Output::new(version.to_string());
                output.all = tokei(&output_dir).context("running tokei")?;

                fs::remove_dir_all(&work_dir)
                    .map_err(|e| anyhow!("failed to remove dir: {}: {}", work_dir.display(), e))?;
                Ok(output)
            }
            Kernel::Git { tag, git } => {
                info!("building statistics for release: {}", tag);
                git.checkout_hard(&tag)?;

                let mut output = Output::new(tag);
                output.all = tokei(git.repo).context("running tokei")?;
                Ok(output)
            }
        }
    }
}

#[derive(Parser, Debug)]
#[command(
    version,
    about = "Calculates statistics across kernel releases.",
    author = "John-John Tedro <udoprog@tedro.se>"
)]
struct Args {
    /// Verify that all kernels are available.
    #[arg(long)]
    verify: bool,
    /// Build all kernel versions, not just important.
    #[arg(long)]
    all: bool,
    /// Sets the path to the cache directory.
    #[arg(long)]
    cache: Option<PathBuf>,
    /// Sets the path to the work directory.
    #[arg(long)]
    work: Option<PathBuf>,
    /// Directory to store statistics in.
    #[arg(long)]
    stats: Option<PathBuf>,
    /// Sets the path to a kernel git directory.
    #[arg(long)]
    kernel_git: Option<PathBuf>,
    /// How many downloads to perform in parallel.
    #[arg(long, short = 'p')]
    parallelism: Option<usize>,
}

#[tokio::main]
async fn main() -> Result<()> {
    pretty_env_logger::init();

    let args = Args::parse();

    let kernel_git_dir = args.kernel_git.as_deref();
    let cache_dir = args.cache.as_deref().unwrap_or(Path::new("cache"));
    let work_dir = args.work.as_deref().unwrap_or(Path::new("work"));
    let stats_dir = args.stats.as_deref().unwrap_or(Path::new("stats"));
    let parallelism = args.parallelism.unwrap_or(2).max(1);

    let mut a = env::args();
    a.next();

    if !cache_dir.is_dir() {
        fs::create_dir_all(cache_dir).map_err(|e| {
            anyhow!(
                "failed to create cache directory: {}: {}",
                cache_dir.display(),
                e
            )
        })?;
    }

    let Kernels { mut releases } = kernels::kernels()?;

    if !args.all {
        releases = releases.into_iter().filter(|v| v.important).collect();
    }

    let mut queue = Vec::new();

    info!("downloading old kernels to: {}", cache_dir.display());
    let cached =
        kernels::download_old_kernels(cache_dir, &releases, args.verify, parallelism).await?;

    for kernel in &cached {
        queue.push(Kernel::Cached {
            version: format!("v{}", kernel.version),
            path: &kernel.path,
        });

        info!("downloaded: {}", kernel.path.display());
    }

    if let Some(kernel_git_dir) = kernel_git_dir {
        if !kernel_git_dir.is_dir() {
            return Err(anyhow!("missing kernel directory"));
        }

        let git = Git::new(&kernel_git_dir);

        for tag in git.tags()? {
            match tag.as_str() {
                // NB: not a commit
                "v2.6.11" => continue,
                tag if tag.ends_with("-tree") || tag.ends_with("-dontuse") => continue,
                // NB: skip release candidates.
                tag if tag.trim_end_matches(char::is_numeric).ends_with("-rc") => {
                    info!("skipping release candidate: {}", tag);
                    continue;
                }
                _ => {}
            }

            queue.push(Kernel::Git { tag, git });
        }
    }

    if args.verify {
        for q in queue {
            info!("verified: {:?}", q);
        }

        return Ok(());
    }

    if !stats_dir.is_dir() {
        fs::create_dir_all(stats_dir).map_err(|e| {
            anyhow!(
                "failed to create stats directory: {}: {}",
                stats_dir.display(),
                e
            )
        })?;
    }

    for q in queue {
        use flate2::write::GzEncoder;
        use flate2::Compression;

        info!("process: {:?}", q);

        let p = stats_dir.join(format!("linux-{}.json.gz", q.version()));

        if p.is_file() {
            continue;
        }

        let output = q.analyze(&work_dir)?;

        let o = fs::File::create(&p)
            .map_err(|e| anyhow!("failed to create output file: {}: {}", p.display(), e))?;

        let mut o = GzEncoder::new(o, Compression::default());

        serde_json::to_writer(&mut o, &output)
            .map_err(|e| anyhow!("failed to serialize: {}", e))?;
        writeln!(o, "")?;

        o.flush()
            .with_context(|| anyhow!("failed to sync: {}", p.display()))?;
    }

    Ok(())
}
