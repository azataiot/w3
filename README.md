# w3

A fast CLI for git worktrees, for humans and coding agents.

Coding agents made worktrees the normal way to work. One repository, five
sessions, five worktrees. Moving between them means `git worktree list`, a
mouse selection, and a `cd`. w3 makes that one short command.

The name is a typo. Late one night, ten worktrees deep, I typed “workthree”
instead of “worktree”. Three was as far as I could still count. The typo
shipped.

## Install

w3 is pre-release. Homebrew is the recommended path:

```sh
brew install azataiot/tap/w3
```

The install script installs the latest stable release:

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://raw.githubusercontent.com/azataiot/w3/main/scripts/install.sh | sh
```

From crates.io, with a Rust toolchain:

```sh
cargo install w3-cli
```

For the tip of main you need git 2.36 or later and a Rust toolchain from
<https://rustup.rs>.

```sh
cargo install --git https://github.com/azataiot/w3 w3-cli
```

## Use

```sh
w3 list
```

On a terminal, a table. A `*` marks the worktree you are in:

```text
  NAME      BRANCH   HEAD      STATE   PATH
* w3        main     1a2b3c4d          ~/Developer/w3
  feature   feature  5e6f7a8b  locked  ~/.worktrees/w3/feature
```

In a pipe, one worktree per line, tab-separated, absolute paths, no header.
The columns are path, head, branch, state:

```sh
cd "$(w3 list | fzf | cut -f1)"
```

For an agent or a script, JSON with the full SHA:

```sh
w3 list --format json
```

Flags override everything: `--format table|plain|json`, `--head-length N`,
`--columns name,branch,head,state,path`, `--fields path,head,branch,bare,locked,prunable,current`.

### Configure

Defaults come from, in rising precedence: `~/.config/w3/config.toml` (or
`$XDG_CONFIG_HOME/w3/config.toml`), a `[w3]` table in the repo `az.toml`, the
variables `W3_FORMAT`, `W3_HEAD_LENGTH`, `W3_COLUMNS`, `W3_FIELDS`, then the
flags. Every key is optional. The full shape, with the built-in defaults:

```toml
head_length = 8

[format]
tty = "table"
pipe = "plain"

[table]
columns = ["name", "branch", "head", "state", "path"]

[plain]
columns = ["path", "head", "branch", "state"]

[json]
fields = ["path", "head", "branch", "bare", "locked", "prunable", "current"]
```

In `az.toml` the same keys sit under `[w3]`, `[w3.format]`, `[w3.table]`,
`[w3.plain]`, and `[w3.json]`.

## Layout

```text
apps/cli      the w3 binary (package w3-cli)
crates/w3     library: worktree discovery, porcelain parsing
packages/     TypeScript packages, none yet
```

## Develop

```sh
just          # list recipes
just qa       # fmt check, lint, tests
```

MIT license.
