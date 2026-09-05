# w3

A fast CLI for git worktrees, for humans and coding agents.

Coding agents made worktrees the normal way to work. One repository, five
sessions, five worktrees. Moving between them means `git worktree list`, a
mouse selection, and a `cd`. w3 makes that one short command.

The name is a typo. Late one night, ten worktrees deep, I typed “workthree”
instead of “worktree”. Three was as far as I could still count. The typo
shipped.

## Install

You need git 2.36 or later and a Rust toolchain from <https://rustup.rs>.

```sh
cargo install --git https://github.com/azataiot/w3 w3-cli
```

## Use

```sh
w3 list
```

One line per worktree: path, short HEAD, branch or `(detached)`, and `locked`
or `prunable` when set.

## Layout

```text
apps/cli      the w3 binary (package w3-cli)
crates/w3     library: worktree discovery, porcelain parsing
packages/     TypeScript packages, none yet
```

## Develop

```sh
just          # list recipes
just qa       # fmt check, lint, tests
```

MIT license.
