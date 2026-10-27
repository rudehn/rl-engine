<!-- documents:
     plugins: MindsPlugin
     files: crates/atglyph-rules/src/ai/brain.rs
            crates/atglyph-rules/src/ai/snapshot.rs
            crates/atglyph-rules/src/ai/tactics.rs
            crates/atglyph-rules/src/ai/standard.rs
            crates/atglyph-rules/src/ai/wits.rs
            crates/atglyph-bevy/src/minds.rs
            crates/atglyph-bevy/src/knowledge.rs
            crates/atglyph-bevy/src/plugin.rs
            crates/atglyph-bevy/src/fov.rs
            crates/atglyph-bevy/src/combat.rs
            crates/atglyph-bevy/src/stealth.rs
            crates/atglyph-bevy/src/items.rs
            crates/atglyph-bevy/src/ability.rs
            crates/atglyph-bevy/src/props.rs
            crates/atglyph-bevy/src/fire.rs
            crates/atglyph-bevy/src/noise.rs
            crates/atglyph-bevy/src/effects/mod.rs
     fingerprint: aeec0d09 -->

# Minds

A mind is a priority list of tactics, asked in order, and the first that answers has spent the turn.
What it is asked about is a `Snapshot`, the world from one actor's point of view, opened when that actor's turn is dealt and filled by every plugin the game added that knows something a mind should.
The decision that comes back becomes the intent of the action that answers it, so a monster writes the same `Intent` a key does and is claimed and refused by the same resolvers.
Deciding is tier 1 and sees no world: a tactic reads the snapshot and asks for the way toward or away from cells, so what a monster does on a turn is settled by code with no `App` under it.

## Turning it on

`MindsPlugin` is every non-player deciding its own turn, and it is opt-in: a game that moves its monsters with systems of its own leaves it out.
It puts `sense` in `DecideSet::Sense`, `begin_thinking` in `PerceiveSet::Begin`, `perceive_roster` in `PerceiveSet::Roster`, `sense_posts`, `sense_leaders` and `sense_ways` in `PerceiveSet::Annotate` and `decide_minds` in `DecideSet::Minds`, and adds `MindRng`, a stream of its own, so writing one more tactic cannot shift combat's rolls.
It declares `depends_on::<FovPlugin>` and nothing else, because a mind's sight is a `Viewshed` of its own cast by the function that casts the player's.
Not combat: without `CombatPlugin` there is no faction matrix, everyone a mind sees is one of the others, and it steps round them rather than at them.
It registers the `Attack` action itself, so a blow decided in a game with no combat is refused by the sweeper rather than left holding the turn, and it registers the messages for a use, a pickup, an equip, a throw and a use of a carried thing so that a brain reaching for one in a game without that subsystem writes into a message nobody reads.
`DecideSet::Perceive` runs under `a_mind_holds_the_turn`, so a pass for the player costs no contributor a dispatch.
A `Mind` put on an entity in a game with no `MindsPlugin` is reported once, by name, rather than leaving a monster that never moves.

## The model

