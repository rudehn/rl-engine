# Minds: how a non-player decides, and why it is built this way

Written 2026-09-16 as the plan for section 2 of `docs/TODO.md`, "Open the minds", after three reviews of a first draft, and built the same day in the six stages of section 4.
Section 3 describes the subsystem as it stands; sections 2 and 4 are its history.

## 1. What a mind is

A `Mind` is a shared `Brain`, a priority list of `Tactic`s from `atglyph-rules`.
On its turn the engine builds a `Snapshot`, the world from that actor's point of view, hands it to the brain with a `TacticCtx`, and turns the `Decision` that comes back into the intent of the action that answers it.
`atglyph-rules` decides and never acts; `atglyph-bevy` perceives and acts and never decides.

## 2. What was wrong

`decide_minds` in `crates/atglyph-bevy/src/minds.rs` was one system of two hundred lines that knew about combat, items, throwing, abilities, lighting, stealth and fire.
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
- `Annotate`: items and throwing fill `missiles` and `items`; abilities copy `UsableAbilities` into `usable`; fire marks `hazards`; a game pushes its own `Sense`.

`decide_minds` is then the last step: sort the snapshot once, build `TacticCtx`, run the brain, write the intent.
Sorting happens there and nowhere else, so the order contributors ran in cannot reach a tactic.

A game's own knowledge is a `Sense`, `Any + Send + Sync + Debug`, pushed onto `Snapshot::senses` and read back by type with `snapshot.sense::<T>()`.
Typed and open, with no id to collide.
`Snapshot` gives up `Clone` and `PartialEq` for it, which nothing used.

### 3.3 A typed choice, routed once

`Decision::Own(Box<dyn Choice>)` replaces `Decision::Game(u32)`, with `Choice: Any + Send + Sync + Debug` and a `name` for the trace, in `atglyph-rules`.
`Decision` keeps `PartialEq`, by hand, comparing an `Own` by its name.

The routing is written once, in `atglyph-bevy`: `app.add_choice::<A: Action + Choice + Clone>()` registers the action, its sweeper, and a system in `DecideSet::Game` that turns a `MindChose` carrying an `A` into an `Intent<A>`.
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
A companion could not take the stairs when this was written; anyone travels now, when its wits say so, section 10.

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

Pack behaviour, keep-at-range, noise and scent (leader behaviour and patrol with a post are built; section 7).
Each is a new tactic over the seams above; `docs/design/fields.md` already names `TileField<T>` as the substrate for noise and scent.

## 7. What part 3 built

Three slices landed on the seams section 3 opened, each a tactic or a perceive contributor and none a change to `decide_minds` itself.

- **Using what it carries.** `Wits::USES` is a fourth capability a kind either has or does not, read the way `OPENS_DOORS` is.
  `ItemsPlugin` fills `Snapshot::carried` in `PerceiveSet::Annotate`, a `Carried { item, targeting, mends }` per usable thing a mind with the wit may use, never one whose use asks for another thing, since nothing in a tactic chooses which.
  `UseCarried` reads it the way `UseAbility` reads `usable`: on itself what mends once it is hurt past `mend_at_pct`, and otherwise what is aimed at foes at a foe or what mends and is aimed at allies at a hurt ally, through `Aim::worth_aiming_at`, the same test an ability's aim is weighed by.
  A thing aimed at anyone or at the ground is left alone on purpose: the snapshot cannot say whether the first helps whoever it lands on, and the second may carry its user somewhere no tactic has reasoned about.
- **Following a leader.** `Keep` gained a third roster, `Station::Leader`, beside the allies and enemies section 3.5 already kept station on.
  The leader is not in the brain, since brains are shared and a leader is not: it lives on the follower as `Leader(Entity)`, and the new `sense_leaders`, chained with `sense_posts` in `PerceiveSet::Annotate`, pushes where it stands as `Led { at, in_sight }`, seen or not, so a follower that loses sight of its chief at a corner does not lose it from its snapshot too.
  While the leader is dead, elsewhere or gone, `Keep::leader` keeps with the follower's allies instead, the way `Keep::allies` always has, so a band that loses its chief still holds together.
  `atglyph-save` maps the follower's `Leader` across a continued run through the entity map, the same device every saved relationship between two actors uses.
- **Weighing a self-aimed ability.** `UseAbility` cast any ability aimed only at its own caster whenever a foe was in sight, a necromancer blinking with nowhere to go and a healer mending itself whole.
  The new `Effect::would_act(landing, world)`, read by `AbilitiesPlugin`'s `weigh_self_abilities` in `DecideSet::Perceive` after `PerceiveSet::Annotate`, asks each self-aimed ability's effects whether its landing, built on the caster's own cell exactly as the resolver would land it, would act on anything; `Snapshot::usable` keeps only the ones that would, or any with no effects to ask, since a game answers those itself.
  The question is asked of the landing's targets rather than its cells, since a self-aimed shape that flies, a teleport among them, covers no cells of its own in the landing the resolver lands; asking the cells first read a blink as never mending and never worth keeping.

