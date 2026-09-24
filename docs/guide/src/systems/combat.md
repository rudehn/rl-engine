<!-- documents:
     plugins: CombatPlugin
     files: crates/rl-bevy/src/combat.rs
            crates/rl-bevy/src/bump.rs
            crates/rl-bevy/src/cue.rs
            crates/rl-bevy/src/plugin.rs
            crates/rl-bevy/src/state.rs
            crates/rl-bevy/src/turn.rs
            crates/rl-rules/src/damage.rs
            crates/rl-rules/src/faction.rs
            crates/rl-rules/src/forecast.rs
     fingerprint: 4dbfcde8 -->

# Combat and loadout

A blow is an `Attack` on an entity: struck in reach with what the attacker wields, fired at range down a clear line of fire, and a spent turn when neither is possible.
What it rolls is never read off the attacker alone, because `Loadout` sums it at the moment it matters from the actor's own components, every worn item's, and the stats `CombatRules` names.
The roll becomes a `Hit`, the hit goes down a pipeline of stages the game composed, and what comes out the far end is taken off `Health`.
Above that line everything is the game's: who hates whom, what a kind of damage is, what stops it, and what a death means beyond a body on the floor.

## Turning it on

`CombatPlugin` declares `needs::<CombatRules>`, hinting that one comes from `CombatRules::new(&sides)`, and `needs::<Registries>` for the damage kinds a blow can deal.
It takes `CombatRng`, a stream of its own derived from the run's seed, so a game never inserts one and a weapon added late cannot shift the rolls of a run that was going fine.
It registers `Attack` as an action, `DamageEvent`, `DamageDealt`, `DeathEvent` and `Struck` as messages, `ShotLanding` as something that can be in the air, and `DamageStages` as a resource whose default is `SubtractArmor` alone.
Its systems are `perceive_reach` in `PerceiveSet::Annotate`, `land_shots` in `LandSet::Shot` chained ahead of `resolve_attacks` in `ResolveSet::Act`, `apply_damage` in `ResolveSet::Damage`, `end_run_on_player_death` in `TurnSet::React` and `process_deaths` in `CleanupSet::Remove`.
`bury_the_dead` is the exception and runs in `Last`, which is how the dead are promised to linger until the frame ends without naming one of the systems that has to see them go.
`Resists` is a component rather than a requirement, so an actor with none meets an empty ladder and a game with no resistances pays nothing.
Nothing here decides who strikes whom: minds choose for monsters, `Bump` turns a walk key into an `Attack` when a foe is in the way, and an ability's damage arrives as the same `DamageEvent` a sword's does.
`CorePlugin` registers `Intent<Attack>` itself so that a bump into a foe runs in a game with no combat at all, and `add_action::<Attack>` adds the sweeper that refuses one nobody resolved.

## The model

