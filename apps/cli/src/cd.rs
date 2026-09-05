use regex::Regex;

use crate::filter;
use crate::render::Row;

pub fn candidates<'a>(
    worktrees: &'a [w3::Worktree],
    current: Option<usize>,
    pattern: Option<&Regex>,
) -> Vec<Row<'a>> {
    worktrees
        .iter()
        .enumerate()
        .filter(|(_, worktree)| !worktree.bare && worktree.prunable.is_none())
        .filter(|(_, worktree)| pattern.is_none_or(|regex| filter::matches(regex, worktree)))
        .map(|(index, worktree)| Row {
            worktree,
            current: Some(index) == current,
        })
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

    fn names(rows: &[Row]) -> Vec<String> {
        rows.iter()
            .map(|row| crate::render::name(&row.worktree.path))
            .collect()
    }

    #[test]
    fn bare_and_prunable_rows_drop_out_and_locked_rows_stay() {
        let mut bare = worktree("/w/repo.git", None);
        bare.bare = true;
        let mut gone = worktree("/w/gone", Some("gone"));
        gone.prunable = Some("gitdir file points to non-existent location".into());
        let mut locked = worktree("/w/locked", Some("locked"));
        locked.locked = Some(String::new());
        let worktrees = [bare, gone, locked, worktree("/w/main", Some("main"))];
        assert_eq!(
            names(&candidates(&worktrees, None, None)),
            ["locked", "main"]
        );
    }

    #[test]
    fn the_current_row_is_marked_by_its_index() {
        let worktrees = [
            worktree("/w/main", Some("main")),
            worktree("/w/feature", Some("feature")),
        ];
        let rows = candidates(&worktrees, Some(1), None);
        assert_eq!(
            rows.iter().map(|row| row.current).collect::<Vec<_>>(),
            [false, true]
        );
    }

    #[test]
    fn a_pattern_narrows_on_name_and_on_branch() {
        let worktrees = [
            worktree("/w/main", Some("main")),
            worktree("/w/hotfix", Some("fix/login")),
            worktree("/w/fixture", Some("spike")),
        ];
        let regex = crate::filter::matcher("fix").unwrap();
        assert_eq!(
            names(&candidates(&worktrees, None, Some(&regex))),
            ["hotfix", "fixture"]
        );
    }
}
