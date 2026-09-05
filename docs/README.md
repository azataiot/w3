# w3 user guide

w3 is a small command line tool for git worktrees. A worktree is a second
checkout of the same repository in its own directory, on its own branch. You
keep one repository and work on several branches at once, each in its own
place, with no stash and no switch.

This guide has one page per command and one for the settings:

| Page | Read it when you want to |
|---|---|
| [list](list.md) | See every worktree, jump between them, or feed them to a script |
| [add](add.md) | Start a new branch in a new directory, with your local files in place |
| [cp](cp.md) | Copy the worktree you are in, unfinished changes included, and continue in a copy |
| [configuration](configuration.md) | Change the defaults once, for you or for one repository |

## The five-minute tour

Install w3 (the [README](../README.md) has the install paths), open a
terminal in any git repository, and run:

```sh
w3 list
```

You see a table with one row per worktree. A `*` marks the one you are in.
Now start a branch for a task:

```sh
cd "$(w3 add fix-login)"
```

You are in `~/.worktrees/<repo>/fix-login` on the new branch `fix-login`. Your
gitignored local files, such as `.env`, are already there when the repository
names them in `.worktreeinclude`. Work, commit, push.

Halfway through, you want to try a different approach without losing what
you have:

```sh
cd "$(w3 cp fix-login-alt)"
```

You are in a copy: same commits, same staged and unstaged changes, same
untracked files, on the new branch `fix-login-alt`. The original stays as it
was.

Every command prints the path of the worktree on stdout and everything else
on stderr, so `cd "$(w3 …)"` always works.

## Conventions in this guide

- `<repo>` is the directory name of the main checkout, for example `w3` for
  `~/Developer/w3`.
- Paths, hashes, and names in the examples are made up.
- Every command takes `--help` and prints its flags.
