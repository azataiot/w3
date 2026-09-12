# w3

A fast CLI for git worktrees, for humans and coding agents.

Coding agents made worktrees the normal way to work. One repository, five
sessions, five worktrees. Moving between them means `git worktree list`, a
mouse selection, and a `cd`. w3 makes that one short command.

The name is a typo. Late one night, ten worktrees deep, I typed “workthree”
instead of “worktree”. Three was as far as I could still count. The typo
shipped.

## Features

- **Fork unfinished work.** `w3 cp` carries staged, unstaged, and untracked
  changes into a new worktree, without a stash or temporary commit.
- **One interface for agents.** Every command supports `--format json`,
  with a versioned envelope, error codes, and noninteractive results.
- **Navigate from the terminal.** Select a worktree by pattern or use the
  fuzzy picker, with completion for bash, zsh, and fish.
- **Bring local files along.** `.worktreeinclude` selects the ignored files
  that a new checkout needs.
- **Inspect before cleanup.** `w3 list --status` shows dirty state and local
  upstream divergence. `w3 remove` keeps branches and protects current or locked
  targets. Use `--force` to explicitly discard local changes and ignored files.
- **Use ordinary Git worktrees.** No daemon or separate worktree registry.

The shared JSON envelope, status inspection, removal, and agent skill are
development features in this source checkout. They are not in `0.1.0-alpha.3`.
Run these commands from the root of the checkout that contains the changes,
not another checkout of the same repository:

```sh
cargo build -p w3-cli
./target/debug/w3 list --status --format json
```

## Install

w3 is pre-release. Homebrew is the recommended path:

```sh
brew install azataiot/tap/w3
```

Only pre-releases are available at present. To install the current alpha
with the install script:

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://raw.githubusercontent.com/azataiot/w3/main/scripts/install.sh | W3_VERSION=0.1.0-alpha.3 sh
```

Without `W3_VERSION`, the script requests the latest stable release and fails
until one exists.

From crates.io, with a Rust toolchain:

```sh
cargo install w3-cli
```

For the tip of main you need git 2.36 or later and a Rust toolchain from
<https://rustup.rs>.

```sh
cargo install --git https://github.com/azataiot/w3 w3-cli
```

One line in the shell's rc file lets `w3 cd` change directory and loads tab
completion, which offers the names and branches of the current repository.
Both come from the binary at shell start, so the shell code and the binary
never drift. In zsh, put the line after `compinit`.

```sh
eval "$(w3 init zsh)"
```

```sh
eval "$(w3 init bash)"
```

```fish
w3 init fish | source
```

## Use

| Command | What it does |
|---|---|
| `w3 list` | List the worktrees of the current repository |
| `w3 list --status` | Include dirty state and local upstream divergence |
| `w3 add <name>` | Create a worktree on a new branch and print its path |
| `w3 cp <name>` | Copy the current worktree, changes included, onto a new branch and print its path |
| `w3 cd [pattern]` | Go to a worktree, picked from a list that filters as you type, or by pattern |
| `w3 init <shell>` | Print the shell function that makes `w3 cd` change directory and loads completion |
| `w3 remove <target>` | Remove one clean worktree by exact name, branch, or path, keeping its branch |

Every command takes `--help`. The [user guide](docs/README.md) has one page
per command and one for the settings.

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
w3 list | cut -f1
```

For an agent or a script, JSON with the full SHA:

```sh
w3 list --format json
```

JSON uses a shared envelope. List records are under `data.worktrees`, not a
top-level array. See the [machine interface](docs/json.md) for the schema,
error codes, copy reports, and migration from the alpha.3 output.

Flags override everything: `--format table|plain|json`, `--head-length N`,
`--columns name,branch,head,state,path`, `--fields path,head,branch,bare,locked,prunable,current`.

A pattern keeps the rows whose name or branch matches:

```sh
w3 list feat
```

The pattern is a regex, so `'fix|feat'` and `^hot` work. It ignores case
unless it has an uppercase letter. No match prints nothing and exits 0.

### Go

```sh
w3 cd
```

A list of the worktrees, name, branch, and path, that filters as you type.
Tab or the arrows move the highlight, Enter goes there, Esc stays. `w3 cd
spike` goes to the one worktree that matches, with no list. Several matches
open the list with those rows. Bare and prunable worktrees are not offered.
The function from `w3 init` does the `cd`. Without it, `cd "$(w3 cd spike)"`
works the same, and a script gets the path on stdout.

### Add

```sh
cd "$(w3 add feature-x)"
```

`w3 add` creates `~/.worktrees/<repo>/feature-x` on the new branch
`feature-x` from the `HEAD` of the worktree you run it in, copies the
gitignored files that `.worktreeinclude` names, and prints the path. Each copied file is one line on
stderr. `-b <branch>` checks out an existing branch instead, `--base <ref>`
starts the new branch elsewhere, `--path <template>` moves the worktree, and
`--include <file>` names another include file. An empty include copies nothing.
A name uses ASCII letters, digits, `-`, `_`, and `/` only.

`.worktreeinclude` follows the Claude Code rules: gitignore syntax, and only a
file that matches a pattern and is also gitignored is copied. Symlinks are
copied as real files. A worktree from either tool carries the same files.

### Copy

```sh
cd "$(w3 cp spike)"
```

`w3 cp` copies the worktree you run it in into a new one on the new branch
`spike`, at the same `HEAD`. It carries the staged changes into the index and
the unstaged changes into the working tree. It copies the untracked files, and
the gitignored files that `.worktreeinclude` names. The include file comes
from the main checkout, the files from the worktree you copy. Each copied file
is one line on stderr. If transfer fails after creation, w3 attempts to remove
the worktree and new branch, and reports any cleanup failure.
`--path` and `--include` work as in `w3 add`.
`add.base` does not apply.

Copy does not lock the source or provide an atomic snapshot while another
process writes. Coordinate with other users and agents when consistency matters.

### Agent skill

The repository includes a [w3 skill](skills/w3/SKILL.md) with command selection,
JSON parsing, and recovery guidance. Install it from a local checkout:

```sh
npx skills@latest add ./skills --skill w3
```

Install the skill from GitHub:

```sh
npx skills@latest add azataiot/w3 --skill w3
```

The skill installs instructions, not the binary. It checks for JSON schema
version 1 before it uses the new interface.

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

[worktree]
path = "~/.worktrees/{repo}/{name}"
include = ".worktreeinclude"
```

`[worktree]` applies to `w3 add` and `w3 cp`. `add.base` names a default base
ref for `w3 add` and is unset by default, meaning `HEAD`. The variables are
`W3_WORKTREE_PATH`, `W3_WORKTREE_INCLUDE`, and `W3_ADD_BASE`. In `az.toml` the
same keys sit under `[w3]`, `[w3.format]`, `[w3.table]`, `[w3.plain]`,
`[w3.json]`, `[w3.worktree]`, and `[w3.add]`.

## Layout

| Path | Purpose |
|---|---|
| [apps/cli](apps/cli/README.md) | The `w3` binary, published as `w3-cli` |
| [crates/w3](crates/w3/README.md) | Library for Git worktree discovery, parsing, and operations |
| [docs](docs/README.md) | User guide |

## Develop

```sh
just          # list recipes
just qa       # fmt check, lint, tests
```

MIT license.
