# The system reference

Twenty-three pages in the published book, one per system, and the checks that keep them true.

Written 2026-09-22.
Status: design agreed, not yet planned.

## 1. What this is, and why

The engine's prose lives in four places, and none of them is a reference a game author can read.

`docs/OVERVIEW.md` is an inventory.
It answers "does the engine have X" and nothing else, by design.

`docs/design/*.md` are rationale.
Their sections are `What abilities buy`, `Cost`, `Risks, and what is left out`, `What is not here`, and they cite the plan inline: "Plan section 3.9 asked for exactly this".
That is a decision record addressed to a reviewer who is deciding whether to agree.
Nine subsystems have one and nine do not.

`docs/guide/src/*.md` teaches by building Warren.
It is a tutorial, so it introduces a system only when Warren needs it, and it stops at what Warren needs.

The crate docs answer "what does this item do" at the item level, under `#![deny(missing_docs)]`.
They cannot answer "how do the seven types in this module fit together".

So a game author who has finished the tutorial and wants to add stealth has a teaser paragraph in chapter 9, a 17KB argument for the design in `docs/design/stealth.md`, and rustdoc.
What is missing is the page that says what stealth is, how to turn it on, what the engine decides, and what is left to the game.

This design adds that page, for every system, in the published book.

## 2. Decisions taken

Settled in brainstorming on 2026-09-22 and recorded so a later reader does not reopen them.

1. **`docs/design` stays unpublished and unchanged**, as the archive of why.
   It may be deleted later, so no published page may depend on one.
   Any rationale that a user needs is absorbed into the system page as a short note.
2. **The progression follows a turn** through the engine, in seven parts, rather than following the crate layout or the reader's task.
   See section 5.
3. **Freshness is enforced in three layers**: coverage, includes and a fingerprint tripwire.
   See section 6.
4. **The pages join the existing book** rather than forming a second one.
5. **`docs/OVERVIEW.md` and `docs/PLAN.md` stay out of scope**, unpublished, unchanged.
6. **Every page carries a `The line` section** naming what the engine decides and what the game decides.

## 3. Where the pages live

`docs/guide/src/systems/<name>.md`, entered in the existing `SUMMARY.md` as seven parts after the nine tutorial chapters.

One book, not two.
A second book would split the search index, break relative links between a chapter and a system page, and need a second `mdbook build` in `pages.yml`.
The tutorial and the reference are the two halves of one reading path: the tutorial is what you read first, the reference is what you read when you know what you are looking for.

`docs/guide/src/09-where-to-go-next.md` becomes the index into the reference.
It is 167 lines of one-paragraph teasers today, each naming a system and pointing outward at rustdoc or an example.
Each teaser keeps its paragraph and gains a link to the page that now exists.

## 4. The page shape

One template, in this order, on every page.

| Section | Holds |
| --- | --- |
| opening, no heading | One paragraph. What the system is. |
| `Turning it on` | The plugin. What it declares with `app.needs::<R>` and the hint it gives. What absence means, if it is optional. |
| `The model` | The types, resources and components, quoted from source. |
| `Using it` | The smallest real example, anchored in `examples/`. |
| `The line` | What the engine decides, and what the game decides. |
| `Where it lives` | Crate, module, tier. |

No `Cost`, no `Risks`, no `Phases`, no plan citations.
Those belong to `docs/design` and stay there.

`The line` is not filler.
It is the property that separates this engine from the one it was extracted from, and three of the rules in `AGENTS.md` are restatements of it: own the loop or leave it out, what the engine cannot know is a `Facet`, no theme words in engine crates.
It is also the question a game author actually arrives with, which is why it gets a fixed heading rather than being left to the prose.

A page is capped at about 120 lines.
Past that the system wants splitting into two pages, and the cap is the signal.

`Using it` quotes an example through an `expand-guide.py` anchor rather than restating code.
Where no example exercises the system, the page says which example is closest and the gap is recorded in `docs/TODO.md`.
A page does not carry invented code.

## 5. The progression

Seven parts, twenty-three pages, ordered so that cross-references run backward.
A reader front-to-back never meets a term before the page that defines it.

**Part I, Foundations.**
The turn loop, with cues, holds and costs.
Registries and content loading.
Seeds and determinism.

**Part II, The world.**
Grids and tiles.
Map generation.
Places and streaming.
Props.

**Part III, What acts.**
Abilities.
Combat and `Loadout`.
Items and equipment.
Minds.

**Part IV, What it perceives.**
Sight and lighting.
Noise.
Stealth.

**Part V, What it leaves behind.**
Fields, meaning fire and gas.
Remains.
Statuses.

**Part VI, What you see.**
Rendering.
Panels, as view, collector and presenter.
Controls, modals and cursors.
Narration.

**Part VII, What persists.**
Saving and the morgue.
The overworld.

Nine of these have a design doc to mine.
Fourteen are written from nothing.

## 6. Freshness: three layers

`scripts/check-systems.py`, run in CI beside the checks that are already there.
Python rather than bash, unlike `check-overview.sh`: this one hashes files and parses a multi-line block, and both are fragile in shell.

### 6.1 Coverage

Every `impl Plugin for X` in `crates/` is named by exactly one system page.
This is the trick `scripts/check-overview.sh` already plays against the overview, applied to a set of pages instead of one file.
A new subsystem cannot land undocumented, because the plugin it must define fails the check until a page claims it.

