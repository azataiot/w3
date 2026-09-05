use std::io::IsTerminal;

use anyhow::Context;
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{ArgValueCandidates, CompleteEnv};

mod add;
mod complete;
mod config;
mod current;
mod filter;
mod render;

use std::path::{Path, PathBuf};

use config::{Format, Layer, Settings};
use render::{Column, Field, Row, parse_list};

#[derive(Parser)]
#[command(name = "w3", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    #[command(about = "List the worktrees of the current repository")]
    List(ListArgs),
    #[command(about = "Create a worktree on a new branch and print its path")]
    Add(AddArgs),
    #[command(
        about = "Copy the current worktree, changes included, onto a new branch and print its path"
    )]
    Cp(CpArgs),
}

#[derive(clap::Args)]
struct CpArgs {
    #[arg(value_name = "NAME", help = "New branch and directory name")]
    name: String,
    #[arg(
        long,
        value_name = "TEMPLATE",
        help = "Where the worktree goes, default ~/.worktrees/{repo}/{name}"
    )]
    path: Option<String>,
    #[arg(
        long,
        value_name = "FILE",
        help = "Include file relative to the main checkout, default .worktreeinclude, empty copies nothing"
    )]
    include: Option<String>,
}

#[derive(clap::Args)]
struct AddArgs {
    #[arg(value_name = "NAME", help = "New branch and directory name")]
    name: String,
    #[arg(
        short = 'b',
        long,
        value_name = "BRANCH",
        add = ArgValueCandidates::new(complete::branch_candidates),
        help = "Check out this existing branch instead of creating NAME"
    )]
    branch: Option<String>,
    #[arg(
        long,
        value_name = "REF",
        conflicts_with = "branch",
        help = "Start the new branch here, default HEAD"
    )]
    base: Option<String>,
    #[arg(
        long,
        value_name = "TEMPLATE",
        help = "Where the worktree goes, default ~/.worktrees/{repo}/{name}"
    )]
    path: Option<String>,
    #[arg(
        long,
        value_name = "FILE",
        help = "Include file relative to the main checkout, default .worktreeinclude, empty copies nothing"
    )]
    include: Option<String>,
}

#[derive(clap::Args)]
struct ListArgs {
    #[arg(
        value_name = "PATTERN",
        add = ArgValueCandidates::new(complete::pattern_candidates),
        help = "Keep the rows whose name or branch matches this regex, case-insensitive unless it has an uppercase letter"
    )]
    pattern: Option<String>,
    #[arg(
        long,
        value_name = "table|plain|json",
        help = "Output mode, default table on a terminal and plain in a pipe"
    )]
    format: Option<Format>,
    #[arg(long, value_name = "N", value_parser = clap::value_parser!(u8).range(1..=40), help = "SHA characters in table and plain output, default 8")]
    head_length: Option<u8>,
    #[arg(
        long,
        value_name = "LIST",
        help = "Columns for table and plain output, comma-separated: name, branch, head, state, path"
    )]
    columns: Option<String>,
    #[arg(
        long,
        value_name = "LIST",
        help = "Fields for json output, comma-separated: path, head, branch, bare, locked, prunable, current"
    )]
    fields: Option<String>,
}

fn main() -> anyhow::Result<()> {
    CompleteEnv::with_factory(Cli::command).complete();
    let cli = Cli::parse();
    match cli.command {
        Command::List(args) => list(args),
        Command::Add(args) => add(args),
        Command::Cp(args) => cp(args),
    }
}

fn list(args: ListArgs) -> anyhow::Result<()> {
    let cwd = std::env::current_dir().context("cannot read the current directory")?;
    let flags = Layer {
        format: args.format,
        head_length: args.head_length.map(usize::from),
        columns: args
            .columns
            .as_deref()
            .map(|text| parse_list::<Column>(text).map_err(|error| format!("--columns: {error}")))
            .transpose()
            .map_err(anyhow::Error::msg)?,
        fields: args
            .fields
            .as_deref()
            .map(|text| parse_list::<Field>(text).map_err(|error| format!("--fields: {error}")))
            .transpose()
            .map_err(anyhow::Error::msg)?,
        ..Layer::default()
    };
    let settings = settings(flags, &cwd).map_err(anyhow::Error::msg)?;
    let matcher = args
        .pattern
        .as_deref()
        .map(|pattern| {
            filter::matcher(pattern).map_err(|error| format!("pattern {pattern}: {error}"))
        })
        .transpose()
        .map_err(anyhow::Error::msg)?;
    let worktrees = w3::list(&cwd)?;
    let current = current::current_index(&worktrees, &cwd);
    let rows: Vec<Row> = worktrees
        .iter()
        .enumerate()
        .filter(|(_, worktree)| {
            matcher
                .as_ref()
                .is_none_or(|regex| filter::matches(regex, worktree))
        })
        .map(|(index, worktree)| Row {
            worktree,
            current: Some(index) == current,
        })
        .collect();
    let mode = settings.mode(std::io::stdout().is_terminal());
    if rows.is_empty() && mode == Format::Table {
        return Ok(());
    }
    let output = match mode {
        Format::Table => {
            let home = std::env::home_dir();
            render::table(
                &rows,
                settings.columns_for(mode),
                settings.head_length,
                home.as_deref(),
            )
        }
        Format::Plain => render::plain(&rows, settings.columns_for(mode), settings.head_length),
        Format::Json => render::json(&rows, &settings.fields),
    };
    print!("{output}");
    Ok(())
}

