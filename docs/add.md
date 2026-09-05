# w3 add

`w3 add <name>` creates a worktree on a new branch and prints its path. Run
it from anywhere inside the repository.

```sh
cd "$(w3 add fix-login)"
```

This creates `~/.worktrees/<repo>/fix-login`, checks out the new branch
`fix-login`, and copies your local files into it. The branch starts at the
`HEAD` of the worktree you run the command in. Run it on `main`, and the
branch starts at `main`. Run it inside another worktree, and the branch
starts there.

## The name

The name is the branch and the directory at once. A `/` in the name is fine
for the branch and becomes a `-` in the directory:

```sh
w3 add az/fix-7
```

This checks out the branch `az/fix-7` in `~/.worktrees/<repo>/az-fix-7`. A
name that is empty or starts with `-` is refused.

## Your local files

Most repositories have files that git ignores but every checkout needs: a
`.env`, a local config, an editor setting. A fresh worktree has none of
them. w3 copies them for you when the repository has a `.worktreeinclude`
file at its root:

```gitignore
/.env
.envrc
/config/local.toml
```

The rules are the ones Claude Code uses for the same file: gitignore syntax,
and w3 copies a file only when it matches a pattern and git also ignores
it. A tracked file never moves, and a file outside the list never moves. A
symlink is copied as a real file. Each copied file is one line on stderr:

```text
copied .env
copied config/local.toml
```

The source is the main checkout. A directory symlink is skipped with a line
that says so.

## Flags

| Flag | Effect |
|---|---|
| `-b, --branch <branch>` | Check out an existing branch instead of creating one. The name still names the directory |
| `--base <ref>` | Start the new branch at this commit or branch instead of `HEAD`, for example `origin/main` |
| `--path <template>` | Put the worktree somewhere else, see below |
| `--include <file>` | Use another include file, relative to the main checkout. An empty value copies nothing |

`--base` and `-b` do not combine, because an existing branch already has its
start.

## Where the worktree goes

The default template is `~/.worktrees/{repo}/{name}`. `{repo}` is the
directory name of the main checkout and `{name}` is the directory name from
above. Change it for one call:

```sh
w3 add spike --path '~/scratch/{name}'
```

Or once, in the [configuration](configuration.md).

## Errors

w3 stops before it touches anything when:

- the target directory exists
- the branch exists and you did not pass `-b`
- the main checkout is bare, because there is nothing to copy from

Every error is one line on stderr and exit code 1.
