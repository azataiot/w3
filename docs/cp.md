# w3 cp

`w3 cp <name>` copies the worktree you are in into a new one, on a new
branch, unfinished work included. It prints the path of the copy.

```sh
cd "$(w3 cp fix-login-alt)"
```

Use it when you are halfway through a task and want to try something
different without losing where you are. The original worktree does not
change. A coding agent uses it to fork its own task into a sibling.

## What the copy carries

Everything git and w3 know about the worktree you copy:

| In the source | In the copy |
|---|---|
| The commits | The same `HEAD`, on the new branch `<name>` |
| Staged changes | Staged, in the index |
| Unstaged changes | Unstaged, in the working tree |
| Untracked files | Copied |
| Gitignored files that `.worktreeinclude` names | Copied from the source, so your edited `.env` comes along |

A staged and an unstaged change to the same file arrive apart, the way you
left them. Binary changes arrive too. Each copied file is one line on
stderr:

```text
copied notes.md
copied .env
```

## What the copy does not carry

- The stash, the reflog, and anything else outside the working tree
- Submodule state
- A file that git ignores and `.worktreeinclude` does not name

## Where the include file comes from

The include file is a repository rule, so w3 reads it from the main
checkout. The files that match are your task state, so w3 takes them from
the worktree you copy. A worktree that `w3 add` created has no include file
of its own when the file is gitignored, and this split makes the copy work
there too.

## Flags

| Flag | Effect |
|---|---|
| `--path <template>` | Put the copy somewhere else, same template as `w3 add` |
| `--include <file>` | Use another include file. An empty value copies no ignored files |

There is no `--base`: the copy always starts where you stand. The `add.base`
setting does not apply here either.

## When something fails

If a step fails after the new worktree exists, w3 removes the worktree and
the branch again, and prints the error. You can fix the cause and run the
same command again. Lines that say `copied` may already be on stderr from
before the failure. They describe files the rollback removed.

`w3 cp` needs a worktree to copy. Run it from inside one.