The workspace has 54 plugins against 23 pages, so the mapping is many-to-one and cannot be derived from names.
The page declares it, in the same comment that carries the fingerprint.
Twenty-six of the 54 are panels and view plugins, most of which belong to the panels page and the rest to rendering and narration.

Two coverage checks will then run over one set of plugins, one requiring a name in `OVERVIEW.md` and one requiring a name in a system page.
They are complementary and both should exist.
Merging them into one script that reads one manifest is worth doing once both are in place, and is not in scope here.

### 6.2 Includes

A page quotes real code through `expand-guide.py`, which expands an anchor and commits the result into the markdown.
A renamed anchor or a moved file then fails `expand-guide.py --check`, the same way it does for a tutorial chapter today.

This also keeps the markdown readable without building the book, which is what makes the pages work for an agent reading the repository.

### 6.3 The fingerprint tripwire

Each page declares what it documents, at the top of the file:

    <!-- documents:
         plugins: LightingPlugin
         files: crates/rl-bevy/src/lighting.rs
                crates/rl-world/src/light.rs
         fingerprint: 8f3a2c91 -->

The fingerprint is a hash of the listed files.
When one of them changes, the check fails, naming the page, the file that moved and the command that clears it:

    systems: lighting.md documents crates/rl-bevy/src/lighting.rs, which changed
    since the page was last confirmed.

    Re-read the page. If it is still true:  scripts/check-systems.py --bless lighting
    If it is not, fix the page first.

This is the only layer that catches prose going quietly wrong: a default that changed, a rule that inverted, a resource that stopped being optional.
Coverage catches an absent page and includes catch a moved symbol, and neither notices a sentence that is now false.

It is the pattern `AGENTS.md` already names for tests, where it reads "fingerprint tripwires labelled as such where none does".
The cost is one bless per real change to a documented file, and the false alarm rate is the rate at which those files change for reasons the page does not care about.
That is the price of the only check that catches the failure that matters.

`--bless` with no argument blesses every page whose files changed, and is the wrong thing to reach for.
The script says so when it is used.

## 7. Changes to existing machinery

- `scripts/expand-guide.py` line 78 is `GUIDE.glob("*.md")`, which does not descend.
  It becomes `rglob`.
  `snippet()` resolves a path against `page.parent`, so a page in `systems/` reaches the crates with one more `../` and needs no other change.
- `scripts/check-guide.sh` assumes a flat `src/` twice: `present` is `ls ./*.md`, and the `SUMMARY.md` regex is `\(([0-9a-z-]+\.md)\)`, which does not match `systems/lighting.md`.
  Left alone, nested pages pass silently unchecked, which is worse than failing.
  Both need to descend.
- `scripts/check-systems.py` is new, and is wired into `.github/workflows/ci.yml` after the `check-overview.sh` step.
- `AGENTS.md` grows the system page into the documentation clause, which today names four files.
  The system page goes first, because it is the one a user reads.
- `docs/README.md` gains the reference in its "Start here" section.
- `.github/workflows/pages.yml` is unchanged.
  The pages are markdown in the same `src`, so the existing build picks them up.

## 8. Voice

The house voice already exists, in `docs/design/fields.md`.
Declarative, present tense, no hedging, and no sentence that claims the system is good.
The new pages inherit it exactly.
What changes is the genre, not the prose.

Banned in review: powerful, simply, just, robust, seamless, leverage, delve, comprehensive, elegant, straightforward.
No sentence beginning "In this section we will".
No paragraph that restates its own heading.
Plain dash, never an em dash.
One sentence per line in the source, per the repository's markdown convention.

A page explains what a reader cannot infer from the code.
It does not narrate the code, and it does not explain what a resource or a component is.

## 9. Phases

**Phase 0, plumbing.**
The two script fixes in section 7, `check-systems.py`, the CI step, and `SUMMARY.md` carrying all twenty-three entries as mdBook `draft` items, which render as greyed and unlinked so the structure is visible while nothing ships blank.
Ends with one pilot page, lighting: it has 16KB of design doc to mine, it is self-contained, and it exercises every section of the template including the optional-resource rule.
The pilot is what the template is judged on, and the template is revised here rather than after twenty-three pages exist.

**Phase 1, the from-scratch spine.**
The turn loop, registries and content, seeds and determinism.
These have no design doc, so they prove the template on the hard case before the easy ones are spent.
The turn loop is first because every other page refers to it.

**Phase 2, the nine with design docs.**
Abilities, fields, minds, noise, props, remains, stealth and panels.
Mining is faster than writing.
Each page absorbs whatever rationale a user needs, which is what makes `docs/design` deletable afterwards.

**Phase 3, the rest.**
Grids and tiles, map generation, places and streaming, combat and `Loadout`, items and equipment, statuses, rendering, controls and modals and cursors, narration, saving and the morgue, the overworld.

Coverage is not enforced until phase 3 lands, because a check that fails on every build is a check that gets commented out.
`check-systems.py` runs from phase 0 with the pages that exist, and the coverage layer is switched from warn to fail in the commit that completes phase 3.

## 10. What is not here

`docs/OVERVIEW.md` and `docs/PLAN.md` are out of scope and stay unpublished, at the user's direction.

Merging the two coverage checks, per section 6.1.

Publishing a markdown rendering of the book for agents that have only the URL.
`expand-guide.py` commits expanded code into the source markdown, so an agent with the repository reads the real thing already, and that is the case that matters here.

Deleting `docs/design`.
This design makes it possible by absorbing what a user needs; whether to do it is a later decision, taken once the reference is complete.
