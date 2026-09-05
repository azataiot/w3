use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use crate::worktree::path_from_bytes;
use crate::{Error, Worktree, parse_porcelain};

pub enum Branch<'a> {
    New(&'a str),
    Existing(&'a str),
}

pub enum Changes {
    Staged,
    Unstaged,
}

pub enum Apply {
    Index,
    WorkingTree,
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

pub fn branches(repo: &Path) -> Result<Vec<String>, Error> {
    let stdout = run(git(repo).args(["for-each-ref", "--format=%(refname:short)", "refs/heads"]))?;
    Ok(String::from_utf8_lossy(&stdout)
        .lines()
        .map(str::to_string)
        .collect())
}

pub fn included_files(repo: &Path, include: &Path) -> Result<Vec<PathBuf>, Error> {
    if !repo.join(include).is_file() {
        return Ok(Vec::new());
    }
    let ignored = ls_files(repo, &["--ignored", "--exclude-standard"])?;
    let exclude_from = format!("--exclude-from={}", include.display());
    let matching = ls_files(repo, &["--ignored", &exclude_from])?;
    Ok(matching.intersection(&ignored).cloned().collect())
}

pub fn untracked_files(worktree: &Path) -> Result<Vec<PathBuf>, Error> {
    Ok(ls_files(worktree, &["--exclude-standard"])?
        .into_iter()
        .collect())
}

pub fn changes(worktree: &Path, changes: Changes) -> Result<Vec<u8>, Error> {
    let mut command = git(worktree);
    command.args([
        "diff",
        "--binary",
        "--no-color",
        "--no-ext-diff",
        "--ignore-submodules=all",
        "--src-prefix=a/",
        "--dst-prefix=b/",
    ]);
    if let Changes::Staged = changes {
        command.arg("--cached");
    }
    run(&mut command)
}

pub fn apply(worktree: &Path, patch: &[u8], apply: Apply) -> Result<(), Error> {
    if patch.is_empty() {
        return Ok(());
    }
    let mut command = git(worktree);
    command.arg("apply");
    if let Apply::Index = apply {
        command.arg("--index");
    }
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn()?;
    child
        .stdin
        .take()
        .expect("stdin is piped")
        .write_all(patch)?;
    finish(child.wait_with_output()?).map(drop)
}

pub fn remove(repo: &Path, path: &Path) -> Result<(), Error> {
    run(git(repo).args(["worktree", "remove", "--force"]).arg(path)).map(drop)
}

pub fn delete_branch(repo: &Path, name: &str) -> Result<(), Error> {
    run(git(repo).args(["branch", "-q", "-D", name])).map(drop)
}

fn ls_files(repo: &Path, flags: &[&str]) -> Result<BTreeSet<PathBuf>, Error> {
    let stdout = run(git(repo).args(["ls-files", "-z", "--others"]).args(flags))?;
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
    finish(command.output()?)
}

fn finish(output: Output) -> Result<Vec<u8>, Error> {
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(Error::Git(stderr));
    }
    Ok(output.stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        let ok = |args: &[&str]| {
            assert!(
                Command::new("git")
                    .arg("-C")
                    .arg(tmp.path())
                    .args(["-c", "user.name=w3", "-c", "user.email=w3@example.com"])
                    .args(args)
                    .output()
                    .unwrap()
                    .status
                    .success(),
                "git {args:?}"
            );
        };
        ok(&["init", "-q"]);
        ok(&["commit", "-q", "--allow-empty", "-m", "init"]);
        tmp
    }

    #[test]
    fn a_clean_tree_has_empty_patches_and_apply_skips_them() {
        let tmp = repo();
        assert!(changes(tmp.path(), Changes::Staged).unwrap().is_empty());
        assert!(changes(tmp.path(), Changes::Unstaged).unwrap().is_empty());
        apply(tmp.path(), b"", Apply::Index).unwrap();
        apply(tmp.path(), b"", Apply::WorkingTree).unwrap();
    }

    #[test]
    fn untracked_files_lists_only_files_git_would_show() {
        let tmp = repo();
        std::fs::write(tmp.path().join(".gitignore"), "ignored.txt\n").unwrap();
        std::fs::write(tmp.path().join("ignored.txt"), "x").unwrap();
        std::fs::create_dir(tmp.path().join("dir")).unwrap();
        std::fs::write(tmp.path().join("dir/new.txt"), "x").unwrap();
        assert_eq!(
            untracked_files(tmp.path()).unwrap(),
            [PathBuf::from(".gitignore"), PathBuf::from("dir/new.txt")]
        );
    }
}
