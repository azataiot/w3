use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const W3: &str = env!("CARGO_BIN_EXE_w3");

struct Fixture {
    tmp: tempfile::TempDir,
    repo: PathBuf,
    feature: PathBuf,
    head: String,
}

impl Fixture {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().canonicalize().unwrap();
        let repo = home.join("repo");
        std::fs::create_dir(&repo).unwrap();
        git(&repo, &["init", "-q"]);
        git(&repo, &["commit", "-q", "--allow-empty", "-m", "init"]);
        let feature = home.join("feature");
        let feature_arg = feature.to_str().unwrap();
        git(
            &repo,
            &["worktree", "add", "-q", "-b", "feature", feature_arg],
        );
        git(
            &repo,
            &["worktree", "lock", "--reason", "busy", feature_arg],
        );
        std::fs::create_dir(home.join("xdg")).unwrap();
        let head = git(&repo, &["rev-parse", "HEAD"]);
        Self {
            tmp,
            repo,
            feature,
            head,
        }
    }

    fn home(&self) -> PathBuf {
        self.tmp.path().canonicalize().unwrap()
    }

    fn write_user_config(&self, text: &str) {
        let dir = self.home().join("xdg/w3");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("config.toml"), text).unwrap();
    }

    fn write_az_toml(&self, text: &str) {
        std::fs::write(self.repo.join("az.toml"), text).unwrap();
    }

    fn run(&self, cwd: &Path, args: &[&str], env: &[(&str, &str)]) -> Output {
        let home = self.home();
        let mut command = Command::new(W3);
        command
            .args(args)
            .current_dir(cwd)
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap())
            .env("HOME", &home)
            .env("XDG_CONFIG_HOME", home.join("xdg"));
        for (name, value) in env {
            command.env(name, value);
        }
        command.output().unwrap()
    }

    fn stdout(&self, cwd: &Path, args: &[&str], env: &[(&str, &str)]) -> String {
        let output = self.run(cwd, args, env);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    }
}

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
fn a_pipe_gets_plain_tsv_with_absolute_paths() {
    let fx = Fixture::new();
    let stdout = fx.stdout(&fx.repo, &["list"], &[]);
    let lines: Vec<Vec<&str>> = stdout
        .lines()
        .map(|line| line.split('\t').collect())
        .collect();
    assert_eq!(lines.len(), 2, "{stdout}");
    assert_eq!(Path::new(lines[0][0]), fx.repo);
    assert_eq!(lines[0][1], &fx.head[..8]);
    assert_eq!(lines[0][2], "main");
    assert_eq!(lines[0][3], "current");
    assert_eq!(Path::new(lines[1][0]), fx.feature);
    assert_eq!(lines[1][2], "feature");
    assert_eq!(lines[1][3], "locked");
}

#[test]
fn a_table_has_a_header_a_marker_and_home_paths() {
    let fx = Fixture::new();
    let stdout = fx.stdout(&fx.repo, &["list", "--format", "table"], &[]);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 3, "{stdout}");
    assert!(lines[0].starts_with("  NAME"), "{}", lines[0]);
    assert!(lines[0].ends_with("PATH"), "{}", lines[0]);
    assert!(lines[1].starts_with("* repo"), "{}", lines[1]);
    assert!(lines[1].ends_with("~/repo"), "{}", lines[1]);
    assert!(lines[2].starts_with("  feature"), "{}", lines[2]);
    assert!(lines[2].contains("locked"), "{}", lines[2]);
    assert_eq!(
        lines[1].find(&fx.head[..8]),
        lines[2].find(&fx.head[..8]),
        "head column must align"
    );
}

#[test]
fn the_marker_follows_the_working_directory() {
    let fx = Fixture::new();
    let stdout = fx.stdout(&fx.feature, &["list", "--format", "table"], &[]);
    let lines: Vec<&str> = stdout.lines().collect();
    assert!(lines[1].starts_with("  repo"), "{}", lines[1]);
    assert!(lines[2].starts_with("* feature"), "{}", lines[2]);
}

#[test]
fn json_has_the_seven_fields_in_order_with_the_full_sha() {
    let fx = Fixture::new();
    let stdout = fx.stdout(&fx.repo, &["list", "--format", "json"], &[]);
    assert_eq!(stdout.lines().count(), 1, "{stdout}");
    let envelope: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let rows: Vec<serde_json::Map<String, serde_json::Value>> =
        serde_json::from_value(envelope["data"]["worktrees"].clone()).unwrap();
    assert_eq!(rows.len(), 2);
    let keys: Vec<&str> = rows[0].keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        [
            "path", "head", "branch", "bare", "locked", "prunable", "current"
        ]
    );
    assert_eq!(rows[0]["head"], fx.head);
    assert_eq!(rows[0]["current"], true);
    assert_eq!(rows[0]["locked"], serde_json::Value::Null);
    assert_eq!(rows[1]["branch"], "feature");
    assert_eq!(rows[1]["locked"], "busy");
    assert_eq!(rows[1]["current"], false);
}

#[test]
fn the_user_file_sets_the_head_length() {
    let fx = Fixture::new();
    fx.write_user_config("head_length = 12\n");
    let stdout = fx.stdout(&fx.repo, &["list"], &[]);
    let first: Vec<&str> = stdout.lines().next().unwrap().split('\t').collect();
    assert_eq!(first[1], &fx.head[..12]);
}

#[test]
fn the_repo_az_toml_narrows_the_plain_columns() {
    let fx = Fixture::new();
    fx.write_az_toml("[project]\nname = \"x\"\n\n[w3.plain]\ncolumns = [\"path\"]\n");
    let stdout = fx.stdout(&fx.repo, &["list"], &[]);
    for line in stdout.lines() {
        assert!(!line.contains('\t'), "{line}");
    }
    assert_eq!(Path::new(stdout.lines().next().unwrap()), fx.repo);
}

#[test]
fn the_environment_wins_over_both_files() {
    let fx = Fixture::new();
    fx.write_user_config("[format]\npipe = \"plain\"\n");
    fx.write_az_toml("[w3.format]\npipe = \"plain\"\n");
    let stdout = fx.stdout(&fx.repo, &["list"], &[("W3_FORMAT", "json")]);
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(value["data"]["worktrees"].is_array(), "{stdout}");
}

#[test]
fn the_flag_wins_over_the_environment() {
    let fx = Fixture::new();
    let stdout = fx.stdout(
        &fx.repo,
        &["list", "--head-length", "4"],
        &[("W3_HEAD_LENGTH", "20")],
    );
    let first: Vec<&str> = stdout.lines().next().unwrap().split('\t').collect();
    assert_eq!(first[1], &fx.head[..4]);
}