fn add(args: AddArgs) -> anyhow::Result<()> {
    add::check_name(&args.name).map_err(anyhow::Error::msg)?;
    let cwd = std::env::current_dir().context("cannot read the current directory")?;
    let flags = Layer {
        worktree_path: args.path,
        worktree_include: args.include,
        add_base: args.base,
        ..Layer::default()
    };
    let settings = settings(flags, &cwd).map_err(anyhow::Error::msg)?;
    let worktrees = w3::list(&cwd)?;
    let main = main_checkout(&worktrees)?;
    let target = target(&settings, main, &args.name)?;
    let branch = match &args.branch {
        Some(existing) => w3::Branch::Existing(existing),
        None => w3::Branch::New(&args.name),
    };
    w3::add(&cwd, &target, branch, settings.add_base.as_deref())?;
    if !settings.worktree_include.is_empty() {
        let files = w3::included_files(&main.path, Path::new(&settings.worktree_include))?;
        copy_from(&main.path, &target, &files)?;
    }
    println!("{}", target.display());
    Ok(())
}

fn cp(args: CpArgs) -> anyhow::Result<()> {
    let cwd = std::env::current_dir().context("cannot read the current directory")?;
    let flags = Layer {
        worktree_path: args.path,
        worktree_include: args.include,
        ..Layer::default()
    };
    let settings = settings(flags, &cwd).map_err(anyhow::Error::msg)?;
    let worktrees = w3::list(&cwd)?;
    let main = main_checkout(&worktrees)?;
    let source = current::current_index(&worktrees, &cwd)
        .map(|index| &worktrees[index])
        .ok_or_else(|| anyhow::anyhow!("not inside a worktree"))?;
    let target = target(&settings, main, &args.name)?;
    w3::add(
        &cwd,
        &target,
        w3::Branch::New(&args.name),
        Some(&source.head),
    )?;
    if let Err(error) = carry(
        &source.path,
        &main.path,
        &target,
        &settings.worktree_include,
    ) {
        let rollback = w3::remove(&cwd, &target).and_then(|()| w3::delete_branch(&cwd, &args.name));
        return Err(match rollback {
            Ok(()) => error,
            Err(failure) => error.context(format!("rollback failed: {failure}")),
        });
    }
    println!("{}", target.display());
    Ok(())
}

fn carry(source: &Path, main: &Path, target: &Path, include: &str) -> anyhow::Result<()> {
    let staged = w3::changes(source, w3::Changes::Staged)?;
    w3::apply(target, &staged, w3::Apply::Index)?;
    let unstaged = w3::changes(source, w3::Changes::Unstaged)?;
    w3::apply(target, &unstaged, w3::Apply::WorkingTree)?;
    copy_from(source, target, &w3::untracked_files(source)?)?;
    if !include.is_empty() {
        let files = w3::included_files(source, &main.join(include))?;
        copy_from(source, target, &files)?;
    }
    Ok(())
}

fn main_checkout(worktrees: &[w3::Worktree]) -> anyhow::Result<&w3::Worktree> {
    let main = worktrees
        .first()
        .ok_or_else(|| anyhow::anyhow!("no worktree found"))?;
    if main.bare {
        anyhow::bail!("the main checkout is bare, there is nothing to copy from");
    }
    Ok(main)
}

fn target(settings: &Settings, main: &w3::Worktree, name: &str) -> anyhow::Result<PathBuf> {
    let repo = add::directory_name(&main.path.file_name().unwrap_or_default().to_string_lossy())
        .map_err(anyhow::Error::msg)?;
    let directory = add::directory_name(name).map_err(anyhow::Error::msg)?;
    let home = std::env::home_dir();
    let target = add::target_path(&settings.worktree_path, home.as_deref(), &repo, &directory)
        .map_err(anyhow::Error::msg)?;
    if target.exists() {
        anyhow::bail!("{} exists", target.display());
    }
    Ok(target)
}

fn copy_from(source: &Path, target: &Path, files: &[PathBuf]) -> anyhow::Result<()> {
    let copied = add::copy_included(source, target, files).map_err(anyhow::Error::msg)?;
    for file in &copied.copied {
        eprintln!("copied {}", file.display());
    }
    for file in &copied.skipped {
        eprintln!("skipped {}: not a regular file", file.display());
    }
    Ok(())
}

fn settings(flags: Layer, cwd: &Path) -> Result<Settings, String> {
    let user = config::user_file(
        std::env::var("XDG_CONFIG_HOME").ok().as_deref(),
        std::env::home_dir().as_deref(),
    )
    .map(|path| config::load_user_file(&path))
    .transpose()?
    .unwrap_or_default();
    let repo = config::repo_file(cwd)
        .map(|path| config::load_repo_file(&path))
        .transpose()?
        .unwrap_or_default();
    let env = config::from_env(|name| std::env::var(name).ok())?;
    Ok(config::resolve(&[user, repo, env, flags]))
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;

    #[test]
    fn clap_definition_is_valid() {
        Cli::command().debug_assert();
    }
}
