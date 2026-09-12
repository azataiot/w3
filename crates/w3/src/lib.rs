mod git;
mod name;
mod status;
mod worktree;

pub use git::{
    Apply, Branch, Changes, add, apply, branches, changes, delete_branch, included_files, list,
    remove, remove_checked, remove_clean, status, untracked_files,
};
pub use name::validate_name;
pub use status::Status;
pub use worktree::{Worktree, parse_porcelain};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("refusing to remove {}: {reason}", path.display())]
    UnsafeRemoval {
        reason: &'static str,
        path: std::path::PathBuf,
    },
    #[error("{0}")]
    InvalidName(String),
    #[error("failed to run git: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("{0}")]
    Git(String),
    #[error("unexpected porcelain line: {0}")]
    Parse(String),
}
