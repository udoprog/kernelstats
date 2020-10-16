use anyhow::{anyhow, Result};
use std::ffi::OsStr;
use std::path::Path;
use std::process;
use std::str;

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
            .map_err(|e| anyhow!("git: failed to call: {}", e))?;

        if !status.success() {
            return Err(anyhow!("git call failed: {}", status).into());
        }

        Ok(())
    }

    /// Call git with the given arguments.
    fn git<S: AsRef<OsStr>>(&self, args: impl IntoIterator<Item = S>) -> Result<String> {
        let out = process::Command::new("git")
            .current_dir(&self.repo)
            .args(args)
            .output()
            .map_err(|e| anyhow!("git: failed to call: {}", e))?;

        if !out.status.success() {
            let out = str::from_utf8(&out.stderr)
                .map_err(|_| anyhow!("git stderr is not valid UTF-8"))?;
            return Err(anyhow!("git error: {}", out).into());
        }

        let out =
            str::from_utf8(&out.stdout).map_err(|_| anyhow!("git stdout is not valid UTF-8"))?;
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
