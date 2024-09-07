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
    fn git_run<S: AsRef<OsStr>>(&self, args: impl IntoIterator<Item = S>) -> Result<()> {
        let status = process::Command::new("git")
            .current_dir(&self.repo)
            .args(args)
            .status()
            .context("git: failed to call")?;

        ensure!(status.success(), "git call failed: {status}");
        Ok(())
    }

    /// Call git with the given arguments.
    fn git<S: AsRef<OsStr>>(&self, args: impl IntoIterator<Item = S>) -> Result<String> {
        let out = process::Command::new("git")
            .current_dir(&self.repo)
            .args(args)
            .output()
            .context("git: failed to call")?;

        if !out.status.success() {
            let out = str::from_utf8(&out.stderr).context("git stderr is not valid utf-8")?;
            return Err(anyhow!("git error: {out}").into());
        }

        let out =
            str::from_utf8(&out.stdout).map_err(|_| anyhow!("git stdout is not valid utf-8"))?;
        Ok(out.to_string())
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
        self.git_run(&["reset", "--hard", "HEAD"])?;
        self.git_run(&["clean", "-fdx"])?;
        self.git_run(&["checkout", reference])?;
        Ok(())
    }
}
