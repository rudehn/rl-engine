# Undermere Town Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Mereholt is a place with people in it: shops whose goods are rolled each run, a temple whose rites cost gold, townsfolk named afresh each run with something to say, rumours true of this run, and a waystone that brings the adventurer home and the gate that takes them back to where they knelt.

**Architecture:** Two engine slices first (trade between two actors, with its screen; an offering with a price), then the game on them: peace in the town, every thing's worth, the shops, the temple, the waystone and the gate mark, the townsfolk and their talk, the rumours, the rail, and the soak's gold.

**Tech Stack:** Rust, Bevy 0.19, `serde`, `ron`, the workspace's own crates.

**Spec:** `docs/superpowers/specs/2026-10-08-undermere-town-design.md`, with `examples/undermere/DESIGN.md` ("The town").

This plan names files, interfaces and the tests to write, and leaves the code to the implementer.

## Global Constraints

- Work in `.claude/worktrees/undermere-town` on branch `undermere-town`, branched from `undermere` at `9eebfed` while part 3 is still being built there and part 4 on `undermere-dungeon`; never `cd` to the main checkout or to another worktree.
- Part 3's actor offers, captives and `Work` for the adventurer are the base; `examples/undermere/src/captives.rs` is the model for answering an offer in the game.
- Names come from the `namegen` branch (part 6's `rl-rules::namegen`), being built by another session; task 9 merges it in when it has merged, and waits for it otherwise. Nothing else depends on it.
- Biome rumours read part 4's draws, which are on `undermere-dungeon`; task 10 writes every rumour now and tests the boss's and the mini-bosses', and the biomes' test lands when part 4 merges.
- Every engine slice: a test in its crate, doc comments that say why, no theme words, a `CHANGELOG.md` line; every game keeps passing; a moved fingerprint is explained before it is re-baselined.
- Lean process (`undermere-lean-process`): no per-task review for game content, one fresh agent per task, only the touched crates' tests and `clippy -p` during a task, the full gate once at the end.
- Commit in stages; keep each tool call short (read about 200 lines, edit about 120 at a time, every long output to a scratchpad file).
- The screen may be locked while the user is away: a black `RL_CAPTURE` means judge from the painted terminal and list the photographs owed.

## Review Focus

1. A shop never tells what the adventurer does not know: an unknown level sells at nought, an unknown kind and a curse's sign have no worth, and a shelf lists a ware in the shop's colour, not its face. Pinned in tasks 5 and 6.
2. A waystone is never an escape: harm breaks the kneeling and keeps the stone, and the gate never leads past the mark. Pinned in task 8.
3. The money is the pack's coins: buying, a rite and selling move real stacks, and the score's treasure never counts the purse or a sale. Pinned in tasks 1, 5 and 7.
4. A run's people and shelves are the seed's: the same seed meets the same names and the same stockings, through a save. Pinned in tasks 6 and 9.

---

## Tasks

### Task 1: Trade between two actors

**Files:** `crates/rl-bevy/src/trade.rs` (new: `TradePlugin`, `Worth(u32)`, `Trader { buys, pays_pct }`, the currency tag, `Buy`/`Sell` requests resolved at once, a `Traded` answer and refusals: short, not bought here, no worth), `crates/rl-bevy/src/lib.rs` and the prelude, `crates/rl-bevy/src/props.rs` only for the `trade` verb opening a trade, `CHANGELOG.md`.
- [ ] Tests: a buy moves the ware into the buyer's pack and its worth in currency, splitting and merging stacks, and takes no turn; a sell pays the seller's share rounded down; short gold, a kind the seller does not buy and a thing with no worth are each refused and say which; a stocked seller's inventory survives `rl-save` as any actor's does.
- [ ] Commit: "two actors can trade: one sells what it carries at its worth and buys what it names at a share, paid in a currency the game tags".

### Task 2: The trade screen

**Files:** `crates/rl-ui/src/view/trade.rs` and `crates/rl-ui/src/panel/trade.rs` (new: `TradeView` of the seller's rows with prices, the buyer's rows with what the seller would pay, the buyer's currency and a game line; `TradePanel` on the modal stack, as `panel/container.rs` is), `crates/rl-ui/src/lib.rs`, `CHANGELOG.md`.
- [ ] Tests: the collector lists both sides with their prices and greys a refused row with its reason; the game's line comes through a facet; the panel opens on the `trade` offer and closes when the seller is out of reach.
- [ ] Commit: "a trade screen: what they sell and what they would pay, side by side, with the gold to hand".

### Task 3: An offering with a price

**Files:** `crates/rl-bevy/src/props.rs` (`Offering` gains `price`; `Refused::Short`; the price spent before `Interacted`), `crates/rl-ui/src/view/offers.rs` (the price on the row), `CHANGELOG.md`.
- [ ] Tests: an offer whose taker carries less is refused and says how much; taken up, the price leaves the taker's coins before the answer runs; an offer with no price is unchanged.
- [ ] Commit: "an actor's offer may have a price, refused to whoever carries less and spent when taken up".

### Task 4: Peace in Mereholt

**Files:** `examples/undermere/src/damage.rs` (a stage after the armor stage dropping harm on the town's map), the `townsfolk` faction where factions are declared (`content.rs`).
- [ ] Tests: a blow and a firebolt in Mereholt harm nobody; the same below still harm; a townsperson bumped is talked to, not struck.
- [ ] Commit: "nothing is harmed in Mereholt, and its people are at peace with the adventurer".

### Task 5: Every thing's worth, and a purse

**Files:** `examples/undermere/assets/items.ron` (a `price:` per item, the spec's tables; the header comment lists it), `examples/undermere/src/armory.rs` (a system keeping `Worth` current from the price, the known level at 100 a level, and none for an unknown kind, a curse's sign or an artifact sold; the curio's 400 for an artifact bought), `examples/undermere/src/run.rs` (50 gold in the kit).
- [ ] Tests: a thing of unknown level is worth its nought price over a span of seeds; an unknown kind and a cursed sign have no worth; knowing a level raises it; the purse is not counted as treasure, and neither is gold from a sale.
- [ ] Commit: "every thing has a worth, counted from what the adventurer knows of it, and the adventurer walks out with fifty gold".

### Task 6: The shops

**Files:** `examples/undermere/assets/wares.ron` (new, with its header comment: per keeper, parts of rows drawn as a `LootTable` at a band, and a shared `levels` table as a `LevelTable`), `examples/undermere/src/shops.rs` (new: stocking from `Seed::stream("wares", stocking)` at the run's start and on returning from a deeper tier; a ware shown in the shop's colour, not its face; buying makes its kind known), `examples/undermere/assets/prefabs/mereholt.ron` (a mark per post), keepers spawned with `Trader`, `Offers(trade)` and their stock.
- [ ] Tests: the same seed stocks the same shelves; a return from the same tier finds the shelves less what was bought, and from a deeper one a new stocking; a keeper never stocks below nought; buying a potion makes its kind known and browsing does not; each keeper buys only its kinds.
- [ ] Photograph: the smithy's screen open in Mereholt.
- [ ] Commit: "Mereholt's four shops, stocked from the run's seed and restocked as the delve goes deeper".

### Task 7: The temple

**Files:** `examples/undermere/assets/rites.ron` (new, with its header comment: a row per verb, its price and its effects in `items.ron`'s effect words), `examples/undermere/src/temple.rs` (new: the priest offering `trade` for waystones and `mend`, `unbind`, `name` at their prices; the rites answered in `Interacted`).
- [ ] Tests: each rite lands its effects and spends its price; each is refused when the gold is short; `name` makes everything worn known and nothing carried.
- [ ] Commit: "the temple of the Still Water mends, unbinds and names, for a price".

### Task 8: The waystone and the gate mark

**Files:** `examples/undermere/assets/items.ron` (the waystone), `examples/undermere/assets/props.ron` (the gate mark, lit violet), `examples/undermere/src/waystone.rs` (new: the use beginning `Work` "calling the gate"; on `WorkDone` the warp to the gate's town side and the mark; `Travelled` written as a way's), `examples/undermere/src/stairs.rs` (the gate's `Transition` rewritten to the mark), `examples/undermere/src/run.rs` (one in the kit), `item_spawns.ron` (a rare one deeper), `save.rs`.
- [ ] Tests: a blow breaks the kneeling and the stone is kept; finished, the adventurer stands at the gate in Mereholt and the gate leads to the mark, through a save; a second call moves the mark; with no mark the gate leads to floor one; a waystone is refused in Mereholt and says why.
- [ ] Commit: "a waystone calls the gate after ten quiet turns, and the gate takes the adventurer back to where they knelt".

### Task 9: The people of Mereholt

**Files:** merge `namegen` first; `examples/undermere/assets/names.ron` (a `townsfolk` set), `examples/undermere/assets/townsfolk.ron` (new, with its header comment: role words, glyph, colour, post, keeps or wanders, offers, lines with `when`), `examples/undermere/src/townsfolk.rs` (new: names drawn distinct at the run's start and saved; the `talk` answer writing the next line that holds, `{role}` said as that role's name; the role as a look facet; the keepers' greeting as the trade screen's line).
- [ ] Tests: names are drawn distinct from each other and from every name the run has given, over a span of seeds, and survive a save; a line's `when` holds it back until it holds; the last line repeats; `{gatewarden}` says the gatewarden's name.
- [ ] Photograph: Mereholt at dusk with its people, and a line in the log.
- [ ] Commit: "Mereholt's people, named afresh each run, each with something to say".

### Task 10: Rumours

**Files:** `examples/undermere/assets/rumours.ron` (new, with its header comment), `examples/undermere/src/townsfolk.rs` (the innkeeper's `talk` says the next rumour: the boss, the mini-bosses drawn, then the biomes drawn from more than one).
- [ ] Tests: every rumour said is of a draw the run fixed when it started; the boss's rumour never names the boss; the order holds and ends on the fixed last line. The biomes' case waits on part 4.
- [ ] Commit: "the innkeeper tells what this run's dungeon holds, true and never exact".

### Task 11: The rail, and the soak's gold

**Files:** `examples/undermere/src/` where the vitals are annotated (the gold facet), the gate mark kept in what the adventurer remembers, `examples/undermere/tests/soak.rs` (the gold found by each floor over its seeds); then the prices in `items.ron` and `wares.ron` checked against what it counts, and moved if the estimate was wrong.
- [ ] Tests: the vitals show the gold carried; a seen gate mark is remembered.
- [ ] Commit: "the rail counts the gold carried, and the soak counts the gold each floor gives".

### Task 12: Documentation pass, whole-part review, and the merge

**Files:** `docs/guide/src/systems/trade.md` (new) and the offers and props pages, `docs/design/trade.md` (new, and listed in `CLAUDE.md`'s layout and `docs/README.md`), `docs/OVERVIEW.md`, `README.md`'s feature list, `CHANGELOG.md`, `examples/undermere/DESIGN.md` (what part 5 built and left).
- [ ] The full gate: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `scripts/check-tiers.sh --wasm`, `scripts/check-overview.sh`, `scripts/check-guide.sh`, `python3 scripts/check-systems.py`.
- [ ] Whole-part review (opus) against the spec and the Review Focus; its fix wave.
- [ ] Merge into `undermere` once part 3 has finished there.
