# w3

A Rust library for Git worktree discovery and operations. It supports the
[`w3-cli` application](https://crates.io/crates/w3-cli), which installs the
`w3` command.

Use this crate when you need worktree records or Git operations without the
CLI's configuration, shell integration, or terminal interface.

## Use

The current API is pre-release:

```toml
[dependencies]
w3 = "0.1.0-alpha.4"
```

List the worktrees for the repository that contains the current directory:

```rust
use std::path::Path;

fn main() -> Result<(), w3::Error> {
    for worktree in w3::list(Path::new("."))? {
        println!(
            "{}\t{}",
            worktree.path.display(),
            worktree.branch.as_deref().unwrap_or("(detached or bare)")
        );
    }
    Ok(())
}
```

Git 2.36 or later must be on `PATH`. Operations run synchronously through
the installed Git executable. This crate does not embed a Git implementation.

## API

| API | Purpose |
|---|---|
| `list`, `Worktree` | Read paths, commit IDs, branches, bare state, and lock/prune reasons |
| `parse_porcelain` | Parse bytes from `git worktree list --porcelain -z` without a subprocess |
| `add`, `Branch` | Create a worktree on a new or existing branch |
| `branches` | List local branch names |
| `validate_name` | Apply the shared creation-name policy |
| `status`, `Status` | Inspect dirty state and local upstream divergence |
| `included_files` | Find files that match an include file and are also gitignored |
| `untracked_files` | List non-ignored untracked paths |
| `changes`, `Changes` | Read staged or unstaged binary-capable patches |
| `apply`, `Apply` | Apply a patch to the index and working tree, or only the working tree |
| `remove`, `delete_branch` | Force-remove a worktree or force-delete a branch |
| `remove_clean` | Remove a clean, unprotected worktree without deleting its branch |
| `remove_checked` | Enforce worktree protections, optionally discarding local files with `force: true` |

`Worktree::branch` is `None` for detached or bare records. `locked` and
`prunable` preserve Git's reason strings. On Unix, parsed paths preserve
non-UTF-8 bytes.

Errors distinguish process/I/O failures (`Spawn`), unsuccessful Git commands
(`Git`), invalid porcelain input (`Parse`), invalid creation names
(`InvalidName`), and unsafe removal (`UnsafeRemoval`).

The validation, status, and checked-removal APIs require `0.1.0-alpha.4` or later.

## Caller responsibilities

These are low-level operations, not a transactional workspace API:

- `remove` uses `git worktree remove --force`. `delete_branch` uses `git branch -D`.
  Both can discard work. Confirm ownership and safety before you call them.
- `included_files` and `untracked_files` return paths, not copies.
- `changes` excludes submodule state. Separate calls do not provide an atomic
  snapshot of a worktree that another process can change.
- `add` validates new branch names with `validate_name`. Existing branches
  retain Git's name rules. Callers own path policy, file copying, and rollback.
- `remove_clean` refuses protected or dirty worktrees and ignored files,
  then invokes Git without force. It does not lock concurrent writers.
- `remove_checked` shares those protections. `force: true` permits deletion of
  local changes and ignored files, but never overrides main/current/locked
  or nested-worktree protections. Callers must obtain permission to discard work.

## Develop

From the workspace root:

```sh
cargo test -p w3
cargo doc -p w3 --no-deps
```

Tests cover porcelain parsing and operations against real temporary Git
repositories. See the
[workspace README](https://github.com/azataiot/w3#readme) for the application
and development commands.

MIT license.
