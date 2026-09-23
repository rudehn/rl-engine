<!-- documents:
     plugins: MindsPlugin
     files: crates/rl-rules/src/ai/brain.rs
            crates/rl-rules/src/ai/snapshot.rs
            crates/rl-rules/src/ai/tactics.rs
            crates/rl-rules/src/ai/wits.rs
            crates/rl-bevy/src/minds.rs
            crates/rl-bevy/src/plugin.rs
            crates/rl-bevy/src/fov.rs
            crates/rl-bevy/src/combat.rs
            crates/rl-bevy/src/stealth.rs
            crates/rl-bevy/src/items.rs
            crates/rl-bevy/src/ability.rs
            crates/rl-bevy/src/props.rs
            crates/rl-bevy/src/fire.rs
            crates/rl-bevy/src/noise.rs
     fingerprint: 2f07fb49 -->

# Minds

A mind is a priority list of tactics, asked in order, and the first that answers has spent the turn.
What it is asked about is a `Snapshot`, the world from one actor's point of view, opened when that actor's turn is dealt and filled by every plugin the game added that knows something a mind should.
The decision that comes back becomes the intent of the action that answers it, so a monster writes the same `Intent` a key does and is claimed and refused by the same resolvers.
Deciding is tier 1 and sees no world: a tactic reads the snapshot and asks for the way toward or away from cells, so what a monster does on a turn is settled by code with no `App` under it.

## Turning it on

`MindsPlugin` is every non-player deciding its own turn, and it is opt-in: a game that moves its monsters with systems of its own leaves it out.
It puts `sense` in `DecideSet::Sense`, `begin_thinking` in `PerceiveSet::Begin`, `perceive_roster` in `PerceiveSet::Roster` and `decide_minds` in `DecideSet::Minds`, and adds `MindRng`, a stream of its own, so writing one more tactic cannot shift combat's rolls.
It declares `depends_on::<FovPlugin>` and nothing else, because a mind's sight is a `Viewshed` of its own cast by the function that casts the player's.
Not combat: without `CombatPlugin` there is no faction matrix, everyone a mind sees is one of the others, and it steps round them rather than at them.
It registers the `Attack` action itself, so a blow decided in a game with no combat is refused by the sweeper rather than left holding the turn, and it registers the messages for a use, a pickup, an equip and a throw so that a brain reaching for one in a game without that subsystem writes into a message nobody reads.
`DecideSet::Perceive` runs under `a_mind_holds_the_turn`, so a pass for the player costs no contributor a dispatch.
A `Mind` put on an entity in a game with no `MindsPlugin` is reported once, by name, rather than leaving a monster that never moves.

## The model

`Mind(Arc<Brain<Entity>>)` is the component, shared because most monsters of a kind think alike, and it requires `Intelligence`, `CameFrom` and a `Viewshed`.
`Brain::then` appends a tactic below the ones already there, and `decide` returns the first `Decision` a tactic gave together with the name of the tactic that gave it.
A `Tactic` is a `name` for that trace and an `evaluate` returning a `Decision` or `None` to let the next one try.
`Decision` is `Step`, `Attack`, `Ability`, `Wait`, `PickUp`, `EquipFromGround`, `Throw`, or `Own(Box<dyn Choice>)` for an action of the game's own; a step onto a shut door is written as an `Open` instead, since the mind knows what it is walking into.
`Perception(i32)` is how far a mind sees and notices, `DEFAULT_PERCEPTION` of 8 without one, and the cast is a disc read through the light, so a monster in the dark sees what is lit, what its `DarkSight` reaches and what it is touching.
`Intelligence(Wits)` is what a mind is able to do whatever its brain would like: `FLEES`, `SEARCHES`, `OPENS_DOORS`, `PICKS_UP`, `EQUIPS` and `THROWS`, with `MINDLESS`, `ANIMAL` and `SAPIENT` presets, sapient unless the spawn says otherwise.
`Profile(MovementProfile)` is the movement class it paths with, and `CameFrom` the cell it stepped from last, so a wanderer drifts rather than dithers.
`Snapshot` carries `me`, `enemies`, `allies`, `others`, `items`, `props`, `missiles`, `usable`, `reach`, `last_known`, `came_from`, `wits` and the game's own `senses`.
An `ActorView` holds `health` and `faction` as options, so a civilian in a game with no combat is still someone a mind sees and steps round, and `is_hurt` is false when health is unknown rather than true.
`Thinking` is that snapshot while it is being filled, plus `mark_hazard` for a cell no mind will step on and `offer_trail` for something worth walking to, the freshest offer becoming `last_known` when the snapshot closes.
The four phases fill it in turn: `Begin` opens it, `Roster` sorts everyone in sight into the three lists by the faction matrix, `Filter` is where stealth drops the hiders the mind has not noticed and offers what it lost, and `Annotate` is where combat says how far its own shot carries, items what it carries and sees lying about, abilities what it may use, props what stands about, fire where not to step and hearing where a sound came from.
`decide_minds` sorts the snapshot once, there and nowhere else, so the order the contributors ran in cannot reach a tactic.
`TacticCtx` then offers `step_toward` and `step_away_from` over `FlowFields`, keyed by the goal cells, the movement class, whether the walker opens doors and which way it is going, stamped with the map's cost epoch and capped at `FIELD_CACHE`: fifty hunters after one player cost one flood.
It also offers `can_step`, which refuses an occupied cell and any cell marked a hazard, `blocks_shot`, the predicate the ability resolver uses, and the turn's stream.
`can_step` answers whether a cell may be stood on and says nothing about the way in, so a tactic that picks a neighbour for itself rather than taking one a field offered pairs it with the resolver's corner rule: a diagonal that squeezes between two cells the actor cannot stand on is refused silently, and a mind deciding on one would decide the same way again on every turn until something moved.
The twelve shipped tactics are `MeleeAdjacent`, `Hunt`, `FleeWhenHurt`, `SearchLastKnown`, `Keep`, `Hover`, `Wander`, `GiveWay`, `UseAbility`, `ThrowAtRange`, `ShootAtRange` and `Scavenge`.
`Keep` is one tactic for both sides of keeping station, parameterised by the roster it reads: `Keep::allies(keep_within, no_closer_than)` is what a companion is and `Keep::enemies(..)` what a spotter or a skirmisher is, each closing past the first distance, backing off inside the second and leaving the band between to the next tactic, and it reports itself as `follow` or `shadow`, because a trace that says `shadow` says more about what a probe did than one that says `keep`.
`app.add_choice::<A>()` registers an action that is also a `Choice` and routes every `MindChose` carrying an `A` into its `Intent` in `DecideSet::Game`, and `Snapshot::add_sense` with `sense::<T>()` carries a game's own knowledge in by type, one per type.

