# rl-engine documentation

GitHub renders this page when you open the `docs` folder, so it is the index.
The guide is the only part of this folder that is published as a website; everything else is read here, on GitHub, or in an editor.

## Start here

- **[The guide](guide/src/introduction.md)**, nine chapters that build a small roguelike called Warren, each one playable in the browser.
  Read it as a website at [rudehn.github.io/rl-engine](https://rudehn.github.io/rl-engine/), which has a sidebar and search, or as markdown from the link above.
  Every chapter's code is a runnable binary in `examples/tutorial/src/bin`, compiled by CI, so the code in the guide is code that works.
- **[The system reference](guide/src/systems/)**, one page per system: what it is, how to turn it on, what the engine decides and what your game decides.
  It is part of the same book as the guide and is read at [rudehn.github.io/rl-engine](https://rudehn.github.io/rl-engine/) or as markdown from the link above.
  The guide teaches by building Warren; the reference is what you read once you know what you are looking for.

## What the engine has

- **[OVERVIEW.md](OVERVIEW.md)** is the inventory: every crate, every plugin, every panel, and a "Not built yet" list.
  Two tables near the top of its `rl-bevy` and `rl-ui` sections are the fastest way back in.
  `scripts/check-overview.sh` keeps it honest, and CI runs it.

## How one subsystem works, and why

One page per subsystem, with the alternatives that were rejected and why.
These are the reasoning; the overview is the list.

- [abilities.md](design/abilities.md), what an actor can spend a turn on, as data.
- [effects.md](design/effects.md), what lands and what sets it off: the effects subsystem, its moments and triggers, and what a use costs a thing.
- [fields.md](design/fields.md), a value per tile stepped a turn at a time, and the fire and gas built on it.
- [items.md](design/items.md), what a thing does: the three carriers of an effect list, and why an item never lends an ability.
- [lighting.md](design/lighting.md), light cast through the same shadows as sight.
- [loot.md](design/loot.md), where items turn up and how many, with what an item is left to the game.
- [minds.md](design/minds.md), how a non-player decides.
- [noise.md](design/noise.md), what a sound is and who hears it.
- [props.md](design/props.md), what stands on a map that is neither an actor nor an item.
- [remains.md](design/remains.md), what is left where something died.
- [stealth.md](design/stealth.md), being noticed, and being looked for.
- [ui.md](design/ui.md), why a panel is a view, a collector and a presenter.
- [work.md](design/work.md), an actor doing one thing across many turns.

The subsystems with no page yet are listed at the end of the layout section in [AGENTS.md](../AGENTS.md).

## Why it is built this way

- **[PLAN.md](PLAN.md)** is the design and the history: every decision, the reasoning, the milestones, and a progress log of what landed when.
  When something in the engine looks odd, the reason is usually here.
- **[reviews/](reviews/)** holds the four code reviews of the repositories the engine was extracted from, with `path:line` citations for every claim the plan makes.
- **[TODO.md](TODO.md)** is the work that has been found and not started, in the recommended order.

## For contributors

- **[AGENTS.md](../AGENTS.md)** (which `CLAUDE.md` symlinks to) is the rules: what the build enforces, what review enforces, the layout, and which documents a slice owes when it adds or removes a system.
- `cargo doc --open -p rl-engine` builds the API reference. Every public item is documented and every example in a doc comment runs as a test.
