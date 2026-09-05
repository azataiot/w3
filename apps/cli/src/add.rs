use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct Copied {
    pub copied: Vec<PathBuf>,
    pub skipped: Vec<PathBuf>,
}

pub fn check_name(name: &str) -> Result<(), String> {
    let allowed = |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '/');
    match name.chars().find(|c| !allowed(*c)) {
        Some(found) => Err(format!(
            "name must use letters, digits, -, _, and /: found '{found}'"
        )),
        None => Ok(()),
    }
}

pub fn directory_name(name: &str) -> Result<String, String> {
    if name.is_empty() {
        return Err("name must not be empty".to_string());
    }
    if name.starts_with('-') {
        return Err("name must not start with -".to_string());
    }
    Ok(name.replace('/', "-"))
}

pub fn target_path(
    template: &str,
    home: Option<&Path>,
    repo: &str,
    name: &str,
) -> Result<PathBuf, String> {
    let mut rest = template;
    let mut out = String::new();
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start..];
        let end = after
            .find('}')
            .ok_or_else(|| format!("unclosed template variable in {template}"))?;
        let variable = &after[..=end];
        match variable {
            "{repo}" => out.push_str(repo),
            "{name}" => out.push_str(name),
            other => return Err(format!("unknown template variable: {other}")),
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    if let Some(tail) = out.strip_prefix("~/") {
        let home = home.ok_or_else(|| {
            "path template starts with ~ but no home directory is known".to_string()
        })?;
        return Ok(home.join(tail));
    }
    Ok(PathBuf::from(out))
}

pub fn copy_included(source: &Path, target: &Path, files: &[PathBuf]) -> Result<Copied, String> {
    let mut result = Copied {
        copied: Vec::new(),
        skipped: Vec::new(),
    };
    for file in files {
        let origin = source.join(file);
        let is_regular = std::fs::metadata(&origin)
            .map(|metadata| metadata.is_file())
            .map_err(|error| format!("{}: {error}", file.display()))?;
        if !is_regular {
            result.skipped.push(file.clone());
            continue;
        }
        let destination = target.join(file);
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("{}: {error}", file.display()))?;
        }
        std::fs::copy(&origin, &destination)
            .map_err(|error| format!("{}: {error}", file.display()))?;
        result.copied.push(file.clone());
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    #[test]
    fn directory_name_keeps_a_plain_name() {
        assert_eq!(directory_name("feature-x"), Ok("feature-x".to_string()));
    }

    #[test]
    fn directory_name_flattens_slashes() {
        assert_eq!(directory_name("az/fix-7"), Ok("az-fix-7".to_string()));
    }

    #[test]
    fn check_name_accepts_the_five_classes() {
        for name in ["feat", "Feat9", "a-b", "a_b", "a/b/c"] {
            assert_eq!(check_name(name), Ok(()), "{name}");
        }
    }

    #[test]
    fn check_name_names_the_first_offending_character() {
        assert_eq!(
            check_name("release/1.2"),
            Err("name must use letters, digits, -, _, and /: found '.'".to_string())
        );
        assert_eq!(
            check_name("feat(api)"),
            Err("name must use letters, digits, -, _, and /: found '('".to_string())
        );
        assert_eq!(
            check_name("f\u{fc}r"),
            Err("name must use letters, digits, -, _, and /: found '\u{fc}'".to_string())
        );
    }

    #[test]
    fn directory_name_rejects_empty_and_flag_like_names() {
        assert_eq!(
            directory_name(""),
            Err("name must not be empty".to_string())
        );
        assert_eq!(
            directory_name("-x"),
            Err("name must not start with -".to_string())
        );
    }

    #[test]
    fn target_path_expands_home_and_both_variables() {
        assert_eq!(
            target_path(
                "~/.worktrees/{repo}/{name}",
                Some(Path::new("/Users/me")),
                "w3",
                "feature-x"
            ),
            Ok(PathBuf::from("/Users/me/.worktrees/w3/feature-x"))
        );
    }

    #[test]
    fn target_path_without_a_tilde_is_used_as_is() {
        assert_eq!(
            target_path("/srv/wt/{name}", None, "w3", "x"),
            Ok(PathBuf::from("/srv/wt/x"))
        );
    }

    #[test]
    fn target_path_needs_a_home_for_a_tilde() {
        assert_eq!(
            target_path("~/wt/{name}", None, "w3", "x"),
            Err("path template starts with ~ but no home directory is known".to_string())
        );
    }

    #[test]
    fn target_path_rejects_an_unknown_variable() {
        assert_eq!(
            target_path("/wt/{branch}", None, "w3", "x"),
            Err("unknown template variable: {branch}".to_string())
        );
    }

    fn fixture() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let main = tmp.path().join("main");
        let target = tmp.path().join("target");
        std::fs::create_dir_all(main.join("nested/dir")).unwrap();
        std::fs::create_dir(&target).unwrap();
        std::fs::write(main.join("plain.txt"), "plain\n").unwrap();
        std::fs::write(main.join("real.txt"), "real\n").unwrap();
        std::os::unix::fs::symlink("real.txt", main.join("link")).unwrap();
        std::fs::write(main.join("nested/dir/deep.txt"), "deep\n").unwrap();
        std::fs::write(main.join("run.sh"), "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(main.join("run.sh"), std::fs::Permissions::from_mode(0o755))
            .unwrap();
        std::os::unix::fs::symlink("nested", main.join("dirlink")).unwrap();
        (tmp, main, target)
    }

    #[test]
    fn copies_files_dereferences_symlinks_and_creates_parents() {
        let (_tmp, main, target) = fixture();
        let files: Vec<PathBuf> = ["plain.txt", "link", "nested/dir/deep.txt"]
            .iter()
            .map(PathBuf::from)
            .collect();

        let result = copy_included(&main, &target, &files).unwrap();

        assert_eq!(result.copied, files);
        assert!(result.skipped.is_empty());
        assert_eq!(
            std::fs::read_to_string(target.join("plain.txt")).unwrap(),
            "plain\n"
        );
        assert_eq!(
            std::fs::read_to_string(target.join("link")).unwrap(),
            "real\n"
        );
        assert!(!target.join("link").is_symlink());
        assert_eq!(
            std::fs::read_to_string(target.join("nested/dir/deep.txt")).unwrap(),
            "deep\n"
        );
    }

    #[test]
    fn preserves_the_executable_bit() {
        let (_tmp, main, target) = fixture();

        copy_included(&main, &target, &[PathBuf::from("run.sh")]).unwrap();

        let mode = std::fs::metadata(target.join("run.sh"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o111, 0o111);
    }

    #[test]
    fn skips_a_directory_symlink_and_reports_it() {
        let (_tmp, main, target) = fixture();
        let files = [PathBuf::from("dirlink"), PathBuf::from("plain.txt")];

        let result = copy_included(&main, &target, &files).unwrap();

        assert_eq!(result.skipped, vec![PathBuf::from("dirlink")]);
        assert_eq!(result.copied, vec![PathBuf::from("plain.txt")]);
        assert!(!target.join("dirlink").exists());
    }

    #[test]
    fn a_missing_source_is_an_error_naming_the_file() {
        let (_tmp, main, target) = fixture();

        let error = copy_included(&main, &target, &[PathBuf::from("gone.txt")]).unwrap_err();

        assert!(error.starts_with("gone.txt: "), "{error}");
    }
}
