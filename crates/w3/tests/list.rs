use std::path::Path;
use std::process::Command;

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=w3", "-c", "user.email=w3@example.com"])
        .args([
            "-c",
            "init.defaultBranch=main",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?} failed");
}

#[test]
fn lists_main_and_locked_worktree_from_real_git() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-q"]);
    git(&repo, &["commit", "-q", "--allow-empty", "-m", "init"]);
    let feature = tmp.path().join("feature");
    let feature_arg = feature.to_str().unwrap();
    git(
        &repo,
        &["worktree", "add", "-q", "-b", "feature", feature_arg],
    );
    git(
        &repo,
        &["worktree", "lock", "--reason", "busy", feature_arg],
    );

    let worktrees = w3::list(&repo).unwrap();

    assert_eq!(worktrees.len(), 2);
    assert_eq!(
        worktrees[0].path.canonicalize().unwrap(),
        repo.canonicalize().unwrap()
    );
    assert_eq!(worktrees[0].branch.as_deref(), Some("main"));
    assert_eq!(worktrees[0].locked, None);
    assert_eq!(
        worktrees[1].path.canonicalize().unwrap(),
        feature.canonicalize().unwrap()
    );
    assert_eq!(worktrees[1].branch.as_deref(), Some("feature"));
    assert_eq!(worktrees[1].locked.as_deref(), Some("busy"));
    assert_eq!(worktrees[0].head, worktrees[1].head);
}

#[test]
fn not_a_repo_is_a_git_error() {
    let tmp = tempfile::tempdir().unwrap();
    let err = w3::list(tmp.path()).unwrap_err();
    assert!(matches!(err, w3::Error::Git(_)));
    assert!(!err.to_string().is_empty());
}

fn repo_with_commit(tmp: &Path) -> std::path::PathBuf {
    let repo = tmp.join("repo");
    std::fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-q"]);
    git(&repo, &["commit", "-q", "--allow-empty", "-m", "init"]);
    repo
}

fn rev_parse(dir: &Path, rev: &str) -> String {
    let output = Command::new("git")
        .args(["-C", dir.to_str().unwrap(), "rev-parse", rev])
        .output()
        .unwrap();
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

#[test]
fn add_creates_a_new_branch_from_head() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = repo_with_commit(tmp.path());
    let target = tmp.path().join("feature");

    w3::add(&repo, &target, w3::Branch::New("feature"), None).unwrap();

    let worktrees = w3::list(&repo).unwrap();
    assert_eq!(worktrees.len(), 2);
    assert_eq!(worktrees[1].branch.as_deref(), Some("feature"));
    assert_eq!(worktrees[1].head, rev_parse(&repo, "HEAD"));
    assert!(target.join(".git").exists());
}

#[test]
fn add_creates_a_new_branch_from_a_given_base() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = repo_with_commit(tmp.path());
    let first = rev_parse(&repo, "HEAD");
    git(&repo, &["commit", "-q", "--allow-empty", "-m", "second"]);
    let target = tmp.path().join("old");

    w3::add(&repo, &target, w3::Branch::New("old"), Some(&first)).unwrap();

    let worktrees = w3::list(&repo).unwrap();
    assert_eq!(worktrees[1].head, first);
}

#[test]
fn add_checks_out_an_existing_branch() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = repo_with_commit(tmp.path());
    git(&repo, &["branch", "existing"]);
    let target = tmp.path().join("existing");

    w3::add(&repo, &target, w3::Branch::Existing("existing"), None).unwrap();

    let worktrees = w3::list(&repo).unwrap();
    assert_eq!(worktrees[1].branch.as_deref(), Some("existing"));
}

#[test]
fn add_refusal_is_a_git_error() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = repo_with_commit(tmp.path());
    let target = tmp.path().join("dup");

    let err = w3::add(&repo, &target, w3::Branch::New("main"), None).unwrap_err();

    assert!(matches!(err, w3::Error::Git(_)));
    assert!(err.to_string().contains("main"), "{err}");
    assert!(!target.exists());
}

fn include_fixture(tmp: &Path) -> std::path::PathBuf {
    let repo = repo_with_commit(tmp);
    std::fs::write(repo.join("keep.txt"), "tracked\n").unwrap();
    git(&repo, &["add", "keep.txt"]);
    git(&repo, &["commit", "-q", "-m", "keep"]);
    std::fs::write(repo.join(".env"), "secret\n").unwrap();
    std::fs::write(repo.join("other.log"), "noise\n").unwrap();
    std::fs::write(repo.join("real.txt"), "real\n").unwrap();
    std::os::unix::fs::symlink("real.txt", repo.join("link")).unwrap();
    std::fs::create_dir_all(repo.join(".claude/skills")).unwrap();
    std::fs::write(repo.join(".claude/skills/a.md"), "skill\n").unwrap();
    std::fs::write(
        repo.join(".gitignore"),
        ".env\n*.log\nreal.txt\nlink\n.claude/\n.worktreeinclude\n",
    )
    .unwrap();
    std::fs::write(
        repo.join(".worktreeinclude"),
        "/.env\n.claude/skills/\nkeep.txt\nlink\n",
    )
    .unwrap();
    repo
}

#[test]
fn included_files_are_the_gitignored_matches_only() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = include_fixture(tmp.path());

    let files = w3::included_files(&repo, Path::new(".worktreeinclude")).unwrap();

    let expected: Vec<std::path::PathBuf> = [".claude/skills/a.md", ".env", "link"]
        .iter()
        .map(Into::into)
        .collect();
    assert_eq!(files, expected);
}

#[test]
fn a_missing_include_file_copies_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = include_fixture(tmp.path());

    let files = w3::included_files(&repo, Path::new("nope")).unwrap();

    assert!(files.is_empty());
}

#[test]
fn branches_are_the_local_heads_in_git_order() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = repo_with_commit(tmp.path());
    git(&repo, &["branch", "zeta"]);
    git(&repo, &["branch", "alpha/one"]);
    assert_eq!(w3::branches(&repo).unwrap(), ["alpha/one", "main", "zeta"]);
}

#[test]
fn a_repo_without_commits_has_no_branches() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("empty");
    std::fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-q"]);
    assert_eq!(w3::branches(&repo).unwrap(), Vec::<String>::new());
}
