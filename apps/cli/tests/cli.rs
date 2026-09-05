use std::path::Path;
use std::process::Command;

const W3: &str = env!("CARGO_BIN_EXE_w3");

fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
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
        .output()
        .unwrap();
    assert!(output.status.success(), "git {args:?} failed");
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

#[test]
fn list_prints_one_aligned_line_per_worktree() {
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
    let head = git(&repo, &["rev-parse", "--short=8", "HEAD"]);

    let output = Command::new(W3)
        .arg("list")
        .current_dir(&repo)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 2, "{stdout}");
    assert!(
        lines[0].ends_with(&format!("{head} [main]")),
        "{}",
        lines[0]
    );
    assert!(
        lines[1].ends_with(&format!("{head} [feature] locked")),
        "{}",
        lines[1]
    );
    assert_eq!(
        lines[0].find(&head),
        lines[1].find(&head),
        "head column must align"
    );
}

#[test]
fn list_outside_a_repo_fails_with_one_line() {
    let tmp = tempfile::tempdir().unwrap();

    let output = Command::new(W3)
        .arg("list")
        .current_dir(tmp.path())
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert!(stderr.starts_with("Error: "), "{stderr}");
}
