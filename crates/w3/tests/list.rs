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
