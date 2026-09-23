# Minds: how a non-player decides, and why it is built this way

Written 2026-09-16 as the plan for section 2 of `docs/TODO.md`, "Open the minds", after three reviews of a first draft, and built the same day in the six stages of section 4.
Section 3 describes the subsystem as it stands; sections 2 and 4 are its history.

## 1. What a mind is

A `Mind` is a shared `Brain`, a priority list of `Tactic`s from `rl-rules`.
On its turn the engine builds a `Snapshot`, the world from that actor's point of view, hands it to the brain with a `TacticCtx`, and turns the `Decision` that comes back into the intent of the action that answers it.
`rl-rules` decides and never acts; `rl-bevy` perceives and acts and never decides.

## 2. What was wrong

`decide_minds` in `crates/rl-bevy/src/minds.rs` was one system of two hundred lines that knew about combat, items, throwing, abilities, lighting, stealth and fire.
Every new subsystem edited it, and a game had no way to put its own knowledge in front of its own tactic.

- Every actor needed `Health` and `Faction` to be perceived at all, so a civilian was invisible to every mind and a game with no combat could not field one.
- `Decision::Game(u32)` was a magic number answered by a match, the `Custom { id }` shape one level down.
- The player's viewshed was the one line-of-sight oracle: a monster saw only what the player had a line to, and two monsters could not fight where the player was not looking.
  The `Perception(20)` bullet in guide chapter 4 was false because of it.
- `FlowFields` was built only toward the player, so a companion could not be followed and an item could not be pathed to.

## 3. The design

### 3.1 Every actor has its own sight

Decision, Nate, 2026-09-16: actors carry their own `Viewshed`, so the sight systems are the same for the player and for monsters.

A `Mind` requires a `Viewshed`; a monster's spawn gives it one sized by its `Perception`, and `Perception` is that radius.
`FovPlugin`'s `update_viewsheds` is the only thing that casts sight, once a frame for everything stale.
Inside the turn loop dozens of monsters move within one frame, so the decide phase recasts the viewshed of the actor holding the turn when it is dirty, through the same function, before anything perceives.
A monster that did not move pays nothing; the player's viewshed is refreshed the same way and the frame's pass then finds it clean.

`Watchers`, noticing and every perceive contributor read `Viewshed::can_see` off the watcher's own component, so the panels, the notice roll and the minds cannot disagree about who could be seen, and none of them needs the player.
`perceivable` and the player-as-oracle are gone.

The viewshed's range is a disc, as the player's is.
The old reach was a Chebyshev square, so a monster loses the corners of its old square; that is the consistency the decision asks for, and the shipped games' `perception` values are retuned in the same commit.

### 3.2 A perceive stage every plugin fills

`Thinking` is a `CorePlugin` resource, always present: the actor whose turn it is, the `Snapshot` under construction, and `hazards`, a `BitGrid` over the window of cells no mind will step into.
It is in the core, beside `Acting` and `FlowFields`, so `CombatPlugin` in a game without minds neither panics nor gates on a resource that happens to be there.
It is empty when no mind holds the turn or a game has already claimed the decision, which short-circuits every contributor.

`DecideSet` becomes `Sense, Notice, Offer, Perceive, Minds, Game`.
`Perceive` has four phases, `PerceiveSet::{Begin, Roster, Filter, Annotate}`, configured in `CorePlugin` the way `ViewSet` phases the panels:

- `Begin`: the minds open the snapshot for the actor holding the turn, after `DecideSet::Sense` recast its sight.
- `Roster`: the minds sort everyone the thinker can see into `enemies`, `allies` and `others` by the faction matrix when combat inserted one, or all into `others` without one.
- `Filter`: stealth removes hiders the thinker has not noticed and fills `last_known`.
- `Annotate`: items and throwing fill `missiles` and `items`; abilities copy `Offered` into `usable`; fire marks `hazards`; a game pushes its own `Sense`.

`decide_minds` is then the last step: sort the snapshot once, build `TacticCtx`, run the brain, write the intent.
Sorting happens there and nowhere else, so the order contributors ran in cannot reach a tactic.

A game's own knowledge is a `Sense`, `Any + Send + Sync + Debug`, pushed onto `Snapshot::senses` and read back by type with `snapshot.sense::<T>()`.
Typed and open, with no id to collide.
`Snapshot` gives up `Clone` and `PartialEq` for it, which nothing used.

### 3.3 A typed choice, routed once

`Decision::Own(Box<dyn Choice>)` replaces `Decision::Game(u32)`, with `Choice: Any + Send + Sync + Debug` and a `name` for the trace, in `rl-rules`.
`Decision` keeps `PartialEq`, by hand, comparing an `Own` by its name.