#[test]
fn an_unknown_column_fails_with_one_error_line() {
    let fx = Fixture::new();
    let output = fx.run(&fx.repo, &["list", "--columns", "nope"], &[]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert!(
        stderr.starts_with("Error: --columns: unknown column: nope"),
        "{stderr}"
    );
}

#[test]
fn a_typo_in_the_user_file_names_the_file() {
    let fx = Fixture::new();
    fx.write_user_config("head_len = 3\n");
    let output = fx.run(&fx.repo, &["list"], &[]);
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("xdg/w3/config.toml"), "{stderr}");
    assert!(stderr.contains("head_len"), "{stderr}");
}

#[test]
fn help_documents_the_four_flags() {
    let fx = Fixture::new();
    let stdout = fx.stdout(&fx.repo, &["list", "--help"], &[]);
    for flag in ["--format", "--head-length", "--columns", "--fields"] {
        assert!(stdout.contains(flag), "{stdout}");
    }
}

#[test]
fn list_outside_a_repo_fails_with_one_line() {
    let fx = Fixture::new();
    let outside = fx.home().join("outside");
    std::fs::create_dir(&outside).unwrap();
    let output = fx.run(&outside, &["list"], &[]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert!(stderr.starts_with("Error: "), "{stderr}");
}

fn git_out(dir: &Path, args: &[&str]) -> String {
    git(dir, args)
}

fn write_ignored(fx: &Fixture) {
    let repo = &fx.repo;
    std::fs::write(repo.join(".env"), "secret\n").unwrap();
    std::fs::write(repo.join("real.txt"), "real\n").unwrap();
    std::os::unix::fs::symlink("real.txt", repo.join("link")).unwrap();
    std::fs::create_dir(repo.join("nested")).unwrap();
    std::os::unix::fs::symlink("nested", repo.join("dirlink")).unwrap();
    std::fs::write(repo.join("keep.txt"), "tracked\n").unwrap();
    git(repo, &["add", "keep.txt"]);
    git(repo, &["commit", "-q", "-m", "keep"]);
    std::fs::write(
        repo.join(".gitignore"),
        ".env\nreal.txt\nlink\nnested/\ndirlink\n.worktreeinclude\n",
    )
    .unwrap();
    std::fs::write(
        repo.join(".worktreeinclude"),
        "/.env\nlink\nkeep.txt\ndirlink\n",
    )
    .unwrap();
}

#[test]
fn add_creates_a_worktree_under_the_home_default() {
    let fx = Fixture::new();
    let stdout = fx.stdout(&fx.repo, &["add", "feature-x"], &[]);
    let expected = fx.home().join(".worktrees/repo/feature-x");
    assert_eq!(stdout, format!("{}\n", expected.display()));
    assert!(expected.join(".git").exists());
    assert_eq!(
        git_out(&expected, &["rev-parse", "--abbrev-ref", "HEAD"]),
        "feature-x"
    );
    assert_eq!(git_out(&expected, &["rev-parse", "HEAD"]), fx.head);
}

#[test]
fn add_checks_out_an_existing_branch_with_b() {
    let fx = Fixture::new();
    git(&fx.repo, &["branch", "existing"]);
    let stdout = fx.stdout(&fx.repo, &["add", "wt", "-b", "existing"], &[]);
    let path = Path::new(stdout.trim());
    assert!(path.ends_with(".worktrees/repo/wt"), "{stdout}");
    assert_eq!(
        git_out(path, &["rev-parse", "--abbrev-ref", "HEAD"]),
        "existing"
    );
}

#[test]
fn add_starts_the_branch_at_the_given_base() {
    let fx = Fixture::new();
    let first = fx.head.clone();
    git(&fx.repo, &["commit", "-q", "--allow-empty", "-m", "second"]);
    let stdout = fx.stdout(&fx.repo, &["add", "old", "--base", &first], &[]);
    assert_eq!(
        git_out(Path::new(stdout.trim()), &["rev-parse", "HEAD"]),
        first
    );
}

#[test]
fn base_with_branch_is_rejected_before_anything_runs() {
    let fx = Fixture::new();
    let output = fx.run(&fx.repo, &["add", "x", "-b", "main", "--base", "HEAD"], &[]);
    assert_eq!(output.status.code(), Some(2));
    assert!(!fx.home().join(".worktrees").exists());
}

#[test]
fn add_copies_included_files_and_skips_the_rest() {
    let fx = Fixture::new();
    write_ignored(&fx);
    let output = fx.run(&fx.repo, &["add", "wt"], &[]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let target = fx.home().join(".worktrees/repo/wt");
    assert_eq!(
        std::fs::read_to_string(target.join(".env")).unwrap(),
        "secret\n"
    );
    assert_eq!(
        std::fs::read_to_string(target.join("link")).unwrap(),
        "real\n"
    );
    assert!(!target.join("link").is_symlink());
    assert!(!target.join("dirlink").exists());
    let stderr = String::from_utf8(output.stderr).unwrap();
    let lines: Vec<&str> = stderr.lines().collect();
    assert_eq!(
        lines,
        [
            "copied .env",
            "copied link",
            "skipped dirlink: not a regular file"
        ],
        "{stderr}"
    );
}

#[test]
fn json_reports_copy_progress_and_rollback_state() {
    let fx = Fixture::new();
    std::fs::write(fx.repo.join(".gitignore"), "a-good\nz-broken\n").unwrap();
    std::fs::write(fx.repo.join(".worktreeinclude"), "a-good\nz-broken\n").unwrap();
    std::fs::write(fx.repo.join("a-good"), "copy\n").unwrap();
    std::os::unix::fs::symlink("missing", fx.repo.join("z-broken")).unwrap();
    for command in ["add", "cp"] {
        let output = fx.run(&fx.repo, &[command, "failed", "--format", "json"], &[]);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stderr.is_empty());
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["error"]["code"], "copy_failed", "{value}");
        assert!(
            value["data"]["copied"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!("a-good"))
        );
        assert_eq!(
            value["data"]["rollback"],
            serde_json::json!({
                "ok": true, "worktree_removed": true, "branch_deleted": true,
            })
        );
        assert!(!fx.home().join(".worktrees/repo/failed").exists());
    }
}

