mod git;
mod worktree;

pub use git::{Branch, add, included_files, list};
pub use worktree::{Worktree, parse_porcelain};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to run git: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("{0}")]
    Git(String),
    #[error("unexpected porcelain line: {0}")]
    Parse(String),
}
