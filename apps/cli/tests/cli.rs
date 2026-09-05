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
    assert!(stdout.starts_with('['), "{stdout}");
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
fn an_existing_directory_is_a_collision() {
    let fx = Fixture::new();
    fx.stdout(&fx.repo, &["add", "wt"], &[]);
    let output = fx.run(&fx.repo, &["add", "wt"], &[]);
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert!(stderr.starts_with("Error: "), "{stderr}");
    assert!(stderr.contains("exists"), "{stderr}");
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