#[test]
fn rollback_failure_reports_both_errors_and_keeps_the_branch() {
    use std::os::unix::fs::PermissionsExt;
    let fx = Fixture::new();
    std::fs::write(fx.repo.join(".gitignore"), "broken\n").unwrap();
    std::fs::write(fx.repo.join(".worktreeinclude"), "broken\n").unwrap();
    std::os::unix::fs::symlink("missing", fx.repo.join("broken")).unwrap();
    let hook = fx.repo.join(".git/hooks/post-checkout");
    std::fs::write(
        &hook,
        "#!/bin/sh\ngit worktree lock --reason test-lock \"$PWD\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    let output = fx.run(&fx.repo, &["add", "failed", "--format=json"], &[]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["error"]["code"], "rollback_failed", "{value}");
    assert!(
        value["error"]["details"]["cause"]
            .as_str()
            .unwrap()
            .contains("broken")
    );
    assert!(
        value["error"]["details"]["cleanup_error"]
            .as_str()
            .unwrap()
            .contains("locked")
    );
    assert_eq!(value["data"]["rollback"]["worktree_removed"], false);
    assert!(fx.home().join(".worktrees/repo/failed").exists());
    assert_eq!(git_out(&fx.repo, &["rev-parse", "failed"]), fx.head);
}

#[test]
fn json_covers_configuration_validation_and_repository_errors() {
    let fx = Fixture::new();
    for (cwd, args, expected) in [
        (
            &fx.repo,
            vec!["add", "release.1", "--format=json"],
            "invalid_name",
        ),
        (
            &fx.repo,
            vec!["cp", "release.1", "--format=json"],
            "invalid_name",
        ),
        (&fx.home(), vec!["list", "--format=json"], "git_error"),
    ] {
        let output = fx.run(cwd, &args, &[]);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stderr.is_empty());
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["error"]["code"], expected, "{value}");
    }
    fx.write_user_config("not-valid-toml");
    let output = fx.run(&fx.repo, &["list", "--format=json"], &[]);
    assert_eq!(output.status.code(), Some(1));
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["error"]["code"], "invalid_configuration", "{value}");
}

#[test]
fn status_handles_diverged_detached_and_missing_worktrees() {
    let fx = Fixture::new();
    git(&fx.repo, &["branch", "--set-upstream-to=feature"]);
    git(&fx.repo, &["commit", "-q", "--allow-empty", "-m", "main"]);
    git(
        &fx.feature,
        &["commit", "-q", "--allow-empty", "-m", "feature"],
    );
    let status = w3::status(&fx.repo).unwrap();
    assert_eq!((status.ahead, status.behind), (Some(1), Some(1)));
    git(&fx.feature, &["checkout", "--detach", "-q"]);
    let status = w3::status(&fx.feature).unwrap();
    assert_eq!(
        (status.upstream, status.ahead, status.behind),
        (None, None, None)
    );
    git(
        &fx.repo,
        &["worktree", "unlock", fx.feature.to_str().unwrap()],
    );
    std::fs::remove_dir_all(&fx.feature).unwrap();
    let stdout = fx.stdout(
        &fx.repo,
        &["list", "--fields", "path,status", "--format=json"],
        &[],
    );
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(value["data"]["worktrees"][1]["status"]["available"], false);
    assert!(value["data"]["worktrees"][1]["status"]["dirty"].is_null());
}

#[test]
fn remove_refuses_ignored_files_and_never_uses_regex() {
    let fx = Fixture::new();
    git(
        &fx.repo,
        &["worktree", "unlock", fx.feature.to_str().unwrap()],
    );
    std::fs::write(fx.feature.join(".gitignore"), ".env\n").unwrap();
    git(&fx.feature, &["add", ".gitignore"]);
    git(&fx.feature, &["commit", "-qm", "ignore"]);
    std::fs::write(fx.feature.join(".env"), "keep\n").unwrap();
    for (target, expected) in [
        ("feat", "worktree_not_found"),
        ("feature", "unsafe_removal"),
    ] {
        let output = fx.run(&fx.repo, &["remove", target, "--format=json"], &[]);
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["error"]["code"], expected);
        if target == "feature" {
            assert_eq!(value["error"]["details"]["reason"], "ignored_files");
        }
        assert!(fx.feature.join(".env").exists());
    }
}

#[test]
fn json_shell_errors_survive_errexit() {
    let fx = Fixture::new();
    for name in installed_shells() {
        let output = shell(
            &fx,
            name,
            &format!("eval \"$('{W3}' init {name})\"; set -e; w3 cd nope --format=json"),
        );
        assert_eq!(output.status.code(), Some(1));
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["error"]["code"], "worktree_not_found");
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn json_shell_passthrough_preserves_results_and_directory() {
    let fx = Fixture::new();
    for name in installed_shells() {
        for (pattern, ok) in [("feature", true), ("nope", false)] {
            let (lines, stderr) = shell_lines(
                &fx,
                name,
                &format!("w3 cd {pattern} --format json; echo \"exit $?\"; pwd"),
            );
            assert!(stderr.is_empty(), "{name}: {stderr}");
            let value: serde_json::Value = serde_json::from_str(&lines[0]).unwrap();
            assert_eq!(value["ok"], ok);
            assert_eq!(lines[1], if ok { "exit 0" } else { "exit 1" });
            assert_eq!(lines[2], fx.repo.to_str().unwrap());
        }
    }
}

#[test]
fn list_status_reports_dirty_files_and_upstream_divergence() {
    let fx = Fixture::new();
    std::fs::write(fx.repo.join("tracked"), "base\n").unwrap();
    git(&fx.repo, &["add", "tracked"]);
    git(&fx.repo, &["commit", "-qm", "base"]);
    git(&fx.repo, &["branch", "upstream"]);
    git(&fx.repo, &["branch", "--set-upstream-to=upstream"]);
    git(&fx.repo, &["commit", "-q", "--allow-empty", "-m", "ahead"]);
    git(&fx.repo, &["config", "status.aheadBehind", "false"]);
    std::fs::write(fx.repo.join("tracked"), "staged\n").unwrap();
    git(&fx.repo, &["add", "tracked"]);
    std::fs::write(fx.repo.join("tracked"), "unstaged\n").unwrap();
    std::fs::write(fx.repo.join("untracked"), "x\n").unwrap();
    let stdout = fx.stdout(&fx.repo, &["list", "--status", "--format", "json"], &[]);
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let status = &value["data"]["worktrees"][0]["status"];
    for field in ["dirty", "staged", "unstaged", "untracked"] {
        assert_eq!(status[field], true, "{value}");
    }
    assert_eq!(status["upstream"], "upstream");
    assert_eq!(status["ahead"], 1);
    assert_eq!(status["behind"], 0);
    assert!(value["data"]["worktrees"][1]["status"]["upstream"].is_null());
    assert!(value["data"]["worktrees"][1]["status"]["ahead"].is_null());
    let plain = fx.stdout(&fx.repo, &["list", "--status"], &[]);
    assert!(
        plain.contains("dirty") && plain.contains("ahead=1"),
        "{plain}"
    );
}

#[test]
fn remove_preserves_branches_and_refuses_unsafe_targets() {
    let fx = Fixture::new();
    for (target, reason) in [("repo", "main_worktree"), ("feature", "locked_worktree")] {
        let output = fx.run(&fx.repo, &["remove", target, "--format", "json"], &[]);
        assert_eq!(output.status.code(), Some(1));
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["error"]["code"], "unsafe_removal", "{value}");
        assert_eq!(value["error"]["details"]["reason"], reason);
    }
    git(
        &fx.repo,
        &["worktree", "unlock", fx.feature.to_str().unwrap()],
    );
    let output = fx.run(&fx.feature, &["remove", "feature", "--format", "json"], &[]);
    assert!(!output.status.success());
    assert!(fx.feature.exists());
    std::fs::write(fx.feature.join("notes"), "keep\n").unwrap();
    let output = fx.run(&fx.repo, &["remove", "feature", "--format", "json"], &[]);
    assert!(!output.status.success());
    assert!(fx.feature.join("notes").exists());
    std::fs::remove_file(fx.feature.join("notes")).unwrap();
    let stdout = fx.stdout(&fx.repo, &["remove", "feature", "--format", "json"], &[]);
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(value["command"], "remove");
    assert_eq!(value["data"]["branch_deleted"], false);
    assert_eq!(value["data"]["forced"], false);
    assert!(!fx.feature.exists());
    assert_eq!(git_out(&fx.repo, &["rev-parse", "feature"]), fx.head);
}

