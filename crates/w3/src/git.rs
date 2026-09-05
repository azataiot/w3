use std::path::Path;
use std::process::Command;

use crate::{Error, Worktree, parse_porcelain};

pub fn list(repo: &Path) -> Result<Vec<Worktree>, Error> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["worktree", "list", "--porcelain", "-z"])
        .output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(Error::Git(stderr));
    }
    parse_porcelain(&output.stdout)
}
