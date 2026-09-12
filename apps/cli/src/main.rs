use std::io::IsTerminal;

use anyhow::Context;
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{ArgValueCandidates, CompleteEnv};

mod add;
mod cd;
mod complete;
mod config;
mod current;
mod filter;
mod init;
mod output;
mod picker;
mod render;

use std::path::{Path, PathBuf};

use config::{Format, Layer, Settings};
use output::{Output, failure};
use render::{Column, Field, Row, parse_list};
use serde_json::json;

#[derive(Parser)]
#[command(name = "w3", version, about)]
struct Cli {
    #[arg(
        long,
        global = true,
        value_name = "table|plain|json",
        help = "Output format; JSON never prompts"
    )]
    format: Option<Format>,
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
    #[command(
        about = "Print the path of one worktree, from a pattern or a picker. Loaded with w3 init, it changes directory"
    )]
    Cd(CdArgs),
    #[command(about = "Print the shell code that makes w3 cd change directory and adds completion")]
    Init(InitArgs),
    #[command(about = "Remove one worktree by exact name, branch, or path; keep its branch")]
    Remove(RemoveArgs),
}

#[derive(clap::Args)]
struct RemoveArgs {
    #[arg(value_name = "TARGET", add = ArgValueCandidates::new(complete::pattern_candidates))]
    target: String,
    #[arg(
        long,
        help = "Discard local changes and ignored files; protected worktrees are still refused"
    )]
    force: bool,
}

#[derive(clap::Args)]
struct CdArgs {
    #[arg(
        value_name = "PATTERN",
        add = ArgValueCandidates::new(complete::pattern_candidates),
        help = "Keep the worktrees whose name or branch matches this regex, case-insensitive unless it has an uppercase letter"
    )]
    pattern: Option<String>,
}

#[derive(clap::Args)]
struct InitArgs {
    #[arg(value_name = "SHELL", help = "The shell of the rc file")]
    shell: init::Shell,
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
        long,
        help = "Inspect dirty state and local upstream divergence for each worktree"
    )]
    status: bool,
    #[arg(
        value_name = "PATTERN",
        add = ArgValueCandidates::new(complete::pattern_candidates),
        help = "Keep the rows whose name or branch matches this regex, case-insensitive unless it has an uppercase letter"
    )]
    pattern: Option<String>,
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
        help = "Fields for json output, comma-separated: path, head, branch, bare, locked, prunable, current, status"
    )]
    fields: Option<String>,
}

fn main() -> std::process::ExitCode {
    CompleteEnv::with_factory(Cli::command).complete();
    let args: Vec<_> = std::env::args_os().collect();
    let mut output = Output::new(&args);
    let cli = match Cli::try_parse_from(&args) {
        Ok(cli) => cli,
        Err(error) => {
            let code = error.exit_code() as u8;
            if !output.json {
                let _ = error.print();
                return code.into();
            }
            let failure = if code == 0 {
                output.data = json!({"text": error.to_string()});
                None
            } else {
                Some(failure("invalid_arguments", error.to_string(), json!({})))
            };
            return if output.finish(failure.as_ref()).is_ok() {
                code
            } else {
                1
            }
            .into();
        }
    };
    if let Some(format) = cli.format {
        output.json = format == Format::Json;
    }
    let result = match cli.command {
        Command::List(args) => list(args, cli.format, &mut output),
        Command::Add(args) => add(args, &mut output),
        Command::Cp(args) => cp(args, &mut output),
        Command::Cd(args) => cd(args, &mut output),
        Command::Init(args) => init(args, &mut output),
        Command::Remove(args) => remove(args, &mut output),
    };
    let failed = result.is_err();
    let written = output.finish(result.as_ref().err());
    u8::from(failed || written.is_err()).into()
}

const CD_COLUMNS: [Column; 3] = [Column::Name, Column::Branch, Column::Path];
const CD_SEARCH_COLUMNS: [Column; 2] = [Column::Name, Column::Branch];
const CANCELLED: i32 = 130;

