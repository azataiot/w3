# Machine interface

Every command accepts `--format json`, before or after its subcommand.
`W3_FORMAT=json` also selects JSON. An explicit format flag wins.
File-based format defaults continue to apply to `list` only.
Use an explicit JSON flag for agents, including when configuration is invalid.

Schema version 1 requires w3 `0.1.0-alpha.4` or later.
The version number describes the JSON contract, not the binary version.

## Envelope

Stdout contains one JSON document for success or failure:

```json
{
  "schema_version": 1,
  "command": "add",
  "ok": true,
  "data": {
    "path": "/worktrees/app/task",
    "branch": "task",
    "copied": [".env"],
    "skipped": []
  },
  "error": null
}
```

Agents must examine `schema_version`, `ok`, and the process exit code.
Unknown fields are permitted additions. A change to existing field meaning or
shape requires a new schema version. Do not depend on JSON key order.

Normal completion returns exit 0. Operation failures return 1.
Invalid command arguments return 2. JSON mode never opens a picker or prompts.
Human interactive cancellation remains exit 130.

Command reports and errors stay in the envelope, not on stderr.
Process termination, a broken output pipe, or failure to start the binary
cannot guarantee a complete document.

`command` names the command. For help, version, or invalid invocations where
no known command is found, it is `null`. JSON help and version results use
`data.text`, with `ok: true`.

## Command data

| Command | Success data |
|---|---|
| `list` | `worktrees`: array of worktree records |
| `list --status` | The same records, each with a `status` object |
| `add` | `path`, `branch`, `copied`, `skipped` |
| `cp` | `path`, `branch`, `copied`, `skipped`, `snapshot: "best_effort"` |
| `cd` | `path` |
| `init` | `script`: shell code as a JSON string |
| `remove` | `path`, `branch`, `branch_deleted: false`, `forced` |

Copied and skipped entries are source-relative paths. `skipped` entries are
not regular files. File symlinks are dereferenced. Directory symlinks are skipped.
The reports contain paths, not file contents.

The shell function passes successful and unsuccessful JSON results through
unchanged. JSON `cd` returns a path but does not change the shell directory.
Do not put JSON output directly inside `cd "$(…)"`.

## Status

`list --status` adds status inspection. `--fields path,status` also requests
inspection in JSON mode. Ordinary list calls do not inspect dirty state.

```json
{
  "available": true,
  "dirty": true,
  "staged": true,
  "unstaged": false,
  "untracked": true,
  "conflicted": false,
  "upstream": "origin/main",
  "ahead": 2,
  "behind": 1,
  "error": null
}
```

`dirty` covers staged, unstaged, untracked, or conflicted changes.
Ignored files do not make Git status dirty, but safe removal still refuses them.
Submodule status follows Git's porcelain status.

`remove --force` can discard dirty and ignored files, but cannot bypass
worktree protections. It reports `data.forced: true` and keeps the branch.
Agents must not escalate to force removal without explicit user authorization.

Ahead/behind uses locally available upstream refs. w3 does not fetch.
Absent upstreams and unavailable comparisons return `null`, not zero.
Bare, prunable, missing, or unreadable worktrees return `available: false`,
null state fields, and an error with code `status_unavailable`.
The list command can still succeed when an individual status is unavailable.

## Errors

```json
{
  "schema_version": 1,
  "command": "cd",
  "ok": false,
  "data": {},
  "error": {
    "code": "worktree_not_found",
    "message": "no worktree matches missing",
    "details": {}
  }
}
```

Use error codes for control flow. Messages are for people and can change:

| Code | Meaning |
|---|---|
| `invalid_arguments` | Clap rejected the invocation |
| `invalid_name` | A creation name violates the shared policy |
| `invalid_configuration` | A configuration file or environment setting is invalid |
| `worktree_not_found` | No candidate matches |
| `ambiguous_worktree` | More than one candidate matches; `details.candidates` lists paths and branches |
| `unsafe_removal` | Removal is refused; `details.reason` and `details.path` identify the protection |
| `copy_failed` | Transfer failed and rollback succeeded |
| `rollback_failed` | Transfer and cleanup failed; `details.cause` and `details.cleanup_error` describe both |
| `git_error` | Git returned a nonzero exit code |
| `git_output_invalid` | Git output could not be parsed |
| `io_error` | A process or I/O operation failed |
| `operation_failed` | Another operation or input validation failed |

On post-creation failure, `data` retains the attempted path, branch, and
completed copy reports. It also contains:

```json
{
  "rollback": {
    "ok": true,
    "worktree_removed": true,
    "branch_deleted": true
  }
}
```

`branch_deleted: false` is expected for `add -b`, which preserves the existing
branch. A failed cleanup can also leave a newly created branch behind.
When rollback succeeds, copied paths describe files that cleanup removed.
When cleanup fails, inspect the remaining state. Do not automatically force-delete it.

Copy remains non-locking. Separate reads of the commit, index, and files do not
produce an atomic snapshot. Coordinate with other writers if consistency matters.

## Migration from alpha.3

The old `list --format json` returned an array. Schema 1 wraps that array in
`data.worktrees`. Field selection still applies to each worktree record.
An empty result is a successful envelope with `data.worktrees: []`.

For example, update a jq consumer from `.[] | .path` to:

```sh
w3 list --format json | jq -er 'select(.schema_version == 1 and .ok) | .data.worktrees[] | .path'
```

Do not infer success from the presence of `data.path`: failed creation can
include an attempted path.