`Health` is `current` and `max`, and `Health::full(n)` is both.
`Faction` puts an actor on a side, and `CombatRules` is the matrix over the sides the game registered: `hostile` sets a pair both ways, `hunts` sets one way only, and `allied` sets help both ways, over a `Factions` that starts allied with itself and neutral to everyone else.
A grudge that is not returned is the reason the matrix is dense and asymmetric rather than a set of pairs.
`CombatRules` also carries `armor` and `attack`, each an optional `StatId`, and `death_ends_run`, which `death_is_not_the_end()` clears for a game that revives or plays on as a ghost.
`MeleeAttack` is a `kind`, a `dice` roll, an optional `cost` in hundredths of a step and an optional `look`; `RangedAttack` is the same with a `range`.
Both are built by `new` and narrowed by `costing` and `looking`, so a field only some games want is added without touching every call site.
`Armor` is flat damage removed and `Strikes` is a list of extra rolls every hit carries, a flaming blade's fire or a venomed edge's poison.
All four sit on an actor or on an item, and that is the whole of how gear fights: a jerkin is an item with `Armor(1)` and nothing copies the 1 onto whoever puts it on.
`Loadout` is the one answer to what an entity fights with, in three layers: the actor's own components, the same components on every item in its `Equipped` slots in slot order, and the value of the stat `CombatRules` names.
A worn blow or shot replaces the actor's own, since a cutlass is swung in place of a fist, while armor, resistances and extra strikes add up.
A worn thing whose `Consumable` is empty lends no blow or shot, so a spent wand is not fired and its wearer falls back on what is left.
`armor`, `resistances`, `melee`, `ranged` and `strikes` are the sums; `melee_with` and `ranged_with` also say which worn item it came from; `blows` is the melee roll followed by the strikes, which is every roll one blow lands.
The attack resolver strikes with it, `apply_damage` defends with it, and `blows` and its ranged twin `shots` are what a forecast is filled from, so what a panel says a fight will cost is worked out from the numbers the fight uses.
`Loadout::arms` packs both of those, both costs and the shot's reach into a `forecast::Arms`, and `Combatant::armed` reads it at the distance the caller passes: one cell away is the melee rolls, further is the shot while the shot reaches, and past its reach is nothing at all.
That is the rule `resolve_attacks` picks by, kept in one place, so an actor carrying only a gun forecasts as dangerous across the room and harmless once you are beside it rather than as harmless everywhere.
`resolve_attacks` picks melee when the two are adjacent and otherwise a shot filtered by `line_of_fire`; an attack with nothing that reaches still spends an ordinary turn, since what was spent was the aim.
It writes `Struck` before any damage, naming the worn item the attack came from, because what a weapon does to itself happens at the trigger rather than at the target.
For a worn item it also reports the `fire` moment at the attacker's cell, and the `hit` moment at the target's cell when the blow or shot lands, so a wand's charge is spent and its effects land through [Effects](effects.md) without combat knowing what either is.
A shot takes its item's triggers with it as it is fired, so a watched shot from a thing its last charge spent still lands what its hits carry, from a remnant in its place.
Every roll is floored at zero where it is rolled, so a weapon with a bad bonus that rolls low has missed rather than healed.
An attack with a `Look` is seen: a shot cues a `Cue::Flight` and a blow a `Cue::Burst`, and while something watches the cues a shot's hits wait in `Airborne<ShotLanding>` until the flight has been seen.
`land_shots` then drops them on a target still standing, so one killed while the shot flew is missed rather than hurt twice.
`shot` is where a projectile goes and `line_of_fire` is that call landing on the cell it was pointed at; a targeting preview draws the same call, so what the player is shown and what the resolver decides cannot disagree.
A `DamageEvent` carries a `Hit`, which separates `attacker`, who triggers on-hit riders, from `credit`, who gets the kill, so a poison tick credits whoever applied it without recursing its own riders; `critical` and `status` are there for the stages and narrators that care.
It also carries a `Reach`, which is how the damage got there: `DamageEvent::new` is `Effect`, what did not travel as a weapon, and `arriving` names `Melee`, `Shot` or `Thrown` instead, carried through to `DamageDealt` for whoever puts it into words and read by nothing in the pipeline.
`apply_damage` builds a `Defender` and the resistances from the target's `Loadout`, runs `resolve` over the game's `DamageStages`, takes the result off health capped at `max`, and writes `DamageDealt` and, at zero, `DeathEvent`.
An `Invulnerable` target keeps a heal and takes no harm: the hit is still written to `DamageDealt`, with nothing dealt, so a narrator says it had no effect rather than saying nothing.
A `DamageKind` is a name and whether armor applies to it, and `Resistances` is a percentage per kind: 100 is immunity, a negative number is vulnerability, and above 100 absorbs the hit into healing.
The engine ships three stages, `SubtractArmor`, `ApplyResistance` and `HalveIfBlocked`, and the default list holds the first alone.
A negative amount is a mend and goes down the same stages, which is why resistance scales a heal and immunity means nothing can patch the defender up.
`process_deaths` takes a dead non-player out of the world, the queue and the occupancy index and marks it `Dead`; `end_run_on_player_death` writes `RunOver` inside the turn, so the monster that would have struck the corpse never gets its move.
`perceive_reach` is combat's one word to a mind: how far its own shot reaches, read from `Loadout::ranged` whether the gun is worn or is the monster itself.

## Using it

What a game hands combat is two registries and two rules, which is the whole of it in the tutorial's third step.