#[test]
fn remove_force_discards_local_files_but_preserves_the_branch() {
    let fx = Fixture::new();
    git(
        &fx.repo,
        &["worktree", "unlock", fx.feature.to_str().unwrap()],
    );
    std::fs::write(fx.feature.join(".gitignore"), ".env\n").unwrap();
    std::fs::write(fx.feature.join("tracked"), "base\n").unwrap();
    git(&fx.feature, &["add", ".gitignore", "tracked"]);
    git(&fx.feature, &["commit", "-qm", "base"]);
    let head = git_out(&fx.feature, &["rev-parse", "HEAD"]);
    std::fs::write(fx.feature.join("tracked"), "staged\n").unwrap();
    git(&fx.feature, &["add", "tracked"]);
    std::fs::write(fx.feature.join("tracked"), "unstaged\n").unwrap();
    std::fs::write(fx.feature.join("untracked"), "discard\n").unwrap();
    std::fs::write(fx.feature.join(".env"), "discard\n").unwrap();
    assert!(
        !fx.run(&fx.repo, &["remove", "feature"], &[])
            .status
            .success()
    );
    assert!(fx.feature.join(".env").exists());
    let stdout = fx.stdout(
        &fx.repo,
        &["remove", "feature", "--force", "--format=json"],
        &[],
    );
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(value["data"]["forced"], true);
    assert_eq!(value["data"]["branch_deleted"], false);
    assert!(!fx.feature.exists());
    assert_eq!(git_out(&fx.repo, &["rev-parse", "feature"]), head);
    assert_eq!(git_out(&fx.repo, &["rev-parse", "HEAD"]), fx.head);
}

#[test]
fn remove_force_does_not_bypass_worktree_protections() {
    let fx = Fixture::new();
    for (cwd, target, reason) in [
        (&fx.repo, "repo", "main_worktree"),
        (&fx.repo, "feature", "locked_worktree"),
        (&fx.feature, "feature", "current_worktree"),
    ] {
        let output = fx.run(cwd, &["remove", target, "--force", "--format=json"], &[]);
        assert_eq!(output.status.code(), Some(1));
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["error"]["code"], "unsafe_removal");
        assert_eq!(value["error"]["details"]["reason"], reason);
        assert!(fx.repo.exists() && fx.feature.exists());
    }
    git(
        &fx.repo,
        &["worktree", "unlock", fx.feature.to_str().unwrap()],
    );
    let nested = fx.feature.join("nested");
    git(
        &fx.repo,
        &["worktree", "add", "-qb", "nested", nested.to_str().unwrap()],
    );
    let output = fx.run(
        &fx.repo,
        &["remove", "feature", "--force", "--format=json"],
        &[],
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["error"]["details"]["reason"], "nested_worktree");
    assert!(nested.exists());
}

#[test]
fn remove_force_handles_ignored_only_files_and_plain_output() {
    let fx = Fixture::new();
    git(
        &fx.repo,
        &["worktree", "unlock", fx.feature.to_str().unwrap()],
    );
    std::fs::write(fx.feature.join(".gitignore"), ".env\n").unwrap();
    git(&fx.feature, &["add", ".gitignore"]);
    git(&fx.feature, &["commit", "-qm", "ignore"]);
    std::fs::write(fx.feature.join(".env"), "discard\n").unwrap();
    assert!(!w3::status(&fx.feature).unwrap().dirty());
    let stdout = fx.stdout(&fx.repo, &["remove", "feature", "--force"], &[]);
    assert_eq!(stdout.trim(), fx.feature.to_str().unwrap());
    assert!(!fx.feature.exists());
    assert!(!git_out(&fx.repo, &["branch", "--list", "feature"]).is_empty());
}

#[test]
fn creation_commands_share_the_name_policy() {
    for command in ["add", "cp"] {
        let fx = Fixture::new();
        let output = fx.run(&fx.repo, &[command, "release.1"], &[]);
        assert_eq!(output.status.code(), Some(1), "{command}");
        assert!(String::from_utf8_lossy(&output.stderr).contains("name must use"));
        assert!(!fx.home().join(".worktrees").exists());
    }
}

#[test]
fn json_is_a_versioned_envelope_for_every_command() {
    let fx = Fixture::new();
    for args in [
        vec!["list", "--format", "json"],
        vec!["cd", "^feature$", "--format=json"],
        vec!["init", "bash", "--format", "json"],
        vec!["add", "json-add", "--format", "json"],
        vec!["cp", "json-copy", "--format", "json"],
    ] {
        let output = fx.run(&fx.repo, &args, &[]);
        assert!(output.status.success(), "{args:?}: {output:?}");
        assert!(output.stderr.is_empty(), "{output:?}");
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["schema_version"], 1, "{value}");
        assert_eq!(value["command"], args[0]);
        assert_eq!(value["ok"], true);
        assert!(value["data"].is_object());
        assert!(value["error"].is_null());
    }
}

