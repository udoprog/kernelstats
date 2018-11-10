//! Calculate code statistics for the linux kernel.
#![deny(missing_docs)]

use clap::{App, Arg};
use kernelstats::error::Error;
use kernelstats::git::Git;
use kernelstats::kernels::{self, KernelRelease, Kernels};
use log::{info, warn};
use rayon::iter::IntoParallelIterator;
use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::ops;
use std::path::{Path, PathBuf};
use std::str;
use tokei::{FileAccess, Language, LanguageType};

fn tokei_process<'a>(
    files: impl IntoParallelIterator<Item = impl Send + FileAccess<'a>>,
) -> Result<BTreeMap<LanguageType, Language>, Error> {
    use rayon::prelude::*;

    let iter = files.into_par_iter().map(|file_access| {
        match LanguageType::parse(file_access, None) {
            Ok(res) => Ok(res),
            Err(e) => Err((e, file_access)),
        }
    });

    let mut out = BTreeMap::new();

    for res in iter.collect::<Vec<_>>() {
        match res {
            Ok(Some((ty, s))) => {
                out.entry(ty).or_insert_with(Language::new).add_stat(s);
            }
            Ok(None) => {
                // NB: could not detect language..
            }
            Err((e, file_access)) => {
                warn!("error processing file: {}: {}", file_access.name(), e);
            }
        }
    }

    Ok(out)
}

fn tokei_tar_gz(
    tar: &Path,
    encoding: Option<&str>,
) -> Result<BTreeMap<LanguageType, Language>, Error> {
    use encoding_rs_io::DecodeReaderBytesBuilder;
    use flate2::read::GzDecoder;
    use std::borrow::Cow;
    use std::io::{self, Read};
    use tar::Archive;

    let file = fs::File::open(tar).map_err(|e| format!("failed to open file: {}", e))?;
    let mut ar = Archive::new(GzDecoder::new(file));

    let entries = ar
        .entries()
        .map_err(|e| format!("failed to get entries: {}", e))?;

    let mut files = Vec::new();

    let mut content = Vec::new();

    for entry in entries {
        let entry = entry.map_err(|e| format!("bad entry: {}", e))?;

        // NB: need to skip first part of the component due to code existing in a nested directory.
        let path = {
            let path = entry
                .header()
                .path()
                .map_err(|e| format!("failed to get path for entry: {}", e))?;

            let mut components = path.components();

            components.next();
            components.as_path().to_owned()
        };

        let encoding = encoding
            .map(|s| s.as_bytes())
            .and_then(encoding_rs::Encoding::for_label)
            .unwrap_or(encoding_rs::UTF_8);

        let mut decoder = DecodeReaderBytesBuilder::new();
        decoder.encoding(Some(encoding));
        let mut entry = decoder.build(entry);

        if let Err(e) = entry.read_to_end(&mut content) {
            warn!("could not read entry: {}: {}", path.display(), e);
            // return Err(format!("could not read entry: {}: {}", path.display(), e).into());
        }

        files.push(TarEntry(path, content.clone()));
        content.clear();
    }

    let files = files.iter().collect::<Vec<_>>();
    return tokei_process(files);

    struct TarEntry(PathBuf, Vec<u8>);

    impl<'a> FileAccess<'a> for &'a TarEntry {
        type Reader = io::Cursor<&'a [u8]>;

        fn open(self) -> io::Result<Self::Reader> {
            Ok(io::Cursor::new(&self.1))
        }

        fn name(self) -> Cow<'a, str> {
            self.0.to_string_lossy()
        }

        fn file_name(self) -> Option<Cow<'a, str>> {
            match self.0.file_name() {
                Some(filename_os) => Some(Cow::from(filename_os.to_string_lossy().to_lowercase())),
                None => None,
            }
        }

        fn extension(self) -> Option<Cow<'a, str>> {
            match self.0.extension() {
                Some(extension_os) => {
                    Some(Cow::from(extension_os.to_string_lossy().to_lowercase()))
                }
                None => None,
            }
        }
    }
}

/// Call tokei on the given path and get statistics.
fn tokei(dir: &Path) -> Result<BTreeMap<LanguageType, Language>, Error> {
    use ignore::Walk;

    let mut files = Vec::new();

    for result in Walk::new(dir) {
        let result = result.map_err(|e| format!("failed to get file entry: {}", e))?;
        let p = result.path();

        let name = p
            .strip_prefix(dir)
            .map_err(|e| format!("failed to strip prefix: {}", e))?
            .to_string_lossy()
            .to_string();

        files.push((format!("./{}", name), p.to_owned()));
    }

    let files = files
        .iter()
        .map(|(name, f)| f.as_path().with_name(&name))
        .collect::<Vec<_>>();

    return tokei_process(files);
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
    all: BTreeMap<LanguageType, Language>,
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
    Archive {
        /// The version of the archive kernel, in `v{major}.{minor}` format.
        version: String,
        /// Path to the archive kernel.
        path: &'a Path,
        /// The release this kernel belongs to.
        release: &'a KernelRelease,
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
            Kernel::Archive { ref version, .. } => version.as_str(),
            Kernel::Git { ref tag, .. } => tag.as_str(),
        }
    }

    /// Analyze the given kernel.
    pub fn analyze(self) -> Result<Output, Error> {
        match self {
            Kernel::Archive {
                version,
                path,
                release,
            } => {
                let encoding = release.encoding.as_ref().map(|e| e.as_str());
                let mut output = Output::new(version.to_string());
                output.all = tokei_tar_gz(path, encoding)?;
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
        queue.push(Kernel::Archive {
            release: kernel.release,
            version: format!("v{}", kernel.release.version),
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

        let output = q.analyze()?;

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