The routing is written once, in `rl-bevy`: `app.add_choice::<A: Action + Choice + Clone>()` registers the action, its sweeper, and a system in `DecideSet::Game` that turns a `MindChose` carrying an `A` into an `Intent<A>`.
A game writes the same code tutorial chapter 9 already teaches for a player action, plus one line, and a choice nobody routed is refused rather than lost.

Declined: `Brain<A, C>` with the choice as a type parameter would be fully static, but every game with no choices of its own would write `MindsPlugin::<()>`, and the common case should not pay for the rare one.

### 3.4 Minds without combat

`ActorView` carries `health: Option<Vitals>` and `faction: Option<FactionId>`, where `Vitals { current, max }` mirrors the component.
`hp_pct` returns `Option<i32>` and `is_hurt` is false when health is unknown, so no call site silently reads a prop as wounded.
`FleeWhenHurt` never flees without health.

`Snapshot::others` holds everyone perceived who is neither enemy nor ally, and `GiveWay` is the first tactic over it, so a civilian steps out of a crowd's way without the game writing the tactic.
Factions are allied with themselves, so a townsperson on the player's side is an ally, not an other.

`MindsPlugin` no longer depends on `CombatPlugin`.
It registers the `Attack` action itself, so a blow nobody resolves is refused rather than left holding the turn, and it rolls from `MindRng`, a stream of its own, so adding a tactic can no longer shift combat's rolls.
Stealth goes the same way: `StealthRng` for the notice roll, and `Option<Res<CombatRules>>`, absent meaning every subject is a subject.

Decision, Nate, 2026-09-16: `Health` is two fields, `current` and `max`.

### 3.5 Flow fields toward any goal

`FlowFields` is keyed by `(goals, profile, opens_doors, away)`, where `goals` is a sorted list of cells, and each entry is stamped with the map's cost epoch, so a field survives across passes and frames until the map or the goals change, as the player's does today.

The tactics never see a `DijkstraMap`.
`TacticCtx` offers `step_toward(goals)` and `step_away_from(goals)`, which return the first steppable descent, so the map stays window-local and the per-call `shift_map` allocation is gone.
`Hunt` asks toward every enemy it sees, `FleeWhenHurt` away from them, `SearchLastKnown` toward `last_known`, `Scavenge` toward the item, and `Keep` toward every one of whichever roster it keeps station on, so a whole faction descends one flood rather than one per monster.

`Keep::allies(keep_within, no_closer_than)` is what a companion is, and `Keep::enemies(..)` a spotter: one tactic, since the only thing that ever differed between the two was the roster it read.
A companion cannot yet take the stairs; `WarpRequest` and `GoThrough` remain the player's, and `docs/TODO.md` tracks "anyone travels".

## 4. Order of work

Each stage green and committed on its own, with `docs/OVERVIEW.md`, the changelog and the plan's progress log in the same commit.

1. `Health { current, max }`, a mechanical rename.
2. Pre-work: sort the notice subjects before rolling, break `Snapshot::sort` ties on identity, and add a seeded-run fingerprint tripwire over minds, combat and stealth, labelled as such, so the re-baseline in stage 6 is a red test rather than a bug report.
3. Slice 1, the perceive stage.
4. Slice 2, `add_choice`, with a section in guide chapter 9 for the first worked example of a mind choosing a game's action.
5. Slice 5, fields toward any goal, and keeping station on allies.
6. Slices 3 and 4 together, since both re-baseline every seeded outcome: own viewsheds, stealth and minds without combat, their own streams, and the retuned content, with one changelog line saying replays recorded before it will not replay.

## 5. Risks

- **Every recorded replay before stage 6 desyncs.** The minds drew from `CombatRng` once per turn and noticing rolled once per subject in view; both change. No recordings are committed, so this is a changelog line and the tripwire.
- **Stealth gets harder.** Cutthroats in Corsair's daylight and beasts around the delve's corners now see round their own corners. The counterplay exists in both games; the content is retuned, and a game's first move on the new engine is to lower its `perception` values.
- **Cost.** One shadowcast per moved monster per turn, into a reused window-sized grid, is far cheaper than the Dijkstra flood the engine already runs per player move.
  Every viewshed recasts together on an opacity change, which is one cast per monster on a door opening; acceptable, and measured if it is not.
- **Contributor order.** Two contributors in one phase that both push onto one list would leak the executor's order; the phases are chosen so no two contributors write the same list, and the sort at the head of `Minds` is the guard.

## 6. What waits

Pack and leader behaviour, keep-at-range, patrol with a post, noise and scent.
Each is a new tactic over the seams above; `docs/design/fields.md` already names `TileField<T>` as the substrate for noise and scent.
Patrol needs per-actor state the shared brain cannot hold, which is a `Sense` a game or a later slice fills.
