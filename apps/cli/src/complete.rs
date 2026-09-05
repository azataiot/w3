use std::collections::BTreeSet;

use clap_complete::CompletionCandidate;

use crate::render::name;

pub fn pattern_candidates() -> Vec<CompletionCandidate> {
    let Ok(cwd) = std::env::current_dir() else {
        return Vec::new();
    };
    let Ok(worktrees) = w3::list(&cwd) else {
        return Vec::new();
    };
    candidates(names_and_branches(&worktrees))
}

pub fn branch_candidates() -> Vec<CompletionCandidate> {
    let Ok(cwd) = std::env::current_dir() else {
        return Vec::new();
    };
    let (Ok(worktrees), Ok(branches)) = (w3::list(&cwd), w3::branches(&cwd)) else {
        return Vec::new();
    };
    candidates(unchecked_branches(&worktrees, branches))
}

fn candidates(values: Vec<String>) -> Vec<CompletionCandidate> {
    values.into_iter().map(CompletionCandidate::new).collect()
}

fn names_and_branches(worktrees: &[w3::Worktree]) -> Vec<String> {
    worktrees
        .iter()
        .flat_map(|worktree| std::iter::once(name(&worktree.path)).chain(worktree.branch.clone()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn unchecked_branches(worktrees: &[w3::Worktree], branches: Vec<String>) -> Vec<String> {
    let checked_out: BTreeSet<&str> = worktrees
        .iter()
        .filter_map(|worktree| worktree.branch.as_deref())
        .collect();
    branches
        .into_iter()
        .filter(|branch| !checked_out.contains(branch.as_str()))
        .collect()
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
    fn names_and_branches_merge_sorted_without_duplicates() {
        let worktrees = [
            worktree("/w/repo", Some("main")),
            worktree("/w/feature", Some("feature")),
            worktree("/w/hotfix", Some("fix/login")),
        ];
        assert_eq!(
            names_and_branches(&worktrees),
            ["feature", "fix/login", "hotfix", "main", "repo"]
        );
    }

    #[test]
    fn a_detached_row_contributes_its_name_only() {
        let worktrees = [worktree("/w/spike", None)];
        assert_eq!(names_and_branches(&worktrees), ["spike"]);
    }

    #[test]
    fn checked_out_branches_drop_out_of_the_b_set() {
        let worktrees = [
            worktree("/w/repo", Some("main")),
            worktree("/w/feature", Some("feature")),
        ];
        let branches = ["feature", "fix/login", "main", "release"]
            .map(String::from)
            .to_vec();
        assert_eq!(
            unchecked_branches(&worktrees, branches),
            ["fix/login", "release"]
        );
    }
}
