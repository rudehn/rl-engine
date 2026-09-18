# Blows, and the log that tells you

> Run it: `cargo run -p tutorial --bin step03_blows`
>
> Source: [`step03_blows.rs`](https://github.com/rudehn/rl-engine/blob/main/examples/tutorial/src/bin/step03_blows.rs)

<div class="demo" data-demo="step03_blows">
  <img src="images/05-blows.png" alt="Two rats closing in, the log counting their bites">
  <button type="button">Play this step</button>
  <p class="weight">Loads about 8 MB</p>
</div>

Rats that hunt you in the dark, and a log that says what happened.

## Combat, and the minds that choose it

<!-- include: ../../../examples/tutorial/src/bin/step03_blows.rs:main -->
```rust,no_run
fn main() -> AppExit {
    let mut app = App::new();
    // What every game adds: the window and the glyph terminal, the turn
    // loop, sight, the map in everything but the status row and the log, and the UI base.
    app.add_plugins(RoguelikePlugins::new("Warren", COLS, ROWS).map(Rect::new(0, 1, COLS, ROWS - 1 - LOG_ROWS)))
        // Without this the world is lit everywhere and sight is geometry
        // alone. With it, `visible` shrinks to what a light reaches.
        .add_plugins((CombatPlugin, MindsPlugin, LightingPlugin))
        .insert_resource(Lighting::dark())
        .insert_resource(Seed(RunSeed(7)))
        // Two panels: the vitals strip on the top row, the log along the
        // bottom. Each draws itself; neither needs a system of yours.
        .add_plugins(VitalsPanel::new(Rect::new(0, 0, COLS, 1)).hints("[t]orch  [.]wait  [q]uit"))
        .add_plugins(LogPanel::new(Rect::new(0, ROWS - LOG_ROWS, COLS, LOG_ROWS)))
        // The engine narrates blows, deaths and pickups into the log, naming
        // things in their own colours. Warren changes one phrase: what a rat
        // does to you is a bite.
        .add_plugins(NarratorPlugin::default().phrase(Phrase::HitsYou, "{Who} bites you for {n}.", Tones::BAD))
        // Escape opens the menu, and the run's end opens it by itself.
        .add_plugins(GameMenuPanel::new(Rect::new(COLS / 2 - 20, 8, 40, 12)).died("The warren keeps you."))
        .insert_resource(Morgue::platform_default("warren", "Warren"))
        .add_systems(NewRun, start)
        // A floor fills the first time it is entered, inside the turn.
        .add_systems(Turn, populate.in_set(TurnSet::React))
        // Once a frame, before the turns: whatever the player pressed becomes
        // at most one intent, however many passes the turn loop then runs.
        .add_systems(Update, (player_input, tend_lantern).in_set(EngineSet::Input))
        .add_systems(Update, note_explored.in_set(ViewSet::Annotate));
    app.run()
}
```

Deciding where to move and deciding whom to hit are the same decision, asked of the same priority list.
`MindsPlugin` owns that decision and `CombatPlugin` owns what a blow does once it is struck, so a monster that thinks needs both.
Forget `MindsPlugin` and the first monster spawned says so in the log, instead of standing still all run.

Two panels arrive here, and neither needs a system of yours.
`VitalsPanel` reads health, armor, the turn and the position off the player and prints them along the top row; `LogPanel` prints the log along the bottom.
Each is a plugin holding the rectangle it draws in, the way the map view is.

## What a hit passes through

<!-- include: ../../../examples/tutorial/src/bin/step03_blows.rs:start -->
```rust,no_run
/// Hands the engine the map rules and the player, then warps the player in.
fn start(
    mut commands: Commands,
    seed: Res<Seed>,
    mut warps: MessageWriter<WarpRequest>,
    mut log: ResMut<MessageLog>,
    mut next: ResMut<NextState<EngineState>>,
) {
    let warren = Warren::new(seed.0);

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
    commands.insert_resource(Rats {
        // Asked in order, first that answers wins: bite what is next to
        // you, run when badly hurt, chase what you can see, else mill about.
        mind: Arc::new(Brain::new().then(MeleeAdjacent).then(FleeWhenHurt { at_pct: 30 }).then(Hunt).then(Wander { chance_pct: 40 })),
        bite: kinds.expect("bite"),
        faction: vermin,
    });

    commands.insert_resource(warren.appearance());
    commands.insert_resource(WorldMap::new(warren.tiles.tables()));
    commands.insert_resource(PlaceRulesRes(Box::new(warren)));

    let player = commands
        .spawn((
            (Actor, Player, Blocks, Position(Point::ZERO)),
            (Viewshed::new(9), RevealsMap, LANTERN, Faction(you), Glyph::new('@', Color::WHITE).on_layer(10)),
            (Health::full(24), Armor(1), MeleeAttack { kind: kinds.expect("kick"), dice: DiceRoll::new(1, 6), cost: None }),
        ))
        .id();
    warps.write(WarpRequest::into_place(player, WARREN));
    log.push(format!("Seed {}. You squeeze into the warren.", seed.0.0), Tones::NOTICE, 0);
    log.push("Something is scratching in the dark.", Tones::MUTED, 0);
    next.set(EngineState::Playing);
}
```

Damage kinds and factions are registries, like tiles.
They go in `Registries`, the one resource every subsystem reads its registries from.
`CombatRules` is only who is hostile to whom, held as a matrix over pairs instead of a flag on a monster, so a three-way war costs nothing extra.

`DamageStages` is what a hit passes through on its way to the target.
Warren has one stage, `SubtractArmor`.
Resistances by damage kind, a shield that eats the first hit each turn, a critical rule reading the attacker's stats: each is another entry in that list, in the order you put them.

Combat rolls from a stream the engine derives from the run's `Seed`, which `main` inserted in [chapter 1](01-a-map-and-walking.md).
A game never inserts a stream of its own.

## A brain is a priority list

<!-- include: ../../../examples/tutorial/src/bin/step03_blows.rs:creatures -->
```rust,no_run
/// What every rat in the warren shares: one brain, one faction, one bite.
#[derive(Resource)]
struct Rats {
    mind: Arc<Brain<Entity>>,
    bite: rl_engine::rl_rules::damage::DamageKindId,
    faction: FactionId,
}
```

Tactics are asked in order and the first that answers wins, so the reading order is the behaviour.
Bite what is next to you, run when badly hurt, chase what you can see, else mill about.

The brain holds no state about any particular rat, so sixteen rats share one `Arc`.
What a tactic needs is passed in: a snapshot of what that actor can see, its health, its position.
That snapshot is cut to the actor's own `Viewshed`, and then to its `Perception`, which is how far its mind considers what it sees.

`Hunt` does not pathfind per rat per turn.
It asks the engine for the way toward the enemies it sees, and the engine keeps one Dijkstra flow field per set of goals and movement class, so sixteen rats after one player read their downhill step off one flood.

## One key, three actions

The walk keys no longer write a `Step`.
They write a `Bump`, and the engine decides what a bump comes to: a step onto open ground, a blow at a foe standing there, or the door in the way opened.
That is an alternate action, read in `ResolveSet::Redirect`, the stage before any resolver claims the turn.

## The narrator

The engine reads every event it raises and says what happened, inside the turn, one pass at a time, so a frame in which three rats act reads in the order they acted.
Each event becomes a `Phrase`, split by who did what to whom, and the `Phrasebook` holds one template and one tone for each.
Warren changes exactly one: what a rat does to you is a bite.

A name in a template is drawn in the colour of the thing it names, so the rat in `The rat bites you for 2.` is the rat's own brown.
Take a rat's `Name` off and the log says `something`.

## Ending the run

When the player dies the engine writes `RunOver`, leaves `EngineState::Playing`, and `GameMenuPanel` opens by itself under the words Warren gave it, offering a new run or the same seed again.
`Morgue` writes the run down when it ends.

## Try it

- Add a stage that halves every hit and read the log to confirm the order.
- Reverse `MeleeAdjacent` and `Hunt` and watch rats walk past you.
- Take `DarkSight` off the rats and hunt them with the lantern shaded.

Next: [things to carry, throw and eat](04-things-and-minds.md).
