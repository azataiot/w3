use std::io::IsTerminal;

use anyhow::Context;
use clap::{Parser, Subcommand};

mod config;
mod current;
mod render;

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
}

#[derive(clap::Args)]
struct ListArgs {
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
    let cli = Cli::parse();
    match cli.command {
        Command::List(args) => list(args),
    }
}

fn list(args: ListArgs) -> anyhow::Result<()> {
    let cwd = std::env::current_dir().context("cannot read the current directory")?;
    let settings = settings(&args, &cwd).map_err(anyhow::Error::msg)?;
    let worktrees = w3::list(&cwd)?;
    let current = current::current_index(&worktrees, &cwd);
    let rows: Vec<Row> = worktrees
        .iter()
        .enumerate()
        .map(|(index, worktree)| Row {
            worktree,
            current: Some(index) == current,
        })
        .collect();
    let mode = settings.mode(std::io::stdout().is_terminal());
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

fn settings(args: &ListArgs, cwd: &std::path::Path) -> Result<Settings, String> {
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
    let flags = Layer {
        format: args.format,
        head_length: args.head_length.map(usize::from),
        columns: args
            .columns
            .as_deref()
            .map(|text| parse_list::<Column>(text).map_err(|error| format!("--columns: {error}")))
            .transpose()?,
        fields: args
            .fields
            .as_deref()
            .map(|text| parse_list::<Field>(text).map_err(|error| format!("--fields: {error}")))
            .transpose()?,
        ..Layer::default()
    };
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
