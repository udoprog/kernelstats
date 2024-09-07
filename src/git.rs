use std::ffi::OsStr;
use std::path::Path;
use std::process;
use std::str;

use anyhow::{anyhow, ensure, Context, Result};

/// Interact with a git repository.
#[derive(Debug, Clone, Copy)]
pub struct Git<'a> {
    pub repo: &'a Path,
}

impl<'a> Git<'a> {
    pub fn new(repo: &'a Path) -> Git {
        Git { repo }
    }

    /// Call git with the given arguments inheriting stdout.
    fn git_run(&self, args: impl IntoIterator<Item: AsRef<OsStr>>) -> Result<()> {
        let mut command = process::Command::new("git");
        command.current_dir(&self.repo);
        command.args(args);

        log_command(&command)?;

        let status = command.status().context("git: failed to call")?;
        ensure!(status.success(), "git call failed: {status}");
        Ok(())
    }

    /// Call git with the given arguments.
    fn git(&self, args: impl IntoIterator<Item: AsRef<OsStr>>) -> Result<String> {
        let mut command = process::Command::new("git");
        command.current_dir(&self.repo);
        command.args(args);

        log_command(&command)?;

        let out = command.output().context("git: failed to call")?;

        if !out.status.success() {
            let out = str::from_utf8(&out.stderr).context("git stderr is not valid utf-8")?;
            return Err(anyhow!("git error: {out}").into());
        }

        let out =
            str::from_utf8(&out.stdout).map_err(|_| anyhow!("git stdout is not valid utf-8"))?;
        Ok(out.to_string())
    }

    /// Get all git tags, sorted by commiter date.
    pub fn ls_remote(&self, remote: &str) -> Result<Vec<(String, String)>> {
        let mut results = Vec::new();

        let out = self.git(&["ls-remote", remote])?;

        for line in out.split('\n') {
            let Some((hash, reference)) = line.trim().split_once('\t') else {
                continue;
            };

            results.push((hash.to_string(), reference.to_string()));
        }

        Ok(results)
    }

    /// Initialize a repo.
    pub fn init(&self, remote: &str) -> Result<()> {
        self.git_run(&[OsStr::new("init"), self.repo.as_os_str()])?;
        self.git_run(&["remote", "add", "origin", remote])?;
        Ok(())
    }

    /// Fetch a hash.
    pub fn fetch(&self, refspecs: impl IntoIterator<Item: AsRef<str>>) -> Result<()> {
        let mut command = vec![
            String::from("fetch"),
            String::from("--tags"),
            String::from("--depth"),
            String::from("1"),
            String::from("origin"),
        ];

        for refspec in refspecs {
            command.push(refspec.as_ref().to_string());
        }

        self.git_run(&command)?;
        Ok(())
    }

    /// Get all git tags, sorted by commiter date.
    pub fn tags(&self) -> Result<Vec<String>> {
        let out = self.git(&["tag", "--sort=taggerdate"])?;

        Ok(out
            .split("\n")
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect())
    }

    pub fn checkout_hard(&self, reference: &str) -> Result<()> {
        self.git_run(&["clean", "-fdx"])?;
        self.git_run(&["checkout", reference])?;
        Ok(())
    }
}

pub(crate) fn log_command(command: &process::Command) -> Result<()> {
    use std::fmt::Write;

    let name = command.get_program().to_string_lossy();
    let mut args = String::new();

    for arg in command.get_args() {
        args.push(' ');
        args.push_str(&arg.to_string_lossy());
    }

    if let Some(path) = command.get_current_dir() {
        write!(args, " (in {})", path.display())?;
    }

    log::info!("{name}{args}");
    Ok(())
}
