use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::worktree::path_from_bytes;
use crate::{Error, Worktree, parse_porcelain};

pub enum Branch<'a> {
    New(&'a str),
    Existing(&'a str),
}

pub fn list(repo: &Path) -> Result<Vec<Worktree>, Error> {
    let stdout = run(git(repo).args(["worktree", "list", "--porcelain", "-z"]))?;
    parse_porcelain(&stdout)
}

pub fn add(repo: &Path, path: &Path, branch: Branch, base: Option<&str>) -> Result<(), Error> {
    let mut command = git(repo);
    command.args(["worktree", "add", "-q"]).arg(path);
    match branch {
        Branch::New(name) => {
            command.args(["-b", name]);
            if let Some(base) = base {
                command.arg(base);
            }
        }
        Branch::Existing(name) => {
            command.arg(name);
        }
    }
    run(&mut command).map(drop)
}

pub fn included_files(repo: &Path, include: &Path) -> Result<Vec<PathBuf>, Error> {
    if !repo.join(include).is_file() {
        return Ok(Vec::new());
    }
    let ignored = ls_files(repo, "--exclude-standard")?;
    let exclude_from = format!("--exclude-from={}", include.display());
    let matching = ls_files(repo, &exclude_from)?;
    Ok(matching.intersection(&ignored).cloned().collect())
}

fn ls_files(repo: &Path, exclude: &str) -> Result<BTreeSet<PathBuf>, Error> {
    let stdout = run(git(repo).args(["ls-files", "-z", "--others", "--ignored", exclude]))?;
    Ok(stdout
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
        .map(path_from_bytes)
        .collect())
}

fn git(repo: &Path) -> Command {
    let mut command = Command::new("git");
    command.arg("-C").arg(repo);
    command
}

fn run(command: &mut Command) -> Result<Vec<u8>, Error> {
    let output = command.output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(Error::Git(stderr));
    }
    Ok(output.stdout)
}