`Mind(Arc<Brain<Entity>>)` is the component, shared because most monsters of a kind think alike, and it requires `Intelligence`, `CameFrom`, a `Viewshed`, `Trails`, whom a mind that travels was after or beside on its last turn, and `LastTactic`, the name of the tactic that decided its last turn, or `None` when none did, which a panel reads to say what the actor is doing.
`Brain::then` appends a tactic below the ones already there, and `decide` returns the first `Decision` a tactic gave together with the name of the tactic that gave it.
A `Tactic` is a `name` for that trace, an `evaluate` returning a `Decision` or `None` to let the next one try, and `keeps_with`, whose company its turn keeps, `None` unless it says.
`Decision` is `Step`, `Attack`, `Ability`, `Wait`, `GoThrough`, `PickUp`, `EquipFromGround`, `Equip`, `Throw`, `UseItem`, `UseItemAt`, `Work` to begin work of many turns, which the brain is not asked about again until it is done or broken, or `Own(Box<dyn Choice>)` for an action of the game's own; a step onto a shut door is written as an `Open` instead, since the mind knows what it is walking into.
`Perception(i32)` is how far a mind sees and notices, `DEFAULT_PERCEPTION` of 8 without one, and the cast is a disc read through the light, so a monster in the dark sees what is lit, what its `DarkSight` reaches and what it is touching.
`Intelligence(Wits)` is what a mind is able to do whatever its brain would like: `FLEES`, `SEARCHES`, `OPENS_DOORS`, `PICKS_UP`, `EQUIPS`, `THROWS`, `USES`, `WARY`, `FOLLOWS_THROUGH` and `FLEES_THROUGH`, with `MINDLESS`, `ANIMAL` and `SAPIENT` presets, sapient unless the spawn says otherwise.
A `WARY` mind, which `SAPIENT` is and `ANIMAL` is not, prices a cell under a shown trap ten steps more in its flow fields, so it walks round one where there is a way round and over it where there is none.
`TRAVELS`, both of the last two, is in no preset: a mind that `FLEES_THROUGH` is told the ways to another map it can see, as the `Ways` sense, and takes one when `FleeWhenHurt` would reach it before any enemy, and one that `FOLLOWS_THROUGH` goes through behind whoever its `Trails` names, which a tactic fills by answering `Tactic::keeps_with`.
`Profile(MovementProfile)` is the movement class it paths with, and `CameFrom` the cell it stepped from last, so a wanderer drifts rather than dithers.
`Snapshot` carries `me`, `enemies`, `allies`, `others`, `items`, `props`, `missiles`, `usable`, `carried`, `reach`, `last_known`, `came_from`, `wits` and the game's own `senses`.
An `ActorView` holds `health` and `faction` as options, so a civilian in a game with no combat is still someone a mind sees and steps round, and `is_hurt` is false when health is unknown rather than true.
`Thinking` is that snapshot while it is being filled, plus `mark_hazard` for a cell no mind will step on and `offer_trail` for something worth walking to, the freshest offer becoming `last_known` when the snapshot closes.
The five phases fill it in turn: `Begin` opens it, `Roster` sorts everyone in sight into the three lists by the faction matrix, `Filter` is where stealth drops the unseen and the hiders the mind has not noticed and offers what it lost, and `Annotate` is where combat says how far its own shot carries, items what it carries and sees lying about and, for a mind with `Wits::USES`, what it may use from its own pack as `carried`, abilities what it may use, props what stands about, fire where not to step, hearing where a sound came from, `sense_posts` where the mind's post is, `sense_leaders` where its leader stands and `sense_ways` which ways out it sees; `Weigh` then keeps a self-aimed ability only when `Effect::would_act` says its landing would act on something.
`decide_minds` sorts the snapshot once, there and nowhere else, so the order the contributors ran in cannot reach a tactic.
`TacticCtx` then offers `step_toward` and `step_away_from` over `FlowFields`, keyed by the goal cells, the movement class, whether the walker opens doors and which way it is going, stamped with the map's cost epoch and capped at `FIELD_CACHE`, the field asked for longest ago going first: fifty hunters after one player cost one flood.
A field floods round whatever stands that is no actor, a blocking prop above all, so a mind behind a rack takes a way round it rather than walking into it.
A field toward up to four goals is composed from one flood per goal, which is cell for cell the flood from all of them, so two hunters that see different enemies share the flood from each enemy both see.
It also offers `can_step`, which refuses an occupied cell and any cell marked a hazard, `blocks_shot` and `blocks_burst`, the two predicates the ability resolver flies and bursts by, and the turn's stream.
`can_step` answers whether a cell may be stood on and says nothing about the way in, so a tactic that picks a neighbour for itself rather than taking one a field offered pairs it with the resolver's corner rule: a diagonal that squeezes between two cells the actor cannot stand on is refused silently, and a mind deciding on one would decide the same way again on every turn until something moved.
The fifteen shipped tactics are `MeleeAdjacent`, `Hunt`, `FleeWhenHurt`, `SearchLastKnown`, `Keep`, `Hover`, `KeepPost`, `Explore`, `Wander`, `GiveWay`, `UseAbility`, `UseCarried`, `ThrowAtRange`, `ShootAtRange` and `Scavenge`, and two wrappers hold any of them: `Leashed { inner, reach }` keeps its step within `reach` of the leader, and `OutOfSight(inner)` lets it decide only while the leader is out of sight.
`Standard` is the chain most monsters decide with, as a game's choices: which of `FleeWhenHurt`, `Keep::enemies`, `UseAbility`, `MeleeAdjacent`, `ShootAtRange`, `ThrowAtRange`, `Scavenge`, `Hunt`, `SearchLastKnown`, `KeepPost` and `Wander` it holds, in that order, whether running comes after the attacks instead, and every chance and reach, none of them the engine's; `Standard::NONE` holds nothing.
`Standard::brain(repertoire)` builds it for one kind, each step only where the kind's `Repertoire` can take it, a threshold to run at, a span to keep, an ability, a blow, a shot, wits that throw or take things up, and `StandardBrain` takes a game's own tactics `first`, `before_attacks`, `before_hunt`, `instead_of_hunt` and `before_wander`; `ActorParts::repertoire` reads one from a kind's parts.
`UseCarried`, only for a mind with `Wits::USES`, drinks what it carries to mend once it is down to `mend_at_pct` of its health, and otherwise points one aimed at foes at a foe or one that mends and is aimed at allies at a hurt ally, on `chance_pct` of the turns there is somewhere worth pointing it; a thing aimed at anyone or at the ground is never pointed by a mind, since the engine cannot tell what the first helps or where the second takes its user.
`Explore` steps toward the nearest cell of the `Uncharted` sense by the way there and passes when there is none it can reach; the engine keeps no record of what a mind has seen, so a game that wants explorers keeps its own and pushes `Uncharted` from it in `PerceiveSet::Annotate`.
`Post(Point)` on an actor is the cell it was set to stand on, pushed into its snapshot as the `Posted` sense, and `KeepPost` walks it back there and waits, waiting too beside a post something else stands on, and passes for an actor with no post, so one brain serves a kind whether or not this one was posted.
`Keep` is one tactic for every kind of keeping station, parameterised by what it keeps within those distances of: `Keep::allies(keep_within, no_closer_than)` is what a companion is, `Keep::enemies(..)` what a spotter or a skirmisher is, and `Keep::leader(..)` what a follower is, closing past the first distance, backing off inside the second and leaving the band between to the next tactic, a leader's or allies' gap measured on foot and an enemy's as the crow flies unless `Keep::measured(gap)` says otherwise, and it reports itself as `follow` or `shadow`, because a trace that says `shadow` says more about what a probe did than one that says `keep`.
A follower keeps on the one actor its `Leader(Entity)` component names, seen or not, through the `Led { at, in_sight }` sense `sense_leaders` pushes, and keeps with its allies instead, as `Keep::allies` would, while that leader is dead, on another map or gone; the leader lives on the follower rather than in the shared brain, so one brain serves a kind led or not, and a save keeps who leads whom.
`app.add_choice::<A>()` registers an action that is also a `Choice` and routes every `MindChose` carrying an `A` into its `Intent` in `DecideSet::Game`, and `Snapshot::add_sense` with `sense::<T>()` carries a game's own knowledge in by type, one per type.

