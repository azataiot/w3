# Configuration

Every flag has a default, and every default can be changed once instead of
on every call. w3 reads the settings from five layers. A later layer wins,
and each layer sets only the keys it names:

1. Built-in defaults
2. Your user file, `~/.config/w3/config.toml`, or
   `$XDG_CONFIG_HOME/w3/config.toml`
3. A `[w3]` table in the repository's `az.toml`
4. Environment variables
5. Flags

## The full file

Every key is optional. This is the complete shape, with the built-in values:

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

[add]
# base = "origin/main"
```

- **head_length**: commit characters in table and plain output, 1 to 40
- **format.tty**, **format.pipe**: the output mode on a terminal and in a pipe
- **table.columns**, **plain.columns**: the columns of each mode
- **json.fields**: the fields of JSON output
- **worktree.path**: where `w3 add` and `w3 cp` put a worktree. `{repo}` is
  the main checkout's directory name, `{name}` the new directory name
- **worktree.include**: the include file, relative to the main checkout. An
  empty string copies nothing
- **add.base**: the default start of a new branch for `w3 add`, unset means
  `HEAD`. `w3 cp` ignores it

## Per repository

Put the same keys under `[w3]` in the repository's `az.toml`. Every table
gets the `w3.` prefix:

```toml
[w3]
head_length = 12

[w3.worktree]
path = "~/work/{repo}/{name}"

[w3.add]
base = "origin/main"
```

w3 looks for `az.toml` in the current directory and its parents, so it also
works from a subdirectory. A committed `az.toml` is present in every
worktree. An uncommitted one is seen only in the checkout that has it.

## Environment variables

For a shell session or a script:

| Variable | Sets |
|---|---|
| `W3_FORMAT` | `table`, `plain`, or `json`, for every call |
| `W3_HEAD_LENGTH` | `head_length` |
| `W3_COLUMNS` | the columns of the active mode |
| `W3_FIELDS` | `json.fields` |
| `W3_WORKTREE_PATH` | `worktree.path` |
| `W3_WORKTREE_INCLUDE` | `worktree.include` |
| `W3_ADD_BASE` | `add.base` |

## Errors

A key w3 does not know, a column or field name it does not know, or a
`head_length` outside 1 to 40 is an error. The message names the file or the
variable, so you know where to look:

```text
Error: /Users/me/.config/w3/config.toml: unknown field `colour`, expected one of `head_length`, `format`, `table`, `plain`, `json`, `worktree`, `add`
```