#[test]
fn json_reports_usage_errors_help_and_ambiguity_without_prompting() {
    let fx = Fixture::new();
    for (args, code, error) in [
        (vec!["add", "--format", "json"], 2, "invalid_arguments"),
        (vec!["cd", "--format", "json"], 1, "ambiguous_worktree"),
        (
            vec!["cd", "absent", "--format", "json"],
            1,
            "worktree_not_found",
        ),
    ] {
        let output = fx.run(&fx.repo, &args, &[]);
        assert_eq!(output.status.code(), Some(code), "{output:?}");
        assert!(output.stderr.is_empty(), "{output:?}");
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["ok"], false);
        assert_eq!(value["error"]["code"], error);
        if error == "ambiguous_worktree" {
            assert_eq!(
                value["error"]["details"]["candidates"]
                    .as_array()
                    .unwrap()
                    .len(),
                2
            );
        }
    }
    for args in [
        vec!["--format", "json", "--help"],
        vec!["list", "--help", "--format=json"],
        vec!["--version", "--format=json"],
    ] {
        let output = fx.run(&fx.repo, &args, &[]);
        assert!(output.status.success(), "{output:?}");
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["ok"], true);
    }
}

#[test]
fn add_rolls_back_when_copying_fails() {
    for existing in [false, true] {
        let fx = Fixture::new();
        std::fs::write(fx.repo.join(".gitignore"), "broken\n").unwrap();
        std::fs::write(fx.repo.join(".worktreeinclude"), "broken\n").unwrap();
        std::os::unix::fs::symlink("missing", fx.repo.join("broken")).unwrap();
        if existing {
            git(&fx.repo, &["branch", "existing"]);
        }
        let args = if existing {
            vec!["add", "failed", "-b", "existing"]
        } else {
            vec!["add", "failed"]
        };
        for _ in 0..2 {
            let output = fx.run(&fx.repo, &args, &[]);
            assert_eq!(output.status.code(), Some(1));
            assert!(output.stdout.is_empty());
            let stderr = String::from_utf8(output.stderr).unwrap();
            assert!(stderr.contains("broken"), "{stderr}");
            assert!(!fx.home().join(".worktrees/repo/failed").exists());
            assert_eq!(git_out(&fx.repo, &["branch", "--list", "failed"]), "");
            assert_eq!(w3::list(&fx.repo).unwrap().len(), 2);
            assert_eq!(git_out(&fx.repo, &["rev-parse", "HEAD"]), fx.head);
            assert!(fx.repo.join("broken").is_symlink());
            if existing {
                assert_eq!(git_out(&fx.repo, &["rev-parse", "existing"]), fx.head);
            }
        }
    }
}

#[test]
fn an_existing_directory_is_a_collision() {
    let fx = Fixture::new();
    fx.stdout(&fx.repo, &["add", "wt"], &[]);
    let output = fx.run(&fx.repo, &["add", "wt"], &[]);
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert!(stderr.starts_with("Error: "), "{stderr}");
    assert!(stderr.contains("exists"), "{stderr}");
    assert_eq!(git_out(&fx.repo, &["rev-parse", "wt"]), fx.head);
    assert!(fx.home().join(".worktrees/repo/wt").exists());
}

#[test]
fn an_existing_branch_without_b_is_a_collision() {
    let fx = Fixture::new();
    git(&fx.repo, &["branch", "taken"]);
    let output = fx.run(&fx.repo, &["add", "taken"], &[]);
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert!(stderr.starts_with("Error: "), "{stderr}");
    assert!(stderr.contains("taken"), "{stderr}");
    assert!(!fx.home().join(".worktrees/repo/taken").exists());
    assert_eq!(git_out(&fx.repo, &["rev-parse", "taken"]), fx.head);
}

#[test]
fn a_bare_main_checkout_is_refused() {
    let fx = Fixture::new();
    let bare = fx.home().join("bare.git");
    git(
        &fx.repo,
        &["clone", "-q", "--bare", ".", bare.to_str().unwrap()],
    );
    let output = fx.run(&bare, &["add", "wt"], &[]);
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.starts_with("Error: "), "{stderr}");
    assert!(stderr.contains("bare"), "{stderr}");
}

#[test]
fn the_environment_beats_az_toml_and_the_flag_beats_both() {
    let fx = Fixture::new();
    let home = fx.home();
    fx.write_az_toml(&format!(
        "[w3.worktree]\npath = \"{}/from-file/{{name}}\"\n",
        home.display()
    ));
    let env_template = format!("{}/from-env/{{name}}", home.display());
    let stdout = fx.stdout(
        &fx.repo,
        &["add", "one"],
        &[("W3_WORKTREE_PATH", &env_template)],
    );
    assert_eq!(stdout.trim(), home.join("from-env/one").to_str().unwrap());
    let flag_template = format!("{}/from-flag/{{name}}", home.display());
    let stdout = fx.stdout(
        &fx.repo,
        &["add", "two", "--path", &flag_template],
        &[("W3_WORKTREE_PATH", &env_template)],
    );
    assert_eq!(stdout.trim(), home.join("from-flag/two").to_str().unwrap());
}

#[test]
fn the_worktree_table_sets_the_path_for_cp_too() {
    let fx = Fixture::new();
    let home = fx.home();
    std::fs::write(
        fx.feature.join("az.toml"),
        format!(
            "[w3.worktree]\npath = \"{}/from-file/{{name}}\"\ninclude = \"\"\n",
            home.display()
        ),
    )
    .unwrap();
    let stdout = fx.stdout(&fx.feature, &["cp", "spike"], &[]);
    assert_eq!(
        stdout.trim(),
        home.join("from-file/spike").to_str().unwrap()
    );
}

#[test]
fn a_path_under_the_add_table_is_rejected_naming_the_file() {
    let fx = Fixture::new();
    fx.write_az_toml("[w3.add]\npath = \"/tmp/{name}\"\n");
    let output = fx.run(&fx.repo, &["add", "wt"], &[]);
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("az.toml"), "{stderr}");
    assert!(stderr.contains("path"), "{stderr}");
    assert!(!fx.home().join(".worktrees").exists());
}

