# w3 list

`w3 list` shows every worktree of the current repository. Run it from
anywhere inside the repository, in the main checkout or in a worktree.

```sh
w3 list
```

## What you see

On a terminal, a table. The `*` marks the worktree you are in:

```text
  NAME       BRANCH     HEAD      STATE   PATH
* app        main       1a2b3c4d          ~/Developer/app
  fix-login  fix-login  5e6f7a8b  locked  ~/.worktrees/app/fix-login
  spike      spike      9c0d1e2f          ~/.worktrees/app/spike
```

- **NAME**: the directory name of the worktree
- **BRANCH**: the checked-out branch, blank when the worktree is detached
- **HEAD**: the first eight characters of the commit
- **STATE**: `bare`, `locked`, `prunable`, or blank, in that order
- **PATH**: the directory, with your home shortened to `~`

In a pipe, one line per worktree, tab separated, absolute paths, no header.
The columns are path, head, branch, and state, and the state names the
current worktree too:

```text
/Users/me/Developer/app	1a2b3c4d	main	current
/Users/me/.worktrees/app/fix-login	5e6f7a8b	fix-login	locked
/Users/me/.worktrees/app/spike	9c0d1e2f	spike
```

For an agent or a script, JSON. Every object carries the same seven fields,
and `head` is the full commit hash:

```sh
w3 list --format json
```

```json
[
  {
    "path": "/Users/me/Developer/app",
    "head": "1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b",
    "branch": "main",
    "bare": false,
    "locked": null,
    "prunable": null,
    "current": true
  }
]
```

`locked` and `prunable` carry the reason git recorded, or `null`. `branch` is
`null` for a detached worktree.

## Find a worktree

A pattern keeps the rows whose name or branch matches it:

```sh
w3 list fix
```

```text
  NAME       BRANCH     HEAD      STATE   PATH
  fix-login  fix-login  5e6f7a8b  locked  ~/.worktrees/app/fix-login
```

The pattern is a regular expression. `fix|spike` matches either, `^fix`
anchors at the start, and `login$` at the end. It ignores case unless it has
an uppercase letter: `fix` finds `Fix-Login`, `Fix` finds only that. The
filter applies in every output mode, so `--format json` returns only the
matches. No match prints nothing and exits 0. A pattern that does not
compile is an error that shows where it failed.

Press Tab after `w3 list` to complete the pattern from the names and
branches of the repository, once the [shell setup](README.md#shell-setup) is
done.

## Jump between worktrees

[`w3 cd`](cd.md) does this with a list that filters as you type. In a
script, the plain output and `cut` give the path of one match:

```sh
cd "$(w3 list fix-login | cut -f1)"
```

## Flags

| Flag | Effect |
|---|---|
| `[pattern]` | Keep the rows whose name or branch matches this regular expression, see above |
| `--format table\|plain\|json` | Force one output mode. The default is table on a terminal and plain in a pipe |
| `--head-length N` | Show N characters of the commit in table and plain output, 1 to 40, default 8 |
| `--columns name,branch,head,state,path` | Pick and order the columns of table and plain output |
| `--fields path,head,branch,bare,locked,prunable,current` | Pick and order the fields of JSON output |

A column or field name that w3 does not know is an error that names it.

The same choices live in the [configuration](configuration.md), so you set
them once instead of on every call.