None of the three touches `decide_minds`, `TacticCtx` or the sort at the head of `Minds`: a capability is a wit, a roster is a `Station`, and a weighed ability is a smaller `usable` the existing contract never has to know was filtered.

## 8. A kind of monster made of parts

Until 2026-10-14 each game spawned its monsters by hand: [Undermere](https://github.com/rudehn/undermere)'s `spawn_monster` and Foundry's put the same health, armor, side, sight, speed, mind, wits, notice, hearing, attacks and drops on an actor in two hand-written bundles, and each saved a monster as its definition's name and spawned it again from that name, and each read its spawn table row by row.
Items had just stopped doing the same thing (`docs/design/items.md`, section 6a), and the reasoning carries over whole: what is shared is the shape of what the engine already simulates, never a schema, and the defaults are the game's.

`atglyph_bevy::ActorParts<I>` is every part of one kind of actor, the actor's `ItemParts`, and a game fills one per definition at load from its own fields.
Each field maps onto a component the engine already plays, `hp` onto `Health`, `wits` onto `Intelligence`, `notice` onto `Notice`, `statuses` onto what `ActiveStatuses` holds for life, `leaves_remains` onto `LeavesRemains`, `drops` onto `Drops<I>`, or onto a tactic the brain is built with, `flee_at` and `keep_away`.
Every field is optional and absent means no mechanic: no notice is a mind that never notices what tries not to be seen, no remains is a death that leaves nothing, no hearing is deaf.
A number written is inserted as written, so a game that writes no armor has no `Armor` and one that writes `Some(0)` has `Armor(0)`; the engine never decides that a nought is the same as nothing.
`ActorParts::insert` puts them on an entity the game spawned with its own marker, its name and its look, rather than spawning one itself, so an actor spawned by hand, as the tutorial's are, and one spawned from parts are the same kind of entity.

A game's defaults are another `ActorParts`, laid under each kind with `ActorParts::over`: a value the kind writes wins, and a list the defaults hold is added to.
[Undermere](https://github.com/rudehn/undermere)'s is that every monster notices by its own `NOTICE`, sure from four tiles further while the adventurer is lit, and Foundry's that every kind notices by the engine's plain rule and leaves remains; the engine gives a kind that names neither no notice and no remains.
A default passed to the engine, an `ActorDefaults` beside the parts or an argument to the spawn, was weighed and rejected: the defaults are only a kind of parts, `over` is the rule `Level::over` already is for items, and a game that wants none writes none.

What stays in the games is what the engine does not simulate, and the line is drawn per field.
[Undermere](https://github.com/rudehn/undermere)'s traits and their fold, its tricks, its boss, mini-boss and conjured kinds, its sleep and its camps are Undermere's; the fold runs first and the parts are filled from the folded values, and whether a kind leaves a body, never a boss or a conjured one, is Undermere's decision written into the part.
A kind whose wits take things up gets a pack, slots and the worth of its own blow from [Undermere](https://github.com/rudehn/undermere), since what a monster does with what it carries is a rule of Undermere's and not a part.
[Foundry](https://github.com/rudehn/foundry)'s resist profile, its alarm, its repairs and its radar are Foundry's, and its radar is why `dark_sight` is a part Foundry leaves empty: `NativeDarkSight` is folded with what a droid wears and whether it is jammed before anything reads `DarkSight`.
A shot, a blow, a light and the span a kind keeps from its enemies are parts both games type their fields with, [Foundry](https://github.com/rudehn/foundry)'s `keep_away` among them, and where in a brain a game keeps that span is its own, Foundry's in place of the hunt and beside `Hover`.

A monster is saved the way an item is (`docs/design/items.md`, section 7a): `atglyph_save::SaveableActor`, registered with `save_actors`, writes which definition it is by its maker's `name_of` and the game's own extra beside it, and restores it through `ActorMaker::make_at`, nowhere and with nothing rolled, before the extra goes back on, since what a game keeps is a change to what it was made as: a side it was turned to, a mind it was given, a look a wreck lost.
`ActorMaker::make` calls `make_at` unless a game rolls something on a monster, so a game that rolls nothing writes one function.

Where each kind turns up is one file shape in both games, read by `atglyph_rules::load_spawns`: `(monster, bands: (first, last), weight, group: (fewest, most))`, `group` optional, into the `BandedTable` a population and a prefab's role draw from.
The band's name is the engine's, `bands` as the item spawn file's is, since a floor and a deck are both a band a maker answers for.

## 9. The standard chain

Every game with monsters wrote one brain by hand, and wrote it the same way: run when hurt, keep a span, reach for an ability, strike, shoot, throw, fetch, hunt, search, walk back to a post and drift, each step guarded by whether the kind can.
`atglyph_rules::ai::Standard` is that chain, in [Undermere](https://github.com/rudehn/undermere)'s order, the one the user chose as the default, and only the engine's own tactics are in it.

A game writes its chain once as a constant: which steps it holds, a `bool` for each step that needs no number, and every number a step does need, the chance of reaching for an ability, of throwing and of a wanderer's step, and how far a scavenger goes, as an `Option`.
`Standard::NONE` holds nothing and the engine has no number of its own for any step, so a step whose number a game never gave is not in its chain, and which of `Hunt`, `SearchLastKnown`, `KeepPost` and `Wander` close the chain is the game's to say: the tutorial's rats neither search nor keep a post.
A shot is taken on every turn there is one, the tactic's own default and not a rule number, since a chance below a hundred is a game's to give a tactic by hand.

`Repertoire` is what one kind can do, read from its parts by `ActorParts::repertoire` or written by hand by a game that keeps none, and a step is in a kind's brain only where the chain holds it and the kind can take it: a threshold above nought to run at, a span to keep, an ability known, a blow, a shot, wits that throw, and wits that take things up, `Wits::takes_things`, the rule `Scavenge` fetches by.
So the chain says what a game's monsters do and the parts say which of it each kind can, and a kind that never runs still has no `FleeWhenHurt` in its brain.

Where flee goes is the one choice of order: `flee_after_attacks` puts it after the blow, the shot and the throw, as [Foundry](https://github.com/rudehn/foundry) and the tutorial have it, a droid trading the blow it has in hand before it goes, where Undermere runs first because a kind its examine panel says runs when hurt must not fight on while its enemy stands beside it.
Everything else a game adds goes in at five named points: `first`, ahead of running, [Undermere](https://github.com/rudehn/undermere)'s flight after a theft and Foundry's alarm; `before_attacks`, after running and the span kept and before the ability, Undermere's mercenary mending and pointing what it carries with `UseCarried`; `before_hunt`, after every attack, Undermere's pack, led and flit; `instead_of_hunt`, which leaves `Hunt` out, Foundry's repairs and its span kept beside `Hover`; and `before_wander`, Undermere's patrol.
`before_attacks` was left out until [Undermere](https://github.com/rudehn/undermere)'s mercenaries needed it on 2026-10-14: without it a game whose tactic must come after running had to put its own `FleeWhenHurt` in `first`, writing again the guard on the kind's threshold the chain exists to hold.
A point between one attack and the next is still not needed by any game and is not there; a game wanting another order builds its `Brain` by hand, as it always could.

[Undermere](https://github.com/rudehn/undermere)'s brains for an ally and a freed captive are the same builder over a second constant: run, ability, blow and shot as their kind, then `Keep::allies` or `Keep::leader` before the hunt, and nothing after it.
A hired mercenary's three, one for each order, are the builder over a third constant that closes nothing, at its own throw chance of always, and each order closes it: hold by `KeepPost`, fight freely by `Hunt` with the leader kept and gear fetched after it, and follow by a hunt leashed to the adventurer in the hunt's place.
Each game's chain, tactic by tactic and in order, is what it was before the builder, which the fingerprints hold.

## 10. Following, leaving, and putting on

Three pieces [Undermere](https://github.com/rudehn/undermere)'s party wrote for itself are the engine's since 2026-10-14, each without a rule number in it.

`Leashed { inner, reach }` and `OutOfSight(inner)` wrap any tactic over the engine's own `Led` sense: the first lets through no step that ends further than `reach` from the leader, and lets every step through with no leader on the map; the second decides only while the leader is out of sight.
Wrappers rather than fields on `Hunt` and `Keep`, because what is held in is any tactic, a hunt, a fetch or a game's own, and a field on each would be the same rule written in every tactic that steps.
How far the leash reaches is the game's: [Undermere](https://github.com/rudehn/undermere)'s is its `LEASH` of three, and the engine has none.

`Decision::Equip(item)` is putting on something carried, answered by the resolver that answers the player's `Equip`.
It had been a game's own choice relayed to the engine's intent, which is a decision of the engine's under another name; when a mind arms, and with what, stays the game's, and no engine tactic decides it.

Leaving a map is two wits, `Wits::FOLLOWS_THROUGH` and `Wits::FLEES_THROUGH`, where it was one.
A hireling that comes down the stairs behind the player and never runs down them alone wanted one without the other, and taking the whole wit off it to stop the running took the following too.
Content's `travels` reads as both, so nothing written before the split changed meaning, and `Wits::TRAVELS` names the pair.

## 11. What a status does to whoever holds it

Until 2026-10-15 [Undermere](https://github.com/rudehn/undermere) answered four of its statuses by name in systems of its own: a confused body's steps turned a random way on a stream of Undermere's, a discordant one's side changed and kept in a `Discordant { was }`, fire immunity's hundred percent written into the holder's own `Resists` behind a `Fireproofed` marker, and a wraith's `Wither` effect refusing a fourth stack of `weakened`.
A troll's mending halted while it burns was a fifth, keyed by a `BurnsBadly` marker, and the rail's saying what holds a monster a sixth, reading nothing but engine fields.
Each is a mechanic any game might want and none should have to take, so each is now an optional field, absent meaning no mechanic, and every number is the game's.

`StatusDef` gains `max`, the most instances a `Stack` status holds at once, refused past it in `Statuses::put` rather than refreshing one, since which one would be refreshed is a rule nobody wrote; `resists`, what its holder resists while it lasts, per instance; `side`, the faction its holder fights for; and `stumbles`, the percent of its holder's steps and bumps turned a random open way.
A cap on a status that never stacks is refused at load, since nothing would ever play it.
Which factions there are and who is hostile to whom stay the game's: a status only names one of them.

What a status lends is kept every pass from what is held, in `ReactSet::Statuses`, rather than answered as the status comes and goes, so a status a save puts back does it again with nothing else saved.
The side is the exception that has to be saved: once the holder's `Faction` is the status's, the side it goes back to is nowhere else, so it is `SideWas`, and `EntityState::side_was` puts it back as the holder's side for the status to turn it from again.
What a status resists goes in its own `StatusResists`, which `Loadout::resistances` adds, rather than into the holder's `Resists`, so taking it off is exact whatever else moved them.
It is kept once a pass rather than read live where a blow is weighed: a draught drunk and a burn landed in one pass are weighed as things stood before it, and the last tick of a burn is still resisted by a ward running out that pass, which is how [Undermere](https://github.com/rudehn/undermere) played it and so how the engine does.

A kind's own rule for a status, burning halting a troll's mending and nobody else's, is not the status's to say, so it is `ActorParts::while_holding`, a list of stat changes by status that becomes `WhileHolding` on the actor and lands under `Source::Holding(status)`.
A field on `StatusDef` keyed by kind was weighed and rejected: the status would then name every kind it treats differently, which is the bestiary written into the status table.

A stumble rewrites the intent where it stands, in `DecideSet::Statuses`, a stage after the game's own answer, so what a game wrote is turned too and nothing reading the intent later reads the old way.
It draws from `StumbleRng`, an engine stream, only where an open way exists and, below a hundred percent, once more for whether to stumble at all, so a holder that always stumbles spends exactly one draw a step.
The run's seed is asked for by the statuses only when one of them stumbles, through a requirement that holds only then, since `CorePlugin` has never needed a seed and a game whose statuses draw nothing should not start to.
[Undermere](https://github.com/rudehn/undermere)'s fingerprint did not move with the stream: its scripted delve is never confused.

The rail's saying of it is `NearbyStatuses`, off unless a game asks with `NearbyPanel::statuses(key, asleep)`: every status with a badge as a facet under the game's key, by its registered name, and a sleeper as doing the game's sleep word.
[Undermere](https://github.com/rudehn/undermere)'s confusion gas, `Discord`'s choice of target, every status name and number, and the lantern going out while shrouded stay Undermere's.

## 12. How the fields and the gap moved

Four changes to how a mind walks came out of [Undermere](https://github.com/rudehn/undermere)'s dungeon and party, the first three moving a fingerprint over the minds they touch and the last none, since a composed field is cell for cell the flood it stands for.

- **A field floods round what stands.** A blocker that is no actor, a blocking prop above all, is a wall to `FlowFields` as it always was to the move resolver: a field that flooded the tiles alone walked a mind into a post or a rack, which the resolver refused, and it stood there with a way round a few steps off. What stands is read off `Occupancy` when it changes, and fields are thrown away only when the cells it stands on differ, so an actor's step costs no flood.
- **A wary mind walks round a shown trap.** `Wits::WARY`, in the sapient preset, prices a cell under a trap anyone can see ten steps more, rather than refusing it, since a refused cell would leave a follower standing behind a trap in a corridor for good; a beast still blunders onto one, which is what a player lures it there for, and the cell its target or leader stands on is never priced.
- **A gap is measured on foot.** `Keep::allies` and `Keep::leader` measure by the walk the mind already closes down, `Gap` and `Fields::on_foot`, where the crow's distance stood a follower three cells off across a wall while the walk to its leader was five; `Keep::enemies` keeps the crow's, since what it holds is a line of sight.
- **Fields are kept longer and shared wider.** The field asked for longest ago goes first when the cache is full, rather than all of them at once, and a field toward up to four goals is composed from one flood per goal, so two hunters that see different enemies share the flood from each enemy both see.