#[test]
fn an_empty_include_copies_nothing() {
    let fx = Fixture::new();
    write_ignored(&fx);
    let output = fx.run(&fx.repo, &["add", "wt", "--include", ""], &[]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert!(!fx.home().join(".worktrees/repo/wt/.env").exists());
}

#[test]
fn list_shows_the_new_worktree_as_current_from_inside_it() {
    let fx = Fixture::new();
    let stdout = fx.stdout(&fx.repo, &["add", "wt"], &[]);
    let target = Path::new(stdout.trim());
    let list = fx.stdout(target, &["list", "--format", "plain"], &[]);
    let rows: Vec<Vec<&str>> = list
        .lines()
        .map(|line| line.split('\t').collect())
        .collect();
    assert_eq!(rows.len(), 3, "{list}");
    let row = rows
        .iter()
        .find(|row| Path::new(row[0]) == target)
        .expect("new row");
    assert_eq!(row[2], "wt");
    assert_eq!(row[3], "current");
}

#[test]
fn add_help_documents_the_five_arguments() {
    let fx = Fixture::new();
    let stdout = fx.stdout(&fx.repo, &["add", "--help"], &[]);
    for piece in ["<NAME>", "--branch", "--base", "--path", "--include"] {
        assert!(stdout.contains(piece), "{stdout}");
    }
}

#[test]
fn add_from_a_secondary_worktree_starts_at_its_head() {
    let fx = Fixture::new();
    git(
        &fx.feature,
        &["commit", "-q", "--allow-empty", "-m", "on feature"],
    );
    let feature_head = git(&fx.feature, &["rev-parse", "HEAD"]);
    assert_ne!(feature_head, fx.head);
    let stdout = fx.stdout(&fx.feature, &["add", "spike"], &[]);
    let path = Path::new(stdout.trim());
    assert_eq!(git_out(path, &["rev-parse", "HEAD"]), feature_head);
    assert_eq!(
        git_out(path, &["rev-parse", "--abbrev-ref", "HEAD"]),
        "spike"
    );
}

#[test]
fn add_from_a_subdirectory_of_a_worktree_starts_at_its_head() {
    let fx = Fixture::new();
    git(
        &fx.feature,
        &["commit", "-q", "--allow-empty", "-m", "on feature"],
    );
    let feature_head = git(&fx.feature, &["rev-parse", "HEAD"]);
    let deep = fx.feature.join("src/deep");
    std::fs::create_dir_all(&deep).unwrap();
    let stdout = fx.stdout(&deep, &["add", "spike"], &[]);
    assert_eq!(
        git_out(Path::new(stdout.trim()), &["rev-parse", "HEAD"]),
        feature_head
    );
}

fn commit_file(dir: &Path, name: &str, content: &[u8]) {
    std::fs::write(dir.join(name), content).unwrap();
    git(dir, &["add", name]);
    git(dir, &["commit", "-q", "-m", name]);
}

#[test]
fn cp_starts_at_the_current_head_and_ignores_add_base() {
    let fx = Fixture::new();
    git(
        &fx.feature,
        &["commit", "-q", "--allow-empty", "-m", "on feature"],
    );
    let feature_head = git(&fx.feature, &["rev-parse", "HEAD"]);
    let stdout = fx.stdout(&fx.feature, &["cp", "spike"], &[("W3_ADD_BASE", &fx.head)]);
    let path = Path::new(stdout.trim());
    assert!(path.ends_with(".worktrees/repo/spike"), "{stdout}");
    assert_eq!(git_out(path, &["rev-parse", "HEAD"]), feature_head);
    assert_eq!(
        git_out(path, &["rev-parse", "--abbrev-ref", "HEAD"]),
        "spike"
    );
}

#[test]
fn cp_carries_staged_and_unstaged_changes_apart() {
    let fx = Fixture::new();
    commit_file(&fx.feature, "a.txt", b"a\n");
    commit_file(&fx.feature, "b.txt", b"b\n");
    std::fs::write(fx.feature.join("a.txt"), "staged\n").unwrap();
    git(&fx.feature, &["add", "a.txt"]);
    std::fs::write(fx.feature.join("a.txt"), "staged\nmore\n").unwrap();
    std::fs::write(fx.feature.join("b.txt"), "unstaged\n").unwrap();
    let staged = git(&fx.feature, &["diff", "--cached"]);
    let unstaged = git(&fx.feature, &["diff"]);
    assert!(!staged.is_empty() && !unstaged.is_empty());
    let stdout = fx.stdout(&fx.feature, &["cp", "spike"], &[]);
    let path = Path::new(stdout.trim());
    assert_eq!(git_out(path, &["diff", "--cached"]), staged);
    assert_eq!(git_out(path, &["diff"]), unstaged);
    assert_eq!(
        std::fs::read_to_string(path.join("a.txt")).unwrap(),
        "staged\nmore\n"
    );
}

#[test]
fn cp_carries_a_staged_new_file_and_a_binary_change() {
    let fx = Fixture::new();
    commit_file(&fx.feature, "bin.dat", b"a\0b\n");
    std::fs::write(fx.feature.join("bin.dat"), b"a\0c\0\n").unwrap();
    std::fs::write(fx.feature.join("new.txt"), "new\n").unwrap();
    git(&fx.feature, &["add", "new.txt"]);
    let stdout = fx.stdout(&fx.feature, &["cp", "spike"], &[]);
    let path = Path::new(stdout.trim());
    assert_eq!(std::fs::read(path.join("bin.dat")).unwrap(), b"a\0c\0\n");
    assert_eq!(
        git_out(path, &["diff", "--cached", "--name-only"]),
        "new.txt"
    );
    assert_eq!(
        std::fs::read_to_string(path.join("new.txt")).unwrap(),
        "new\n"
    );
}

#[test]
fn cp_carries_untracked_files_and_reports_them() {
    let fx = Fixture::new();
    std::fs::create_dir(fx.feature.join("nested")).unwrap();
    std::fs::write(fx.feature.join("nested/new.txt"), "untracked\n").unwrap();
    let output = fx.run(&fx.feature, &["cp", "spike"], &[]);
    assert!(output.status.success());
    let path = fx.home().join(".worktrees/repo/spike");
    assert_eq!(
        std::fs::read_to_string(path.join("nested/new.txt")).unwrap(),
        "untracked\n"
    );
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(stderr, "copied nested/new.txt\n");
    assert_eq!(git_out(&path, &["status", "--short"]), "?? nested/");
}

#[test]
fn cp_copies_included_files_from_the_source_worktree() {
    let fx = Fixture::new();
    commit_file(
        &fx.feature,
        ".gitignore",
        b".env\nsecret.txt\n.worktreeinclude\n",
    );
    std::fs::write(fx.repo.join(".worktreeinclude"), "/.env\n").unwrap();
    std::fs::write(fx.repo.join(".env"), "main\n").unwrap();
    std::fs::write(fx.feature.join(".env"), "feature\n").unwrap();
    std::fs::write(fx.feature.join("secret.txt"), "no\n").unwrap();
    let output = fx.run(&fx.feature, &["cp", "spike"], &[]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let path = fx.home().join(".worktrees/repo/spike");
    assert_eq!(
        std::fs::read_to_string(path.join(".env")).unwrap(),
        "feature\n"
    );
    assert!(!path.join("secret.txt").exists());
    assert!(!path.join(".worktreeinclude").exists());
    assert_eq!(String::from_utf8(output.stderr).unwrap(), "copied .env\n");
}

#[test]
fn cp_with_a_clean_tree_creates_a_plain_worktree() {
    let fx = Fixture::new();
    let output = fx.run(&fx.feature, &["cp", "spike"], &[]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let path = fx.home().join(".worktrees/repo/spike");
    assert_eq!(git_out(&path, &["status", "--short"]), "");
}

#[test]
fn cp_rolls_back_when_the_carry_fails() {
    use std::os::unix::fs::PermissionsExt;
    let fx = Fixture::new();
    let locked = fx.feature.join("locked.txt");
    std::fs::write(&locked, "hidden\n").unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
    let output = fx.run(&fx.feature, &["cp", "spike"], &[]);
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("locked.txt"), "{stderr}");
    assert!(!fx.home().join(".worktrees/repo/spike").exists());
    assert_eq!(git_out(&fx.repo, &["branch", "--list", "spike"]), "");
}

#[test]
fn cp_outside_every_worktree_fails_with_one_line() {
    let fx = Fixture::new();
    let git_dir = fx.repo.join(".git");
    let output = fx.run(
        &fx.home(),
        &["cp", "spike"],
        &[("GIT_DIR", git_dir.to_str().unwrap())],
    );
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert!(stderr.contains("not inside a worktree"), "{stderr}");
}

#[test]
fn cp_help_documents_the_three_arguments() {
    let fx = Fixture::new();
    let stdout = fx.stdout(&fx.repo, &["cp", "--help"], &[]);
    for piece in ["<NAME>", "--path", "--include"] {
        assert!(stdout.contains(piece), "{stdout}");
    }
    assert!(!stdout.contains("--base"), "{stdout}");
}

fn add_hotfix_on_fix_login(fx: &Fixture) -> PathBuf {
    let hotfix = fx.home().join("hotfix");
    git(
        &fx.repo,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "fix/login",
            hotfix.to_str().unwrap(),
        ],
    );
    hotfix
}

fn first_cells(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .map(|line| line.split('\t').next().unwrap().to_string())
        .collect()
}

#[test]
fn a_pattern_narrows_plain_output_to_the_matching_rows() {
    let fx = Fixture::new();
    let stdout = fx.stdout(&fx.repo, &["list", "feat"], &[]);
    assert_eq!(first_cells(&stdout), [fx.feature.to_str().unwrap()]);
}

#[test]
fn a_pattern_matches_on_the_branch_when_the_name_differs() {
    let fx = Fixture::new();
    let hotfix = add_hotfix_on_fix_login(&fx);
    let stdout = fx.stdout(&fx.repo, &["list", "login"], &[]);
    assert_eq!(first_cells(&stdout), [hotfix.to_str().unwrap()]);
    let stdout = fx.stdout(&fx.repo, &["list", "^hot"], &[]);
    assert_eq!(first_cells(&stdout), [hotfix.to_str().unwrap()]);
}

#[test]
fn json_returns_only_the_matches() {
    let fx = Fixture::new();
    let stdout = fx.stdout(&fx.repo, &["list", "feat", "--format", "json"], &[]);
    let envelope: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let rows = envelope["data"]["worktrees"].as_array().unwrap();
    assert_eq!(rows.len(), 1, "{stdout}");
    assert_eq!(rows[0]["branch"], "feature");
}

#[test]
fn no_match_is_exit_0_with_empty_output() {
    let fx = Fixture::new();
    let output = fx.run(&fx.repo, &["list", "zzz"], &[]);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    let table = fx.stdout(&fx.repo, &["list", "zzz", "--format", "table"], &[]);
    assert_eq!(table, "");
    let json = fx.stdout(&fx.repo, &["list", "zzz", "--format", "json"], &[]);
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["data"]["worktrees"], serde_json::json!([]));
}

