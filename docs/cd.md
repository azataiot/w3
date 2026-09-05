# w3 cd

`w3 cd [pattern]` takes you to a worktree. With no pattern it opens a list
that filters as you type. With a pattern that matches one worktree it goes
there at once.

```sh
w3 cd
```

```text
> 
> * app        main       ~/Developer/app
    fix-login  fix-login  ~/.worktrees/app/fix-login
    spike      spike      ~/.worktrees/app/spike
    docs       docs/site  ~/.worktrees/app/docs
type to filter, tab or arrows to move, enter to go, esc to cancel
```

Type `sp` and the list shrinks to `spike`. Press Enter and the prompt is in
`~/.worktrees/app/spike`. Press Esc to stay where you are.

## Set up the shell once

A program cannot change the directory of the shell that runs it. `w3 init`
prints a small shell function that does. Put one line in your rc file:

```sh
eval "$(w3 init zsh)"
```

```sh
eval "$(w3 init bash)"
```

```fish
w3 init fish | source
```

The line also loads tab completion, so the `COMPLETE` line of the README is
not needed next to it. In zsh, put the line after `compinit`, or completion
stays off while `w3 cd` still works.

The function passes every other command to the binary unchanged. Only
`w3 cd` is caught, and only when its output is a directory. `w3 cd --help`
prints the help and stays put.

## The list

One row per worktree: name, branch, and path, with your home shortened to
`~`. The `*` marks the worktree you are in, and the highlight starts there.
Bare and prunable worktrees are not in the list, because there is no
directory to go to. Locked worktrees are.

| Key | Effect |
|---|---|
| any character | Filter the rows. The match is fuzzy over name and branch, and ignores case unless you type an uppercase letter |
| Tab, Shift-Tab | Move the highlight down or up, around the ends |
| Down, Up, Ctrl-N, Ctrl-P | Move the highlight down or up, stopping at the ends |
| Home, End | Jump to the first or the last row |
| Backspace, Ctrl-W, Ctrl-U | Delete a character, a word, or the whole filter |
| Enter | Go to the highlighted worktree |
| Esc, Ctrl-C, Ctrl-G | Cancel and stay where you are |

The list draws on stderr and takes at most ten rows. The path goes to
stdout, so `cd "$(w3 cd)"` works in a shell without the function.

## The pattern

The pattern is the regular expression of [`w3 list`](list.md): it matches
the name or the branch, and it ignores case unless it has an uppercase
letter. Three outcomes:

- **One match**: w3 prints its path and goes there. No list.
- **Several matches**: the list opens with those rows. Typing narrows them
  again.
- **No match**: `Error: no worktree matches <pattern>`, exit 1, and the shell
  stays where it is.

Press Tab after `w3 cd` to complete the pattern from the names and branches
of the repository.

## In a script or an agent

Without a terminal there is no list. One match prints its path:

```sh
cd "$(w3 cd fix-login)"
```

Several matches are an error on stderr that names them, exit 1. Give a
pattern that matches one worktree. A cancelled list exits 130 and prints
nothing, so `cd "$(w3 cd)"` stays in place on Esc.