## Using it

Warren's bestiary is a brain per kind, the standard chain at Warren's numbers built from what that kind can do, with a shove of Warren's own put first for a kind that shoves, and every rat of the kind is spawned carrying it.

<!-- include: ../../../../examples/tutorial/src/bin/step09_panels.rs:brain -->
```rust,no_run
            let wits = if def.clever { Wits::SAPIENT } else { Wits::ANIMAL };
            let can = Repertoire { flee_at: def.flee_at, melee: true, abilities: def.clever, wits, ..Repertoire::default() };
            let mut brain = if def.clever { CLEVER.brain(can) } else { WARREN.brain(can) };
            // A shover leads with its shove: Warren's own choice, routed to
            // Warren's own action.
            if def.shoves {
                brain = brain.first(ShoveAdjacent);
            }
            minds.push(Arc::new(brain.build()));
```

## The line

The engine owns when a mind is asked, what it is told and what becomes of the answer; the game owns the list of tactics, so what a monster is for is never the engine's opinion.
A mind knows only what the plugins the game added put in front of it, which is why a game with no stealth has minds that see on sight and a game with no combat has minds that see no sides.
Nothing a game extends this with is numbered: an action of its own arrives as a `Choice` found by type and knowledge of its own as a `Sense` found by type, so two games' additions cannot collide.
A game's own contributor goes in `PerceiveSet::Annotate`, its own answer to its own choice in `DecideSet::Game`, and neither orders itself after another crate's system function.
Two contributors in one phase never write the same list, and the sort at the head of the decision is the guard, so which one the executor ran first cannot reach a replay.
A game that decides one actor's turn itself claims that decision in `TurnSet::Decide`, and the stage never opens for it, so no contributor works for a decision nobody will make.
`Wits` are the engine's whole vocabulary for what a mind is able to do; anything finer, a pack that hunts together or a scent to follow, is a tactic and a `Sense` of the game's.
A post is the one such sense the engine pushes itself: the engine decides how a post is kept, and the game decides which actors have one, where, and where `KeepPost` sits in a brain, usually after `SearchLastKnown` and before its idle tactic.

## Where it lives

`atglyph-rules` is tier 1 and has no Bevy in it: `brain.rs` is `Brain`, `Tactic`, `Decision`, `Choice` and the `Fields` trait a tactic asks for the way through, `snapshot.rs` is what an actor knows, `tactics.rs` the shipped list and `wits.rs` the capabilities.
A tactic never sees a `DijkstraMap` or an `Entity`, only a `Snapshot` and a `Fields`, so one is tested against a snapshot built by hand and `NoFields` with no `App` anywhere, and the whole priority contract is arithmetic over ids made up on the spot.
`atglyph-bevy` is tier 2 and owns the perceiving and the acting: `minds.rs` has the components, `Thinking`, `FlowFields`, `decide_minds` and `add_choice`, and `plugin.rs` fixes `DecideSet` and its five `PerceiveSet` phases, because the order the contributors run in is one decision and not six.
Each contributor lives with its own subsystem rather than here, so a subsystem added later adds a system to a phase and edits nothing in `minds.rs`.