#[test]
fn an_uppercase_letter_in_the_pattern_matches_case() {
    let fx = Fixture::new();
    assert_eq!(fx.stdout(&fx.repo, &["list", "Feat"], &[]), "");
    let stdout = fx.stdout(&fx.repo, &["list", "fEAT"], &[]);
    assert_eq!(stdout, "");
    let stdout = fx.stdout(&fx.repo, &["list", "FEATURE"], &[]);
    assert_eq!(stdout, "");
    let stdout = fx.stdout(&fx.repo, &["list", "feature"], &[]);
    assert_eq!(first_cells(&stdout), [fx.feature.to_str().unwrap()]);
}

#[test]
fn an_invalid_pattern_fails_before_git_runs() {
    let fx = Fixture::new();
    let outside = fx.home().join("outside");
    std::fs::create_dir(&outside).unwrap();
    let output = fx.run(&outside, &["list", "a("], &[]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.starts_with("Error: pattern a(: regex parse error:"),
        "{stderr}"
    );
    assert!(stderr.contains("unclosed group"), "{stderr}");
    assert!(!stderr.contains("git"), "{stderr}");
}

#[test]
fn list_help_documents_the_pattern() {
    let fx = Fixture::new();
    let stdout = fx.stdout(&fx.repo, &["list", "--help"], &[]);
    assert!(stdout.contains("[PATTERN]"), "{stdout}");
}

#[test]
fn a_name_outside_the_rule_is_refused_before_git_runs() {
    let fx = Fixture::new();
    let output = fx.run(&fx.repo, &["add", "release/1.2"], &[]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(
        stderr,
        "Error: name must use letters, digits, -, _, and /: found '.'\n"
    );
    assert_eq!(git(&fx.repo, &["branch", "--list", "release/*"]), "");
    assert!(!fx.home().join(".worktrees/repo").exists());
}

fn complete(fx: &Fixture, cwd: &Path, words: &[&str]) -> Output {
    let index = (words.len() - 1).to_string();
    let mut args = vec!["--"];
    args.extend_from_slice(words);
    fx.run(
        cwd,
        &args,
        &[
            ("COMPLETE", "zsh"),
            ("_CLAP_COMPLETE_INDEX", &index),
            ("_CLAP_IFS", "\n"),
        ],
    )
}

#[test]
fn complete_prints_the_zsh_registration_script() {
    let fx = Fixture::new();
    let stdout = fx.stdout(&fx.repo, &[], &[("COMPLETE", "zsh")]);
    assert!(stdout.contains("compdef"), "{stdout}");
}

#[test]
fn the_pattern_completes_from_names_and_branches() {
    let fx = Fixture::new();
    add_hotfix_on_fix_login(&fx);
    let output = complete(&fx, &fx.repo, &["w3", "list", "f"]);
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout, "feature\nfix/login");
}

