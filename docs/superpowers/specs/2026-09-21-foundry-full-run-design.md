# Foundry: the full run

Ten decks, four charges, the climb back out, and the two ways a run can end.

Written 2026-09-21.
Status: design agreed, not yet planned.

## 1. What this is, and why

`examples/foundry` plays three decks today.
A deck is built, populated and looted; a charge is set on deck three's reactor; the pick that follows it writes `RunOver::won()` as a placeholder so the slice has an ending at all.
Everything past that is unbuilt: there is no deck four, no second charge, no way back up, no victory that means anything, and no summary when the commando dies.

This design closes the loop.
After it, a run is a whole run: down ten decks setting four charges, back up all ten with the foundry awake behind you, out through the lift you came down on, or dead on the way with a morgue file to show for it.

It deliberately adds almost no content.
The decks all use today's assembly generator, the roster gains no new kinds, and the upgrade pool stays at three until the loop is finished.
Zones, hazards, salvagers, hunters and the rest are content, and content comes after the loop works.

## 2. Decisions taken

These were settled before this was written, and are recorded here so a later reader does not reopen them.

1. **Ten decks on one generator.** `DECKS = 10` on today's rooms-and-doors builder, with reactor prefabs on 3, 6 and 9 and a core on 10. Zone tiles, hazards and per-zone light are content for later.
2. **The climb repopulates at the deepest band reached**, with that deck's population cap. See section 3.
3. **Repopulation is not gated on the core charge.** Any revisit repopulates. Farming is prevented by banding drops to the deck rather than by gating the repopulation. See section 3.3.
4. **Deck one's lift out refuses with a line** until the last charge is set, rather than not existing until then. The map does not change under the player.
5. **The upgrade pool stays at three for the loop.** A pick offers what is left rather than always three, which makes four picks correct with three upgrades. Growing the pool is section 8.
6. **Noise loudness is one component**, not one per kind of sound. See section 4.1.
7. **The title screen shows at startup and on either ending**, and carries Foundry's own ASCII art.
8. **Save and resume is in scope**, so the title screen's second row continues a run that was quit out of. Death deletes the save, so permadeath holds.

## 3. The climb

### 3.1 What the classics do

The question worth asking first is whether a revisited level draws from its own spawn table or from a new one.
Neither, in the two games that have solved this longest: they keep one table and change the depth it is sampled at.

NetHack pins it to progress outright.
Carrying the Amulet of Yendor on the way up, "monster difficulty will depend on your deepest level reached, not your current dungeon level".
That is one table, sampled at a different depth, and it has shipped since the 1980s.

Dungeon Crawl Stone Soup's Orb Run keys the return trip to progress as well, though to how much was taken rather than how deep the player went: "the frequency and size of waves depend on the number of runes you have".
Its design page also wants the waves to arrive from the stairs, since the monsters "are running down to find you", which is the same idea as Foundry's rule that each faction comes in from where it would really come from.

So Foundry's design was already the conventional answer, and it is cheap: one banded table, one parameter.

### 3.2 The rule

A `Deepest` resource records the deepest deck the run has reached, updated on every arrival.

`populate_deck` today skips a revisit entirely: `PlaceEntered::first` being false leaves the deck alone.
It gains a revisit branch that populates at `band = deepest` with `target = BASE_GROUPS + deepest * GROUPS_PER_DECK`, so a deck one revisited after the core holds what deck ten holds, in the numbers deck ten holds them in.

Loot does not re-scatter.
`scatter_on_arrival` stays first-arrival-only: the design says salvagers strip what the fighting leaves behind, and a deck that re-scattered would turn the climb into a shopping trip.

### 3.3 Why this does not become a grind, and what it cost to get there

DCSS removed monster respawn on cleared levels in 0.6.0 for a reason worth taking seriously.
Players waited on early levels killing respawns at no risk, and the project's stated principle is to avoid "activities that have low risk, take a lot of time, and bring some reward".

Foundry has no experience levels, and loot does not re-scatter, so two of the three rewards are already absent.
The third is not: monsters **drop** things.
Left alone, a commando could bounce between two decks killing deepest-band droids for deepest-band drops, which is exactly the loop DCSS deleted.

