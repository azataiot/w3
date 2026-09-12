# w3-cli

The command-line application for [w3](https://github.com/azataiot/w3), a Git
worktree tool for humans and coding agents. The Cargo package is `w3-cli`.
The installed binary is `w3`. The reusable library is the separate `w3` crate.

## Install

w3 is an alpha release. It requires Git 2.36 or later on `PATH`.
Release binaries target macOS and Linux on ARM64 and x86-64.

```sh
brew install azataiot/tap/w3
```

With a Rust toolchain:

```sh
cargo install w3-cli
```

Add the applicable line to your shell configuration. In zsh, put it after
`compinit`:

```sh
eval "$(w3 init zsh)"
```

```sh
eval "$(w3 init bash)"
```

```fish
w3 init fish | source
```

This function lets `w3 cd` change the parent shell's directory and loads
completion. Without it, the binary only prints the selected path.

## Use

Run these commands inside a Git repository:

| Command | Result |
|---|---|
| `w3 list` | A table on a terminal, tab-separated rows in a pipe |
| `w3 list --format json` | Worktree records with full commit IDs |
| `w3 list --status` | Dirty state and local upstream divergence |
| `w3 add feature-x` | A new worktree and branch at the current `HEAD` |
| `w3 cp experiment` | A new branch with the current worktree's uncommitted changes |
| `w3 cd feature` | Select a worktree by pattern, or open a picker for multiple matches |
| `w3 cd` | Open the worktree picker when multiple candidates exist |
| `w3 remove feature-x` | Remove one clean worktree, keeping its branch |

`remove --force` explicitly discards local changes and ignored files.
It still refuses protected worktrees and keeps branches. JSON results include
`data.forced`. Agents must obtain authorization before discarding local work.

`add` and `cp` print the new path on stdout. File-copy messages go to stderr:

```sh
cd "$(w3 add feature-x)"
```

The default destination is `~/.worktrees/{repo}/{name}`. Both commands accept
`--path` and `--include`. The include file uses gitignore syntax and selects
only gitignored files. Treat it as permission to copy local secrets, not as
a sandbox.

`cp` preserves staged and unstaged changes separately, copies untracked files,
and copies selected ignored files. It is not a complete environment snapshot.
It does not carry submodule state. Copied file symlinks become regular files.
Stop other writers if you need a consistent copy.

If file transfer fails, `add` and `cp` attempt to remove their new worktree
and newly created branch. `add -b` preserves the existing branch.
If cleanup fails, inspect the reported remaining state before you retry.

Every command accepts `--format json`. Results use a versioned envelope with
`schema_version`, `command`, `ok`, `data`, and `error`. JSON mode never prompts.
See the [machine interface](https://github.com/azataiot/w3/blob/main/docs/json.md).
This interface, status inspection, and removal are development features that
are not in the published `0.1.0-alpha.3` binary.

Configuration precedence is: built-in defaults, user configuration,
the repository's `[w3]` table in `az.toml`, environment variables, then flags.

See the [user guide](https://github.com/azataiot/w3/blob/main/docs/README.md)
for command details and configuration. Every command accepts `--help`.

## Develop

From the workspace root:

```sh
cargo run -p w3-cli -- --help
cargo test -p w3-cli
just qa
```

The CLI owns argument parsing, configuration, rendering, the interactive
picker, shell integration, and file-copy orchestration. The `w3` library owns
Git subprocess calls and worktree parsing. Integration tests use real temporary
Git repositories.

MIT license.