## Using it

Heist's watch is a brain per kind, built from that kind's own fields, and a spawn that carries it.

<!-- include: ../../../../examples/heist/src/main.rs:watch -->
```rust,no_run
    fn load(names: &Names, faction: FactionId) -> Self {
        let defs: Registry<WatchDef> = names.load(WATCH_RON).unwrap_or_else(|e| panic!("assets/watch.ron: {e}"));
        let mut table = BandedTable::default();
        let mut brains = Vec::new();
        for (id, def) in defs.iter() {
            let (lo, hi, w, gmin, gmax) = def.spawn;
            table.push(BandedEntry::new(id).bands(lo, hi).weight(w).group(gmin, gmax));
            // Strike what is in reach, hunt what is seen, search where it was
            // last seen; a watcher with hands relights the lamps on its round;
            // otherwise drift.
            let mut brain = Brain::new().then(MeleeAdjacent).then(Hunt).then(SearchLastKnown);
            if def.wits.has(Wits::OPENS_DOORS) {
                brain = brain.then(RelightLamps);
            }
            brains.push(Arc::new(brain.then(Wander { chance_pct: 30 })));
        }
        Self { defs, table, brains, faction }
    }

    fn spawn(&self, commands: &mut Commands, id: Id<WatchDef>, at: Point) -> Entity {
        let d = self.defs.get(id);
        commands
            .spawn((
                (Actor, Blocks, Kind(id), Position(at), Speed(d.speed), Faction(self.faction), Health::full(d.hp), Armor(d.armor)),
                (
                    MeleeAttack::new(d.kind.id(), d.attack),
                    Perception(d.perception),
                    DarkSight(d.dark_sight),
                    Notice(d.notice),
                    Hearing(d.hearing),
                    Mind(self.brains[id.index()].clone()),
                    Intelligence(d.wits),
                    Name::new(d.name.clone()),
                    Glyph::new(d.glyph, Color::srgb(d.color.0, d.color.1, d.color.2)).on_layer(5),
                ),
            ))
            .id()
    }
```

## The line

The engine owns when a mind is asked, what it is told and what becomes of the answer; the game owns the list of tactics, so what a monster is for is never the engine's opinion.
A mind knows only what the plugins the game added put in front of it, which is why a game with no stealth has minds that see on sight and a game with no combat has minds that see no sides.
Nothing a game extends this with is numbered: an action of its own arrives as a `Choice` found by type and knowledge of its own as a `Sense` found by type, so two games' additions cannot collide.
A game's own contributor goes in `PerceiveSet::Annotate`, its own answer to its own choice in `DecideSet::Game`, and neither orders itself after another crate's system function.
Two contributors in one phase never write the same list, and the sort at the head of the decision is the guard, so which one the executor ran first cannot reach a replay.
A game that decides one actor's turn itself claims that decision in `TurnSet::Decide`, and the stage never opens for it, so no contributor works for a decision nobody will make.
`Wits` are the engine's whole vocabulary for what a mind is able to do; anything finer, a post to return to or a pack that hunts together, is a tactic and a `Sense` of the game's.

## Where it lives

`rl-rules` is tier 1 and has no Bevy in it: `brain.rs` is `Brain`, `Tactic`, `Decision`, `Choice` and the `Fields` trait a tactic asks for the way through, `snapshot.rs` is what an actor knows, `tactics.rs` the shipped list and `wits.rs` the capabilities.
A tactic never sees a `DijkstraMap` or an `Entity`, only a `Snapshot` and a `Fields`, so one is tested against a snapshot built by hand and `NoFields` with no `App` anywhere, and the whole priority contract is arithmetic over ids made up on the spot.
`rl-bevy` is tier 2 and owns the perceiving and the acting: `minds.rs` has the components, `Thinking`, `FlowFields`, `decide_minds` and `add_choice`, and `plugin.rs` fixes `DecideSet` and its four `PerceiveSet` phases, because the order the contributors run in is one decision and not six.
Each contributor lives with its own subsystem rather than here, so a subsystem added later adds a system to a phase and edits nothing in `minds.rs`.