<!-- include: ../../../../examples/tutorial/src/bin/step03_blows.rs:combat -->
```rust,no_run
    // The two registries combat reads: what damage can be, and who hates
    // whom. Both are the game's content, named nowhere in the engine.
    let kinds = Registry::from_defs(vec![DamageKind::new("bite"), DamageKind::new("kick")]).unwrap();
    let sides = Registry::from_defs(vec![FactionDef::new("you"), FactionDef::new("vermin")]).unwrap();
    let (you, vermin) = (sides.expect("you"), sides.expect("vermin"));
    commands.insert_resource(CombatRules::new(&sides).hostile(you, vermin));
    commands.insert_resource(Registries { damage_kinds: kinds.clone(), factions: sides, ..default() });
    // What a hit passes through on its way to the target. One stage here;
    // resistances, a shield, a critical rule would each be another.
    commands.insert_resource(DamageStages(vec![Box::new(SubtractArmor)]));
```

## The line

The engine decides whether a blow is in reach, whether a shot has a line, what it is struck with, what it costs, what it rolls, the order the stages run in, what comes off health and who died.
Which of an actor's two attacks a forecast counts is the engine's for the same reason: a panel hands over both sets of rolls and how far apart the two stand, and `Arms::at` picks, so what a screen says about a fight and what the resolver does in it cannot drift apart.
The game decides what a damage kind is and whether armor applies to it, who hates whom, what mitigates a hit, and what a hit or a death is worth beyond health reaching zero.
`DamageStages` is a list of boxed `DamageStage`s, so there is no enum of mitigations and no `Custom` arm: a game's critical rule sits in the list beside `SubtractArmor` and `resolve` cannot tell them apart.
`Defender::blocked` is never set by the engine, which builds one with `blocked: false` every time, so `HalveIfBlocked` is for a caller that fills its own and a game that blocks rolls the block inside a stage of its own.
Accuracy does not exist either: a blow lands unconditionally, and a to-hit roll when a game wants one is a stage that returns zero rather than a change to the resolver.
`armor_stat` and `attack_stat` are the whole seam between the registered stats and a blow, which is why a status that hardens the skin and an affix that sharpens the hand both work by moving a stat and neither is named in combat.
A game that registered no stats names neither, and its armor is components alone.
What a weapon does to itself is the game's, hung on `Struck`: heat, ammunition, wear, each read off a message that already names the item, so no game recomputes which weapon the loadout would have chosen.
The player's death ends the run unless `death_is_not_the_end()` says otherwise, because a game that revives keeps the ending for itself.
Experience, levels, kill credit beyond `Hit::credit`, wound locations and morale are each a game's rule over these messages, and the engine should not guess which of them a game wants.
Arithmetic that is really about the rules lives in `rl-rules`, and what that buys is a test with no `App` in it: a game's own stage is proved against a `Hit` and a `Defender` filled by hand, and a panel's verdict about a fight is proved against the same call a real blow makes.

## Where it lives

`rl-rules` is tier 1 and has no Bevy in it: `damage.rs` is `Hit`, `Defender`, the `DamageStage` trait and a `resolve` that is a fold over stages, all of it tested against ids made out of thin air.
`faction.rs` is the dense matrix, which is a table and an index rather than anything that needs a world.
`forecast.rs` is where the split earns its keep: `expected_damage` calls the same `resolve` with the average roll in place of a real one and through the game's own stages, so a panel that says a fight is deadly got the word from the arithmetic the fight will use.
`Arms` is what a fight fought at a distance costs it: both sets of rolls, both costs and the shot's reach, with `Arms::at` the one place the choice between them is made and `Combatant` still one set already chosen.
The distance is an argument because only the caller knows where the two stand, and the one thing `Arms::at` will not check is whether the line of fire is clear, since a forecast a wall may yet block is still the right forecast for the fight the two would have.
`rl-bevy` is tier 2 and owns everything that touches the world: `combat.rs` is the components, `Loadout`, the resolver, the pipeline runner and the deaths, in one file because a blow is one decision and not six.
`bump.rs` is beside it rather than inside it, since a walk key that comes to a blow is the turn loop's redirection and works the same in a game with doors and no foes.
