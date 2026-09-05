use std::path::Path;

pub fn current_index(worktrees: &[w3::Worktree], cwd: &Path) -> Option<usize> {
    let cwd = cwd.canonicalize().ok()?;
    worktrees
        .iter()
        .enumerate()
        .filter_map(|(index, worktree)| {
            let root = worktree.path.canonicalize().ok()?;
            cwd.starts_with(&root)
                .then_some((index, root.components().count()))
        })
        .max_by_key(|(_, depth)| *depth)
        .map(|(index, _)| index)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;

    fn worktree(path: &Path) -> w3::Worktree {
        w3::Worktree {
            path: PathBuf::from(path),
            head: String::new(),
            branch: None,
            locked: None,
            prunable: None,
            bare: false,
        }
    }

    fn fixture() -> (tempfile::TempDir, Vec<w3::Worktree>) {
        let tmp = tempfile::tempdir().unwrap();
        let main = tmp.path().join("main");
        let second = tmp.path().join("second");
        std::fs::create_dir_all(main.join("src/deep")).unwrap();
        std::fs::create_dir(&second).unwrap();
        let worktrees = vec![
            worktree(&main),
            worktree(&second),
            worktree(&tmp.path().join("gone")),
        ];
        (tmp, worktrees)
    }

    #[test]
    fn inside_the_main_worktree() {
        let (tmp, worktrees) = fixture();
        assert_eq!(current_index(&worktrees, &tmp.path().join("main")), Some(0));
    }

    #[test]
    fn inside_a_nested_directory() {
        let (tmp, worktrees) = fixture();
        assert_eq!(
            current_index(&worktrees, &tmp.path().join("main/src/deep")),
            Some(0)
        );
    }

    #[test]
    fn inside_the_second_worktree() {
        let (tmp, worktrees) = fixture();
        assert_eq!(
            current_index(&worktrees, &tmp.path().join("second")),
            Some(1)
        );
    }

    #[test]
    fn outside_every_worktree() {
        let (tmp, worktrees) = fixture();
        assert_eq!(current_index(&worktrees, tmp.path()), None);
    }

    #[test]
    fn a_missing_directory_is_nowhere() {
        let (tmp, worktrees) = fixture();
        assert_eq!(current_index(&worktrees, &tmp.path().join("nope")), None);
    }

    #[test]
    fn the_deepest_match_wins() {
        let tmp = tempfile::tempdir().unwrap();
        let outer = tmp.path().join("outer");
        let inner = outer.join("inner");
        std::fs::create_dir_all(&inner).unwrap();
        let worktrees = vec![worktree(&outer), worktree(&inner)];
        assert_eq!(current_index(&worktrees, &inner), Some(1));
    }
}
