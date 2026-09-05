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
            .arg("list")
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
    let stdout = fx.stdout(&fx.repo, &[], &[]);
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
    let stdout = fx.stdout(&fx.repo, &["--format", "table"], &[]);
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
    let stdout = fx.stdout(&fx.feature, &["--format", "table"], &[]);
    let lines: Vec<&str> = stdout.lines().collect();
    assert!(lines[1].starts_with("  repo"), "{}", lines[1]);
    assert!(lines[2].starts_with("* feature"), "{}", lines[2]);
}

#[test]
fn json_has_the_seven_fields_in_order_with_the_full_sha() {
    let fx = Fixture::new();
    let stdout = fx.stdout(&fx.repo, &["--format", "json"], &[]);
    assert_eq!(stdout.lines().count(), 1, "{stdout}");
    let rows: Vec<serde_json::Map<String, serde_json::Value>> =
        serde_json::from_str(&stdout).unwrap();
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
    let stdout = fx.stdout(&fx.repo, &[], &[]);
    let first: Vec<&str> = stdout.lines().next().unwrap().split('\t').collect();
    assert_eq!(first[1], &fx.head[..12]);
}

#[test]
fn the_repo_az_toml_narrows_the_plain_columns() {
    let fx = Fixture::new();
    fx.write_az_toml("[project]\nname = \"x\"\n\n[w3.plain]\ncolumns = [\"path\"]\n");
    let stdout = fx.stdout(&fx.repo, &[], &[]);
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
    let stdout = fx.stdout(&fx.repo, &[], &[("W3_FORMAT", "json")]);
    assert!(stdout.starts_with('['), "{stdout}");
}

#[test]
fn the_flag_wins_over_the_environment() {
    let fx = Fixture::new();
    let stdout = fx.stdout(
        &fx.repo,
        &["--head-length", "4"],
        &[("W3_HEAD_LENGTH", "20")],
    );
    let first: Vec<&str> = stdout.lines().next().unwrap().split('\t').collect();
    assert_eq!(first[1], &fx.head[..4]);
}

#[test]
fn an_unknown_column_fails_with_one_error_line() {
    let fx = Fixture::new();
    let output = fx.run(&fx.repo, &["--columns", "nope"], &[]);
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
    let output = fx.run(&fx.repo, &[], &[]);
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("xdg/w3/config.toml"), "{stderr}");
    assert!(stderr.contains("head_len"), "{stderr}");
}

#[test]
fn help_documents_the_four_flags() {
    let fx = Fixture::new();
    let stdout = fx.stdout(&fx.repo, &["--help"], &[]);
    for flag in ["--format", "--head-length", "--columns", "--fields"] {
        assert!(stdout.contains(flag), "{stdout}");
    }
}

#[test]
fn list_outside_a_repo_fails_with_one_line() {
    let fx = Fixture::new();
    let outside = fx.home().join("outside");
    std::fs::create_dir(&outside).unwrap();
    let output = fx.run(&outside, &[], &[]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert!(stderr.starts_with("Error: "), "{stderr}");
}
