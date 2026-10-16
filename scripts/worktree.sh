#!/usr/bin/env bash
# Makes a worktree for a new branch with its build already warm.
#
#   scripts/worktree.sh <name> [base]
#
# The worktree is .claude/worktrees/<name> of the main checkout, on a new
# branch <name> cut from <base>, local `main` if left out: local `main` runs
# far ahead of `origin`, and a worktree cut from `origin/main` builds against
# code hundreds of commits old.
#
# Its target/ starts as an APFS clone of the main checkout's: `cp -c` shares
# every block until one side writes it, so the copy is near instant and costs
# no disk, and every dependency already built there is reused rather than
# compiled again, which for Bevy at opt-level 3 is most of a cold build. The
# clone keeps each file's time (`-p`), so the workspace's own crates, whose
# sources the checkout has just written, are older than those sources and are
# rebuilt; a clone with fresh times would pass them off as built from these
# sources. The incremental caches are left out: they are keyed to the main
# checkout's paths and would only be thrown away. While the clone runs it holds
# the main checkout's build lock, so it never copies a build half written.
set -euo pipefail

name=${1:?usage: scripts/worktree.sh <name> [base]}
base=${2:-main}
main=$(git worktree list --porcelain | sed -n '1s/^worktree //p')
dest="$main/.claude/worktrees/$name"

git -C "$main" worktree add -b "$name" "$dest" "$base"

if [ ! -d "$main/target" ]; then
    echo "worktree.sh: $main has no target/ to clone; the first build is cold"
    exit 0
fi

clone() {
    local from=$1 to=$2 entry
    mkdir -p "$to"
    for entry in "$from"/* "$from"/.[!.]*; do
        [ -e "$entry" ] || continue
        case $(basename "$entry") in
            incremental | .cargo-lock) ;;
            debug | release | profiling | bench | wasm32-unknown-unknown) clone "$entry" "$to/$(basename "$entry")" ;;
            *) cp -c -p -R "$entry" "$to/" ;;
        esac
    done
}

# Cargo locks a profile's directory while it builds there; taking the same
# lock waits out a build in progress and holds the next one off until done.
lock="$main/target/debug/.cargo-lock"
if [ -e "$lock" ]; then
    perl -MFcntl=:flock -e 'open(my $f, "<", shift) or die; flock($f, LOCK_EX) or die; system(@ARGV) == 0 or exit 1' \
        "$lock" bash -c "$(declare -f clone); clone '$main/target' '$dest/target'"
else
    clone "$main/target" "$dest/target"
fi
echo "worktree.sh: $dest is on $name from $base, its build cloned from $main/target"
