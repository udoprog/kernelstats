//! Calculate code statistics for the linux kernel.
#![deny(missing_docs)]

use clap::{App, Arg};
use kernelstats::error::Error;
use kernelstats::git::Git;
use kernelstats::kernels::{self, Kernels};
use log::info;
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::ops;
use std::path::{Path, PathBuf};
use std::process;
use std::str;

/// Call tokei on the given path and get statistics.
fn tokei(dir: &Path) -> Result<HashMap<String, LanguageStats>, Error> {
    let out = process::Command::new("tokei")
        .current_dir(dir)
        .args(&["-o", "json"])
        .output()
        .map_err(|e| format!("failed to call tokei: {}", e))?;

    if !out.status.success() {
        let out = str::from_utf8(&out.stderr).map_err(|_| "tokei stderr is not valid UTF-8")?;
        return Err(format!("git error: {}", out).into());
    }

    let stdout = str::from_utf8(&out.stdout).map_err(|_| "tokei stdout is not valid UTF-8")?;
    Ok(serde_json::from_str(&stdout)
        .map_err(|e| format!("failed to deserialize tokei output: {}", e))?)
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Stat {
    blanks: u64,
    code: u64,
    comments: u64,
    lines: u64,
    name: PathBuf,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct LanguageStats {
    blanks: u64,
    code: u64,
    comments: u64,
    lines: u64,
    stats: Vec<Stat>,
}

impl ops::AddAssign for LanguageStats {
    fn add_assign(&mut self, other: LanguageStats) {
        self.blanks += other.blanks;
        self.code += other.code;
        self.comments += other.comments;
        self.lines += other.lines;
        self.stats.extend(other.stats);
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
    pub fn analyze(self, work_dir: &Path) -> Result<Output, Error> {
        match self {
            Kernel::Cached { version, path } => {
                use flate2::read::GzDecoder;
                use tar::Archive;

                let work_dir = work_dir.join(format!("linux-{}", version));

                if !work_dir.is_dir() {
                    let f = fs::File::open(path).map_err(|e| {
                        format!("failed to open cached archive: {}: {}", path.display(), e)
                    })?;
                    let mut a = Archive::new(GzDecoder::new(f));

                    a.unpack(&work_dir).map_err(|e| {
                        format!("failed to unpack archive: {}: {}", path.display(), e)
                    })?;
                }

                let e = fs::read_dir(&work_dir)
                    .map_err(|e| format!("failed to read directory: {}: {}", work_dir.display(), e))
                    .and_then(|mut e| {
                        e.next()
                            .ok_or_else(|| format!("no sub-directory in: {}", work_dir.display()))
                    })?;

                let output_dir = e
                    .map_err(|e| format!("no entry: {}: {}", work_dir.display(), e))?
                    .path();

                if !output_dir.is_dir() {
                    return Err(format!("missing linux directory: {}", output_dir.display()).into());
                }

                let mut output = Output::new(version.to_string());
                output.all = tokei(&output_dir)?;

                fs::remove_dir_all(&work_dir)
                    .map_err(|e| format!("failed to remove dir: {}: {}", work_dir.display(), e))?;
                Ok(output)
            }
            Kernel::Git { tag, git } => {
                info!("building statistics for release: {}", tag);
                git.checkout_hard(&tag)?;

                let mut output = Output::new(tag);
                output.all = tokei(git.repo)?;
                Ok(output)
            }
        }
    }
}

fn app() -> App<'static, 'static> {
    App::new("kernelstats")
        .version("0.0.1")
        .author("John-John Tedro <udoprog@tedro.se>")
        .about("Calculates statistics across kernel releases.")
        .arg(
            Arg::with_name("verify")
                .long("verify")
                .help("Verify that all kernels are available."),
        ).arg(
            Arg::with_name("all")
                .long("all")
                .help("Build all kernel versions, not just important."),
        ).arg(
            Arg::with_name("cache")
                .long("cache")
                .value_name("DIR")
                .help("Sets the path to the cache directory.")
                .takes_value(true),
        ).arg(
            Arg::with_name("work")
                .long("work")
                .value_name("DIR")
                .help("Sets the path to the work directory.")
                .takes_value(true),
        ).arg(
            Arg::with_name("stats")
                .long("stats")
                .value_name("DIR")
                .help("Directory to store statistics in.")
                .takes_value(true),
        ).arg(
            Arg::with_name("kernel-git")
                .long("kernel-git")
                .value_name("DIR")
                .help("Sets the path to a kernel git directory.")
                .takes_value(true),
        )
}

fn main() -> Result<(), Error> {
    pretty_env_logger::init();

    let matches = app().get_matches();

    let kernel_git_dir = matches.value_of("kernel-git").map(Path::new);
    let verify = matches.is_present("verify");
    let all = matches.is_present("all");

    let cache_dir = matches
        .value_of("cache")
        .map(Path::new)
        .unwrap_or_else(|| Path::new("cache"));

    let work_dir = matches
        .value_of("work")
        .map(Path::new)
        .unwrap_or_else(|| Path::new("work"));

    let stats_dir = matches
        .value_of("stats")
        .map(Path::new)
        .unwrap_or_else(|| Path::new("stats"));

    use std::io::Write;

    let mut a = env::args();
    a.next();

    if !cache_dir.is_dir() {
        fs::create_dir_all(cache_dir).map_err(|e| {
            format!(
                "failed to create cache directory: {}: {}",
                cache_dir.display(),
                e
            )
        })?;
    }

    let Kernels { mut releases } = kernels::kernels()?;

    if !all {
        releases = releases.into_iter().filter(|v| v.important).collect();
    }

    let mut queue = Vec::new();

    info!("downloading old kernels to: {}", cache_dir.display());
    let cached = kernels::download_old_kernels(cache_dir, &releases, verify)?;

    for kernel in &cached {
        queue.push(Kernel::Cached {
            version: format!("v{}", kernel.version),
            path: &kernel.path,
        });

        info!("downloaded: {}", kernel.path.display());
    }

    if let Some(kernel_git_dir) = kernel_git_dir {
        if !kernel_git_dir.is_dir() {
            return Err("missing kernel directory".into());
        }

        let git = Git::new(&kernel_git_dir);

        for tag in git.tags()? {
            match tag.as_str() {
                // NB: not a commit
                "v2.6.11" => continue,
                tag if tag.ends_with("-tree") => continue,
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

    if verify {
        for q in queue {
            info!("verified: {:?}", q);
        }

        return Ok(());
    }

    if !stats_dir.is_dir() {
        fs::create_dir_all(stats_dir).map_err(|e| {
            format!(
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
            .map_err(|e| format!("failed to create output file: {}: {}", p.display(), e))?;

        let mut o = GzEncoder::new(o, Compression::default());

        serde_json::to_writer(&mut o, &output)
            .map_err(|e| format!("failed to serialize: {}", e))?;
        writeln!(o, "")?;

        o.flush()
            .map_err(|e| format!("failed to sync: {}: {}", p.display(), e))?;
    }

    Ok(())
}