fn cd(args: CdArgs, output: &mut Output) -> anyhow::Result<()> {
    let cwd = std::env::current_dir().context("cannot read the current directory")?;
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
    let rows = cd::candidates(&worktrees, current, matcher.as_ref());
    let home = std::env::home_dir();
    let labels = render::labels(&rows, &CD_COLUMNS, 0, home.as_deref());
    let picked = match rows.len() {
        0 => {
            return Err(failure(
                "worktree_not_found",
                match &args.pattern {
                    Some(pattern) => format!("no worktree matches {pattern}"),
                    None => "no worktree to go to".into(),
                },
                json!({}),
            ));
        }
        1 => 0,
        count
            if output.json
                || !(std::io::stdin().is_terminal() && std::io::stderr().is_terminal()) =>
        {
            let scope = match &args.pattern {
                Some(pattern) => format!("match {pattern}, refine the pattern"),
                None => "in the repository, give a pattern".to_string(),
            };
            let candidates = render::records(&rows, &[Field::Path, Field::Branch]);
            return Err(failure(
                "ambiguous_worktree",
                format!(
                    "{count} worktrees {scope} or pick on a terminal:\n{}",
                    labels.join("\n")
                ),
                json!({"candidates": candidates}),
            ));
        }
        _ => {
            let entries = render::labels(&rows, &CD_SEARCH_COLUMNS, 0, None)
                .into_iter()
                .zip(&labels)
                .map(|(searchable, label)| picker::Entry {
                    label: label.clone(),
                    search_end: searchable.chars().count(),
                })
                .collect();
            let start = rows.iter().position(|row| row.current).unwrap_or(0);
            match picker::pick(entries, start).context("cannot draw the picker")? {
                Some(index) => index,
                None => std::process::exit(CANCELLED),
            }
        }
    };
    output.data = json!({"path": rows[picked].worktree.path.to_string_lossy()});
    if !output.json {
        println!("{}", rows[picked].worktree.path.display());
    }
    Ok(())
}

fn init(args: InitArgs, output: &mut Output) -> anyhow::Result<()> {
    let exe = std::env::current_exe().context("cannot find the w3 binary")?;
    let script = init::script(args.shell, &exe);
    output.data = json!({"script": script});
    if !output.json {
        print!("{script}");
    }
    Ok(())
}

fn list(args: ListArgs, format: Option<Format>, output: &mut Output) -> anyhow::Result<()> {
    let cwd = std::env::current_dir().context("cannot read the current directory")?;
    let flags = Layer {
        format,
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
    let mut settings = settings(flags, &cwd)
        .map_err(|message| failure("invalid_configuration", message, json!({})))?;
    output.json = settings.mode(std::io::stdout().is_terminal()) == Format::Json;
    if args.status && !settings.fields.contains(&Field::Status) {
        settings.fields.push(Field::Status);
    }
    let inspect = args.status || (output.json && settings.fields.contains(&Field::Status));
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
            status: inspect.then(|| {
                if worktree.bare || worktree.prunable.is_some() {
                    Err("bare or prunable worktree".into())
                } else {
                    w3::status(&worktree.path).map_err(|error| error.to_string())
                }
            }),
        })
        .collect();
    let mode = settings.mode(std::io::stdout().is_terminal());
    if rows.is_empty() && mode == Format::Table {
        return Ok(());
    }
    if output.json {
        output.data = json!({"worktrees": render::records(&rows, &settings.fields)});
        return Ok(());
    }
    let rendered = match mode {
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
    print!("{rendered}");
    Ok(())
}

fn remove(args: RemoveArgs, output: &mut Output) -> anyhow::Result<()> {
    let cwd = std::env::current_dir().context("cannot read the current directory")?;
    let worktrees = w3::list(&cwd)?;
    let path = cwd.join(&args.target).canonicalize().ok();
    let matches: Vec<_> = worktrees
        .iter()
        .filter(|worktree| {
            render::name(&worktree.path) == args.target
                || worktree.branch.as_deref() == Some(args.target.as_str())
                || worktree.path == Path::new(&args.target)
                || path
                    .as_ref()
                    .is_some_and(|path| worktree.path.canonicalize().ok().as_ref() == Some(path))
        })
        .collect();
    let worktree = match matches.as_slice() {
        [worktree] => *worktree,
        [] => {
            return Err(failure(
                "worktree_not_found",
                format!("no worktree matches {}", args.target),
                json!({}),
            ));
        }
        _ => {
            return Err(failure(
                "ambiguous_worktree",
                "multiple worktrees match; supply an absolute path",
                json!({
                    "candidates": matches.iter().map(|worktree| json!({"path": worktree.path.to_string_lossy(), "branch": worktree.branch})).collect::<Vec<_>>()
                }),
            ));
        }
    };
    w3::remove_checked(&cwd, &worktree.path, args.force)?;
    output.data = json!({"path": worktree.path.to_string_lossy(), "branch": worktree.branch, "branch_deleted": false, "forced": args.force});
    if !output.json {
        println!("{}", worktree.path.display());
    }
    Ok(())
}