#[test]
fn b_completes_the_branches_no_worktree_holds() {
    let fx = Fixture::new();
    git(&fx.repo, &["branch", "release"]);
    let output = complete(&fx, &fx.repo, &["w3", "add", "x", "-b", ""]);
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout, "release");
}

#[test]
fn completion_outside_a_repo_is_silent() {
    let fx = Fixture::new();
    let outside = fx.home().join("outside");
    std::fs::create_dir(&outside).unwrap();
    let output = complete(&fx, &outside, &["w3", "list", "f"]);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[test]
fn cd_prints_the_path_of_the_one_match() {
    let fx = Fixture::new();
    let stdout = fx.stdout(&fx.repo, &["cd", "feature"], &[]);
    assert_eq!(stdout, format!("{}\n", fx.feature.display()));
}

#[test]
fn cd_with_many_matches_and_no_terminal_lists_them_and_fails() {
    let fx = Fixture::new();
    let output = fx.run(&fx.repo, &["cd"], &[]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.starts_with("Error: 2 worktrees"), "{stderr}");
    assert!(stderr.contains("* repo"), "{stderr}");
    assert!(stderr.contains("  feature"), "{stderr}");
}

#[test]
fn cd_with_no_match_fails_with_one_line() {
    let fx = Fixture::new();
    let output = fx.run(&fx.repo, &["cd", "nope"], &[]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "Error: no worktree matches nope\n"
    );
}

#[test]
fn cd_skips_a_prunable_worktree() {
    let fx = Fixture::new();
    let hotfix = add_hotfix_on_fix_login(&fx);
    std::fs::remove_dir_all(&hotfix).unwrap();
    let output = fx.run(&fx.repo, &["cd", "hotfix"], &[]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "Error: no worktree matches hotfix\n"
    );
}

#[test]
fn cd_help_documents_the_pattern() {
    let fx = Fixture::new();
    let stdout = fx.stdout(&fx.repo, &["cd", "--help"], &[]);
    assert!(stdout.contains("[PATTERN]"), "{stdout}");
}

#[test]
fn the_cd_pattern_completes_from_names_and_branches() {
    let fx = Fixture::new();
    let output = complete(&fx, &fx.repo, &["w3", "cd", "f"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "feature");
}

#[test]
fn init_prints_a_function_that_calls_this_binary() {
    let fx = Fixture::new();
    for shell in ["zsh", "bash", "fish"] {
        let stdout = fx.stdout(&fx.repo, &["init", shell], &[]);
        assert!(stdout.contains(&format!("'{W3}'")), "{stdout}");
        assert!(stdout.contains("w3"), "{stdout}");
    }
}

#[test]
fn init_rejects_an_unknown_shell() {
    let fx = Fixture::new();
    let output = fx.run(&fx.repo, &["init", "elvish"], &[]);
    assert_eq!(output.status.code(), Some(2));
}

fn installed_shells() -> Vec<&'static str> {
    ["zsh", "bash"]
        .into_iter()
        .filter(
            |name| match Command::new(name).arg("-c").arg("true").output() {
                Ok(_) => true,
                Err(_) => {
                    eprintln!("skipped: {name} not installed");
                    false
                }
            },
        )
        .collect()
}

fn shell(fx: &Fixture, shell: &str, script: &str) -> Output {
    let home = fx.home();
    Command::new(shell)
        .args(["-c", script])
        .current_dir(&fx.repo)
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap())
        .env("HOME", &home)
        .env("XDG_CONFIG_HOME", home.join("xdg"))
        .output()
        .unwrap()
}

fn shell_lines(fx: &Fixture, name: &str, commands: &str) -> (Vec<String>, String) {
    let output = shell(
        fx,
        name,
        &format!("eval \"$('{W3}' init {name})\"; {commands}"),
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    (stdout.lines().map(String::from).collect(), stderr)
}

#[test]
fn the_shell_function_changes_directory_on_one_match() {
    let fx = Fixture::new();
    for name in installed_shells() {
        let (lines, stderr) = shell_lines(&fx, name, "w3 cd feature; pwd");
        assert_eq!(lines, [fx.feature.to_str().unwrap()], "{name}: {stderr}");
        assert!(stderr.is_empty(), "{name}: {stderr}");
    }
}

#[test]
fn the_shell_function_stays_put_on_no_match() {
    let fx = Fixture::new();
    for name in installed_shells() {
        let (lines, stderr) = shell_lines(&fx, name, "w3 cd nope; echo \"status $?\"; pwd");
        assert_eq!(lines, ["status 1", fx.repo.to_str().unwrap()], "{name}");
        assert_eq!(stderr, "Error: no worktree matches nope\n", "{name}");
    }
}

#[test]
fn the_shell_function_prints_help_instead_of_moving() {
    let fx = Fixture::new();
    for name in installed_shells() {
        let (lines, _) = shell_lines(&fx, name, "w3 cd --help; pwd");
        assert!(
            lines.iter().any(|line| line.contains("[PATTERN]")),
            "{name}: {lines:?}"
        );
        assert_eq!(lines.last().unwrap(), fx.repo.to_str().unwrap(), "{name}");
    }
}

#[test]
fn the_shell_function_passes_other_commands_through() {
    let fx = Fixture::new();
    for name in installed_shells() {
        let (lines, _) = shell_lines(&fx, name, "w3 list | cut -f1");
        assert_eq!(
            lines,
            [fx.repo.to_str().unwrap(), fx.feature.to_str().unwrap()],
            "{name}"
        );
    }
}

#[test]
fn the_init_scripts_parse_in_their_shell() {
    let fx = Fixture::new();
    for name in ["zsh", "bash", "fish"] {
        let script = fx.stdout(&fx.repo, &["init", name], &[]);
        match Command::new(name).arg("-n").arg("-c").arg(&script).output() {
            Ok(output) => assert!(
                output.status.success(),
                "{name}: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
            Err(_) => eprintln!("skipped: {name} not installed"),
        }
    }
}
