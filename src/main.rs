//! Calculate code statistics for the linux kernel.
#![deny(missing_docs)]

use kernelstats::error::Error;
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

#[derive(Debug, Serialize)]
struct Output<'a> {
    tag: &'a str,
    all: HashMap<String, LanguageStats>,
}

impl<'a> Output<'a> {
    pub fn new(tag: &'a str) -> Output {
        Output {
            tag,
            all: Default::default(),
        }
    }
}

fn main() -> Result<(), Error> {
    use std::io::Write;

    let mut a = env::args();
    a.next();

    let kernel_dir = PathBuf::from(a.next().ok_or_else(|| Error::MissingArgument)?);
    let out_file = PathBuf::from(a.next().unwrap_or_else(|| String::from("kernelstats.json")));

    if !kernel_dir.is_dir() {
        return Err("missing kernel directory".into());
    }

    let mut out_file = fs::File::create(&out_file).map_err(|e| {
        format!(
            "failed to create output file: {}: {}",
            out_file.display(),
            e
        )
    })?;
    let kernel_git = kernelstats::git::Git::new(&kernel_dir);

    for tag in kernel_git.tags()? {
        match tag.as_str() {
            // NB: not a commit
            "v2.6.11" => continue,
            tag if tag.ends_with("-tree") => continue,
            // NB: skip release candidates.
            tag if tag.trim_end_matches(char::is_numeric).ends_with("-rc") => {
                println!("skipping release candidate: {}", tag);
                continue;
            }
            _ => {}
        }

        println!("building statistics for release: {}", tag);
        kernel_git.checkout_hard(&tag)?;

        let mut output = Output::new(&tag);
        output.all = tokei(&kernel_dir)?;

        serde_json::to_writer(&mut out_file, &output)
            .map_err(|e| format!("failed to serialize: {}", e))?;
        writeln!(out_file, "")?;
    }

    Ok(())
}
