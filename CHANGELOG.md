# Changelog

## [0.1.0-alpha.4] - 2026-09-12

### Breaking change

All commands now support a shared JSON envelope with `schema_version`,
`command`, `ok`, `data`, and `error`.

`w3 list --format json` no longer returns a top-level array. Records are
under `data.worktrees`. Update jq consumers from `.[] | .path` to
`.data.worktrees[] | .path`, and check `ok` and the process exit code.
An empty result is a successful envelope with `data.worktrees: []`.

See the [machine-interface guide](docs/json.md) for schema version 1,
error codes, and migration examples.

### Added

- Global `--format json`, including structured help, errors, and copy reports
- Noninteractive JSON navigation with candidates for ambiguous matches
- `w3 list --status` for dirty state and local upstream divergence
- `w3 remove` with exact targets, branch preservation, and worktree protections
- `w3 remove --force` to discard local changes and ignored files without
  overriding main, current, locked, or nested-worktree protections
- Core APIs for shared name validation, status inspection, and checked removal
- An installable `w3` skill, plus package READMEs and command documentation

### Fixed

- Failed `add` transfers now attempt rollback, while `add -b` preserves the
  existing branch. Cleanup failures report the original error and remaining state.
- `add` and `cp` now share creation-name rules in the core library.
- Shell wrappers preserve JSON errors and exit codes, including with `set -e`.

### Limits

- Copy remains non-locking and best-effort, not an atomic snapshot.
- Status compares with local upstream refs and does not fetch.
- Force removal requires deliberate authorization to discard local work.
- This remains an alpha release for macOS and Linux.
