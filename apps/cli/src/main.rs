use anyhow::Context;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "w3", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    #[command(about = "List the worktrees of the current repository")]
    List,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::List => list(),
    }
}

fn list() -> anyhow::Result<()> {
    let cwd = std::env::current_dir().context("cannot read the current directory")?;
    let worktrees = w3::list(&cwd)?;
    let width = path_width(&worktrees);
    for worktree in &worktrees {
        println!("{}", render(worktree, width));
    }
    Ok(())
}

fn path_width(worktrees: &[w3::Worktree]) -> usize {
    worktrees
        .iter()
        .map(|worktree| worktree.path.to_string_lossy().chars().count())
        .max()
        .unwrap_or(0)
}

fn render(worktree: &w3::Worktree, width: usize) -> String {
    let head: String = worktree.head.chars().take(8).collect();
    let branch = match &worktree.branch {
        Some(name) => format!("[{name}]"),
        None => "(detached)".to_string(),
    };
    let mut line = format!(
        "{:<width$}  {head} {branch}",
        worktree.path.to_string_lossy()
    );
    if worktree.bare {
        line.push_str(" bare");
    }
    if worktree.locked.is_some() {
        line.push_str(" locked");
    }
    if worktree.prunable.is_some() {
        line.push_str(" prunable");
    }
    line
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use clap::CommandFactory;

    use super::*;

    const HEAD: &str = "14b96db3c138a070d35201b350cba339eedd99f2";

    fn worktree(path: &str, branch: Option<&str>) -> w3::Worktree {
        w3::Worktree {
            path: PathBuf::from(path),
            head: HEAD.into(),
            branch: branch.map(Into::into),
            locked: None,
            prunable: None,
            bare: false,
        }
    }

    #[test]
    fn clap_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn renders_path_short_head_and_branch() {
        assert_eq!(
            render(&worktree("/repo", Some("main")), 5),
            "/repo  14b96db3 [main]"
        );
    }

    #[test]
    fn renders_detached_head() {
        assert_eq!(
            render(&worktree("/repo", None), 5),
            "/repo  14b96db3 (detached)"
        );
    }

    #[test]
    fn pads_path_to_column_width() {
        assert_eq!(
            render(&worktree("/a", Some("main")), 6),
            "/a      14b96db3 [main]"
        );
    }

    #[test]
    fn appends_state_flags() {
        let mut locked = worktree("/repo", Some("main"));
        locked.bare = true;
        locked.locked = Some(String::new());
        locked.prunable = Some("gitdir file points to non-existent location".into());
        assert_eq!(
            render(&locked, 5),
            "/repo  14b96db3 [main] bare locked prunable"
        );
    }

    #[test]
    fn column_width_counts_characters_not_bytes() {
        let worktrees = [worktree("/äöü", Some("main")), worktree("/ab", Some("dev"))];
        let width = path_width(&worktrees);
        assert_eq!(width, 4);
        assert_eq!(render(&worktrees[1], width), "/ab   14b96db3 [dev]");
    }

    #[test]
    fn empty_list_has_zero_width() {
        assert_eq!(path_width(&[]), 0);
    }
}