The fix is to band the drops to the deck rather than to gate the repopulation:

> A drop entry whose item declares a `spawn` band is kept only when the deck it died on lies inside that band.
> An item with no `spawn` band is not of the decks at all, and always drops.

`drop_on_death` already reads the dead actor's `OnMap`, so the deck is in hand.
A heavy droid killed on deck one drops nothing, because its blaster carbine bands at 3-10 and its composite plate at 2-10.
The climb therefore pays deepest-band danger for deck-band loot, on a deck whose loot was already taken, and the loop is closed by construction rather than by a rule the player has to be told.

The second half of the rule is not a special case but the thing that makes it correct.
Hunter's plate is designed to drop from bounty hunters and to lie on no deck, so it will carry no `spawn` band, and it must still drop wherever a hunter dies.

## 4. Engine changes

### 4.1 Noise loudness, unified

`Footfall(pub i32)` is removed and replaced by one component in `crates/rl-bevy/src/noise.rs`:

```rust
/// Steps of loudness added to every sound this actor's own actions make.
pub struct Loudness(pub i32);
```

The semantics change as well as the name.
`Footfall` was an absolute override of `NoiseRules::step` and applied to steps only.
`Loudness` is a modifier in the same unit as everything else, applied to all four of the engine's own sources in `make_engine_noise` (step, strike, door, landing), clamped so a sound never carries a negative distance.
Negative pads, positive clatters, absent is zero.

This is what makes the Dampers upgrade the one the design asked for.
A ranged attack raises a `DamageEvent` exactly as a blow does, so the strike sound already covers shots, and "strikes and shots make less noise" becomes one number on the player rather than a second component and a second rule.

The trade, stated so nobody has to rediscover it: one modifier cannot say "silent steps but a loud bite".
Expressing that needs two components again, and it is not worth the second concept.

The rename is an engine-wide rename, not a game-side alias. The surface is:

- `crates/rl-bevy/src/noise.rs`: the definition, the `Sources.footfalls` query field, the use in `make_engine_noise`, two doc references, and the test `a_step_is_heard_where_it_lands_and_a_footfall_of_its_own_replaces_the_rule`, whose property changes from replacement to addition and which is renamed to match.
- `crates/rl-bevy/src/lib.rs`: the two exports, line 76 and the prelude at line 131.
- `docs/OVERVIEW.md`: the `NoisePlugin` row and the noise paragraph.
- `docs/design/noise.md`: six references, including the declaration quoted in the document and the line contrasting it with `Stealth::quiet`.
- `CHANGELOG.md`: one entry, since this is a breaking change to a published component.

Nothing in `examples/` uses `Footfall`, and it is registered nowhere in `rl-save`, so no game breaks.

### 4.2 A menu backdrop

`MenuLayout` gains a backdrop the game supplies, and the engine draws it behind the menu's rows.
The capability is the engine's and the art is the game's, so no theme word enters `rl-ui`.

The backdrop is plain data in the shape the terminal already speaks: rows of cells with a tone, never a `Color`, indexed through the `Palette` like every other widget.

### 4.3 A screen before the first run

This is the only part of the design with real unknowns, and they are plumbing rather than taste.

`begin_first_run` runs the `NewRun` schedule unconditionally in `Startup`, so a game is always in a run from its first frame.
Two things therefore need doing, and the second has a trap in it.

1. **Drawing while idle.** Drawing is gated on `world_is_shown`, which is `Playing | Over`. The title screen needs a draw path that does not require a world.
2. **Beginning a run from idle.** `restart_runs` inserts a `PendingRun` and sets the state to `Idle`, and `begin_pending_run` runs on `OnEnter(EngineState::Idle)`. A `Restart` written while already in `Idle` would insert the `PendingRun` and never fire `OnEnter`, so the run would never begin. The fix is for the pending run to be begun by a system that runs in `Idle` rather than only on entering it, which keeps one door into a run for the first and the tenth alike.

The opt-in is a setting on the engine's plugins rather than a new state: a game says whether the first run begins at startup or waits to be asked.
Games that say nothing behave exactly as they do today.

