use regex::{Regex, RegexBuilder};

use crate::render::name;

pub fn matcher(pattern: &str) -> Result<Regex, String> {
    let has_uppercase = pattern.chars().any(char::is_uppercase);
    RegexBuilder::new(pattern)
        .case_insensitive(!has_uppercase)
        .build()
        .map_err(|error| error.to_string())
}

pub fn matches(regex: &Regex, worktree: &w3::Worktree) -> bool {
    regex.is_match(&name(&worktree.path))
        || worktree
            .branch
            .as_deref()
            .is_some_and(|branch| regex.is_match(branch))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn worktree(path: &str, branch: Option<&str>) -> w3::Worktree {
        w3::Worktree {
            path: PathBuf::from(path),
            head: "1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b".into(),
            branch: branch.map(Into::into),
            locked: None,
            prunable: None,
            bare: false,
        }
    }

    #[test]
    fn a_lowercase_pattern_ignores_case() {
        let regex = matcher("feat").unwrap();
        assert!(regex.is_match("Feat-x"));
        assert!(regex.is_match("feat-x"));
    }

    #[test]
    fn an_uppercase_letter_makes_the_pattern_sensitive() {
        let regex = matcher("Feat").unwrap();
        assert!(regex.is_match("Feat-x"));
        assert!(!regex.is_match("feat-x"));
    }

    #[test]
    fn an_invalid_pattern_carries_the_crate_message() {
        let error = matcher("a(").unwrap_err();
        assert!(error.starts_with("regex parse error:"), "{error}");
        assert!(error.contains("unclosed group"), "{error}");
    }

    #[test]
    fn a_row_matches_on_the_name() {
        let regex = matcher("hotfix").unwrap();
        assert!(matches(&regex, &worktree("/w/hotfix", Some("fix/login"))));
    }

    #[test]
    fn a_row_matches_on_the_branch() {
        let regex = matcher("login").unwrap();
        assert!(matches(&regex, &worktree("/w/hotfix", Some("fix/login"))));
    }

    #[test]
    fn a_detached_row_matches_on_the_name_only() {
        assert!(matches(
            &matcher("spike").unwrap(),
            &worktree("/w/spike", None)
        ));
        assert!(!matches(
            &matcher("main").unwrap(),
            &worktree("/w/spike", None)
        ));
    }

    #[test]
    fn the_path_beyond_the_name_does_not_match() {
        let regex = matcher("^w$").unwrap();
        assert!(!matches(&regex, &worktree("/w/hotfix", Some("fix/login"))));
    }
}