fn add(args: AddArgs, output: &mut Output) -> anyhow::Result<()> {
    w3::validate_name(&args.name)?;
    let cwd = std::env::current_dir().context("cannot read the current directory")?;
    let flags = Layer {
        worktree_path: args.path,
        worktree_include: args.include,
        add_base: args.base,
        ..Layer::default()
    };
    let settings = settings(flags, &cwd)
        .map_err(|message| failure("invalid_configuration", message, json!({})))?;
    let worktrees = w3::list(&cwd)?;
    let main = main_checkout(&worktrees)?;
    let target = target(&settings, main, &args.name)?;
    let branch = match &args.branch {
        Some(existing) => w3::Branch::Existing(existing),
        None => w3::Branch::New(&args.name),
    };
    w3::add(&cwd, &target, branch, settings.add_base.as_deref())?;
    output.data = json!({"path": target.to_string_lossy(), "branch": args.branch.as_deref().unwrap_or(&args.name), "copied": [], "skipped": []});
    let copy = (|| {
        if !settings.worktree_include.is_empty() {
            let files = w3::included_files(&main.path, Path::new(&settings.worktree_include))?;
            copy_from(&main.path, &target, &files, output)?;
        }
        Ok(())
    })();
    if let Err(error) = copy {
        let new_branch = args.branch.is_none().then_some(args.name.as_str());
        return Err(rollback(&cwd, &target, new_branch, error, output));
    }
    if !output.json {
        println!("{}", target.display());
    }
    Ok(())
}

fn cp(args: CpArgs, output: &mut Output) -> anyhow::Result<()> {
    w3::validate_name(&args.name)?;
    let cwd = std::env::current_dir().context("cannot read the current directory")?;
    let flags = Layer {
        worktree_path: args.path,
        worktree_include: args.include,
        ..Layer::default()
    };
    let settings = settings(flags, &cwd)
        .map_err(|message| failure("invalid_configuration", message, json!({})))?;
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
    output.data = json!({"path": target.to_string_lossy(), "branch": args.name, "copied": [], "skipped": [], "snapshot": "best_effort"});
    if let Err(error) = carry(
        &source.path,
        &main.path,
        &target,
        &settings.worktree_include,
        output,
    ) {
        return Err(rollback(&cwd, &target, Some(&args.name), error, output));
    }
    if !output.json {
        println!("{}", target.display());
    }
    Ok(())
}

fn rollback(
    repo: &Path,
    target: &Path,
    new_branch: Option<&str>,
    error: anyhow::Error,
    output: &mut Output,
) -> anyhow::Error {
    output.data["rollback"] =
        json!({"ok": false, "worktree_removed": false, "branch_deleted": false});
    if let Err(failure) = w3::remove(repo, target) {
        return output::failure(
            "rollback_failed",
            format!(
                "{error:#}; rollback failed: {failure}; worktree may remain at {}; branch {} retained",
                target.display(),
                new_branch.unwrap_or("(pre-existing)")
            ),
            json!({"cause": format!("{error:#}"), "cleanup_error": failure.to_string()}),
        );
    }
    output.data["rollback"]["worktree_removed"] = json!(true);
    if let Some(branch) = new_branch
        && let Err(failure) = w3::delete_branch(repo, branch)
    {
        return output::failure(
            "rollback_failed",
            format!(
                "{error:#}; rollback failed: {failure}; worktree removed; branch {branch} may remain"
            ),
            json!({"cause": format!("{error:#}"), "cleanup_error": failure.to_string()}),
        );
    }
    output.data["rollback"]["ok"] = json!(true);
    output.data["rollback"]["branch_deleted"] = json!(new_branch.is_some());
    failure("copy_failed", format!("{error:#}"), json!({}))
}

fn carry(
    source: &Path,
    main: &Path,
    target: &Path,
    include: &str,
    output: &mut Output,
) -> anyhow::Result<()> {
    let staged = w3::changes(source, w3::Changes::Staged)?;
    w3::apply(target, &staged, w3::Apply::Index)?;
    let unstaged = w3::changes(source, w3::Changes::Unstaged)?;
    w3::apply(target, &unstaged, w3::Apply::WorkingTree)?;
    copy_from(source, target, &w3::untracked_files(source)?, output)?;
    if !include.is_empty() {
        let files = w3::included_files(source, &main.join(include))?;
        copy_from(source, target, &files, output)?;
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

fn copy_from(
    source: &Path,
    target: &Path,
    files: &[PathBuf],
    output: &mut Output,
) -> anyhow::Result<()> {
    let mut copied = Vec::new();
    let mut skipped = Vec::new();
    for file in files {
        let report = add::copy_included(source, target, std::slice::from_ref(file))
            .map_err(anyhow::Error::msg)?;
        for (key, paths) in [("copied", &report.copied), ("skipped", &report.skipped)] {
            for path in paths {
                output.data[key]
                    .as_array_mut()
                    .expect("copy report initialized")
                    .push(json!(path.to_string_lossy()));
            }
        }
        copied.extend(report.copied);
        skipped.extend(report.skipped);
    }
    if !output.json {
        for file in copied {
            eprintln!("copied {}", file.display());
        }
        for file in skipped {
            eprintln!("skipped {}: not a regular file", file.display());
        }
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
