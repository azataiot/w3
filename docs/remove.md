# w3 remove

`w3 remove <target>` removes one clean worktree and preserves its branch.
The target must match an exact directory name, branch, or worktree path.
It is not a regex. An ambiguous target fails without a picker.

```sh
w3 remove feature-x
w3 remove /absolute/path/to/feature-x --format json
```

Run it from another worktree. The command refuses:

- The main checkout or the current worktree, including its subdirectories
- Bare, locked, or prunable worktrees
- Worktrees that contain another registered worktree
- Staged, unstaged, untracked, or conflicted changes
- Ignored files, including local environment files

Move or preserve files before normal removal. Git makes the final removal
check without `--force`. Branches are never deleted.
This does not lock other writers. Do not remove a worktree that another user
or agent still uses.

## Discard local work

`--force` deletes local changes, untracked files, and ignored files. Those
files can include secrets and unfinished work that Git cannot recover.
Use it only when you intend to discard them:

```sh
w3 remove feature-x --force
w3 remove /absolute/path/to/feature-x --force --format json
```

Force still refuses the main checkout, current worktree, bare, locked,
prunable, and nested worktrees. It preserves the branch. It passes one
`--force` flag to Git, never the repeated force needed to override a lock.

Agents must obtain explicit authorization to discard the target's local work.
They must not add `--force` automatically after a failed removal.

Human output is the removed path. JSON output uses the
[shared envelope](json.md) and includes `branch_deleted: false`.
`data.forced` is `true` when `--force` was requested, otherwise `false`.
JSON mode remains noninteractive.

For a missing worktree directory, use Git's inspection and pruning tools.
This command does not perform a cleanup sweep.