### 4.4 A Continue row

`MenuItem` gains `Continue`, offered only when a save exists, which loads it.
`MenuItem::offered` currently takes a single `playing: bool`; it grows to take what the menu can actually offer, so no caller has to know the rule.

## 5. Foundry changes

### 5.1 Ten decks

`DECKS` goes from 3 to 10.
`Foundry::generate` stamps the reactor prefab when `deck` is 3, 6 or 9, and a new `core` prefab on deck 10.
`spawn_console_on_arrival` stops naming deck three and plants a console at whatever `R` mark the deck reports, which is what makes one system serve four reactors.

`monsters.ron` spawn bands extend to cover decks 1 to 10.
Today they stop at 8, so `Roster::table.gaps(1..=DECKS)` would be non-empty and decks 9 and 10 would be empty of everything.
Existing kinds only: this is a band edit, not new content.

`lifts::deck_line` gains a line for decks 4 to 10.

### 5.2 Four charges

`quests.ron` becomes four tasks chained with `after`: `ChargeSet(3)`, `ChargeSet(6)`, `ChargeSet(9)`, `ChargeSet(10)`.
All four are `victory: false`, because victory is reaching the lift, not setting the last charge.
`On::ChargeSet(deck)` already carries the deck, so the vocabulary does not change.

`offer_the_pick` opens the pick "the moment the tracker reports the mission done".
With four tasks chained by `after`, the mission is done once, at the fourth, which would open one pick instead of four.
It therefore has to hinge on a task finishing rather than on the mission finishing, which is the one behavioural change in this section and the thing to get right first.

`upgrades::OFFERED` stops being the whole pick.
A pick offers the upgrades not yet taken, up to three, so four picks from a pool of three offer three, two, one and then nothing, and the run is still correct.
This is the minimum that makes four picks honest; section 8 grows the pool.

### 5.3 The climb

`Deepest` as described in section 3.2, the revisit branch in `populate_deck`, and the deck-banded drop filter in `drop_on_death`.

Deck one gains a lift out at its entry, spawned on first arrival, carrying no `Transition` and a `LiftOut` marker.

### 5.4 Victory, defeat and the morgue

One system in `TurnSet::React` reads `ActionRefused` for the player.
The engine refuses a `GoThrough` that finds no `Transition` under the actor and leaves the player its turn at no cost, and that refusal is the seam the game answers in:

- standing on the `LiftOut` with `ChargeSet(10)` reported: `RunOver::won()`, with Foundry's own words.
- standing on it without it: a `Tell` saying the lift will not move without the charges set.
- anywhere else: nothing, and the engine's own refusal stands.

The placeholder `RunOver::won()` comes out of `upgrades.rs`.

`main.rs` inserts a `Morgue`, which Foundry has never had, so no obituary has ever been filed.
Foundry pushes its own sections in reaction to `RunOver`: the deepest deck reached, the charges set, the upgrades picked, and what was worn and carried.
`GameMenuPanel` files them with the engine's own header, the last of the log and the character sheet.

`GameMenuPanel`'s `died` and `won` lines stop describing the first slice.

### 5.5 The title screen

Foundry supplies the backdrop: `Foundry` in block capitals, and restrained theming under it.

The bar for this is that it looks deliberate, not decorated.
Block capitals built from half-blocks and box-drawing, one warm accent against the greys for a furnace glow low on the screen, conveyor rails as horizontal rules, and nothing animated.
If it reads as clutter when it is on screen, it is cut back to the title alone rather than shipped busy.

### 5.6 Save and resume

Foundry gains a `SavePlugin` and registers what it owns, following `examples/corsair/src/save.rs`, which is the worked example and about four hundred lines.
What has to survive a save: heat, ammunition, the upgrades taken, the lamp, the mission's facts and quests, `Deepest`, the consoles and which are set, the lifts and the lift out, and every monster and item on every built deck.

Death deletes the save, which is how permadeath is enforced rather than asserted.

## 6. Slices

Each slice ends green: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and `scripts/check-tiers.sh`.
`docs/OVERVIEW.md` is updated in the same commit as any slice that adds or removes a system, and `examples/foundry/DESIGN.md`'s status marks move as each slice lands.

The fingerprint tripwire in `examples/foundry/tests/fingerprint.rs` moves on nearly every slice here.
It is re-baselined once per slice that moves it, with the reason in `CHANGELOG.md`, as its own comment requires.

1. **Ten decks.** Section 5.1. Property over a seed range: every deck of ten builds, reports an entry and an exit, and has something to spawn at its band.
2. **Four charges.** Section 5.2. A scripted run sets all four charges and takes four picks; the fourth pick offers what is left rather than panicking or repeating.
3. **The climb.** Section 5.3. Properties: a revisited deck holds kinds from the deepest band; a deck-one revisit after deck ten holds deck ten's numbers; a deep kind killed on a shallow deck drops nothing, and an unbanded item dropped by anything drops anywhere.
4. **Victory and defeat.** Section 5.4. A scripted run reaches the lift out without the charges and is refused with a line at no cost; sets the charges, returns, and wins. A death files an obituary with Foundry's sections in it.
5. **Noise unified.** Section 4.1. Engine-only and depends on nothing else here, so it can be pulled forward freely; doing it early is preferable, since it touches documents the in-flight props and remains work also touches.
6. **The title screen.** Sections 4.2, 4.3 and 5.5. The engine work is the slice; the art is the last hour of it.
7. **Save and resume.** Sections 4.4 and 5.6. Its own slice, and the largest. Properties: a run saved and loaded comes back to the same place, clock and inventory; a death leaves no save to continue.
8. **The upgrade pool.** Section 8, and content rather than loop, so it is last.

Slices 1 to 4 are the loop, and after slice 4 a run can be played from the first deck to a won or lost ending.
That is the point at which this design has paid for itself, and slices 5 to 8 can be reordered or deferred freely.

## 7. Out of scope

Named so that no reader mistakes their absence for an oversight.

- Zone tiles, per-zone light, furnace glow and collapsed sections.
- Every hazard: coolant spills, live cables, fires and vents.
- Salvagers, bounty hunters, and every droid and critter kind not already in `monsters.ron`.
- Reinforcements on a clock, the assembly line, breaches and camps, remains, and critters that feed.
- The trooper droid, and so the deck two shooter the ranged ramp describes.
- Log lines for the salvagers and hunters, which `DESIGN.md` records as undecided.

## 8. The upgrade pool, later

Four picks want a pool of at least six, so that the fourth pick still offers three.
Three exist.
Four more work on systems that exist once section 4.1 has landed: **heat sinks** (vent faster), **hydraulics** (melee harder), **dampers** (`Loudness(-n)` on the player), and **shaded lamp**.

Two of the six the game design lists are not buildable yet and would be dead picks, so they wait: **blast packing** needs grenades, and **hazard seals** resists cryo, arc and thermal, which nothing in the game deals.

One open question, to be answered before shaded lamp is built rather than during.
A droid's `notice` adds `lit_bonus` tiles of certainty while the subject stands in light.
If being lit is a boolean, a smaller lamp reduces only what the commando sees and is a pure downside, in which case shaded lamp is replaced rather than shipped.

## 9. Sources

- [Amulet of Yendor, NetHack Wiki](https://nethackwiki.com/wiki/Amulet_of_Yendor) - monster difficulty on the ascension run follows the deepest level reached, not the current one.
- [Orb Run, DCSS design wiki](https://crawl.develz.org/wiki/doku.php?id=dcss:brainstorm:dungeon:orbrun) - waves keyed to runes taken, and generated near the stairs on the way out.
- [Scumming, CrawlWiki](http://crawl.chaosforge.org/Scumming) - why monster respawn on cleared levels was removed in 0.6.0, and the principle behind it.
- [Monster creation, NetHack Wiki](https://nethackwiki.com/wiki/Monster_creation) - how difficulty gates which kinds are eligible at a depth.
- [Dungeon persistence, RogueBasin](https://www.roguebasin.com/index.php?title=Dungeon_persistence) - the shape of the choice between persistent and regenerated levels.
