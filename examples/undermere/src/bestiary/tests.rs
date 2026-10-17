use bevy::ecs::system::RunSystemOnce;
use bevy::ecs::world::CommandQueue;
use rand::SeedableRng;
use rl_engine::rl_core::Rect;

use super::*;
use crate::content::Library;
use rl_engine::rl_rules::parts::KeepAway;

/// The deepest floor, the throne's.
const DEEPEST: i32 = 10;

fn library() -> Library {
    Library::load(&crate::content::effect_kinds(), &Moments::default())
}

/// `text`, a monsters file of a test's own, read against the shipped
/// registries, traits and items.
fn fixture(library: &Library, text: &str) -> Result<Registry<MonsterDef>, ContentError> {
    read(text, &library.registries, &library.traits, &library.armory.defs, &crate::content::effect_kinds(), &Moments::default())
}

/// Every reason a fixture that should not read was refused.
fn refused(library: &Library, text: &str) -> Vec<String> {
    match fixture(library, text) {
        Err(ContentError::Invalid(errors)) => errors,
        Err(other) => panic!("refused for the wrong reason: {other}"),
        Ok(_) => panic!("it read"),
    }
}

/// Spawns `def` at the origin of floor one in a bare world.
fn spawn(library: &Library, def: &MonsterDef) -> (World, Entity) {
    let mut world = World::new();
    let mut queue = CommandQueue::default();
    let mut commands = Commands::new(&mut queue, &world);
    let e = spawn_monster(&mut commands, def, Point::ZERO, MapId(1), &library.registries, false);
    queue.apply(&mut world);
    (world, e)
}

/// The name of the tactic `e`'s mind decides with at `hp` of `max`,
/// an enemy of the adventurer's side standing beside it.
fn decides(world: &World, e: Entity, hp: i32, max: i32, library: &Library) -> Option<&'static str> {
    let brain = world.get::<Mind>(e).expect("a mind").0.clone();
    let me = ActorView { id: e, pos: Point::ZERO, health: Some(Vitals { current: hp, max }), faction: world.get::<Faction>(e).map(|f| f.0) };
    let foe = ActorView {
        id: Entity::PLACEHOLDER,
        pos: Point::new(1, 0),
        health: Some(Vitals { current: 10, max: 10 }),
        faction: Some(library.registries.factions.expect("adventurer")),
    };
    let mut snapshot = Snapshot::alone(me);
    snapshot.enemies.push(foe);
    snapshot.wits = world.get::<Intelligence>(e).expect("wits").0;
    let mut rng = rand::rngs::StdRng::seed_from_u64(1);
    let ctx = &mut TacticCtx {
        snapshot: &snapshot,
        fields: &mut NoFields,
        can_step: &|_| true,
        blocks_shot: &|_| false,
        blocks_burst: &|_| false,
        bounds: Rect::new(-20, -20, 40, 40),
        rng: &mut rng,
    };
    brain.decide(ctx).1
}

/// A tripwire over the bestiary, by name, so a kind is added or
/// dropped on purpose, and the spectral blade a staff conjures; and
/// the three floor ten bosses, any one of whose death wins the run.
#[test]
fn every_monster_of_the_bestiary_is_in_the_file_and_the_floor_ten_boss_is_one_of_three() {
    let library = library();
    let bestiary_kinds = [
        "rat",
        "jackal",
        "kobold",
        "goblin",
        "monkey",
        "cave spider",
        "giant spider",
        "spiderling",
        "goblin archer",
        "goblin conjurer",
        "goblin mystic",
        "myconid",
        "myconid elder",
        "pink jelly",
        "vampire bat",
        "ogre",
        "skeleton",
        "ghoul",
        "necromancer",
        "wraith",
        "will-o'-wisp",
        "drowned pilgrim",
        "drowned acolyte",
        "pale scribe",
        "drowned guard",
        "drowned thrall",
        "wight",
        "troll",
        "golem",
        "dar blademaster",
        "dar priestess",
        "dar battlemage",
        "fire imp",
        "salamander",
        "skeleton knight",
        "the Drowned King",
        "the Mother Below",
        "spawnling",
        "the Ember Wyrm",
        "Grubnak the goblin king",
        "the Widow",
        "the Choirmaster",
        "the Drowned Abbess",
        "the Forgemaster",
        "spectral blade",
        "mercenary",
        "moth",
        "blind cave rat",
    ];
    let defs = &library.bestiary.defs;
    for name in bestiary_kinds {
        assert!(defs.id(name).is_some(), "no monster called {name:?}");
    }
    assert_eq!(defs.len(), bestiary_kinds.len(), "a monster this list does not name");
    let bosses: Vec<&str> = defs.iter().filter(|(_, d)| d.boss).map(|(_, d)| d.name.as_str()).collect();
    assert_eq!(bosses, ["the Drowned King", "the Mother Below", "the Ember Wyrm"]);
    assert!(defs.iter().all(|(id, d)| d.id() == id), "each knows its own id");
}

#[test]
fn a_monster_is_its_core_and_its_traits_folded_together() {
    let library = library();
    let defs = fixture(
        &library,
        r#"[(name: "test hound", glyph: 'h', color: (0.5, 0.4, 0.3), hp: 7, attack: (roll: "1d4+1", kind: "pierce"), armor: 1, faction: "beasts", traits: ["animal", "swift", "thick-hided"])]"#,
    )
    .expect("a hound reads");
    let hound = defs.get(defs.expect("test hound"));
    assert_eq!((hound.glyph, hound.color, hound.hp), ('h', (0.5, 0.4, 0.3), 7), "the core as written");
    assert_eq!(hound.attack.roll, DiceRoll { num: 1, sides: 4, bonus: 1 });
    assert_eq!(hound.attack.kind.id(), library.registries.damage_kinds.expect("pierce"));
    assert_eq!(hound.folded.speed, Some(150), "swift");
    assert_eq!(hound.speed(), 150);
    assert_eq!(hound.folded.wits, Wits::ANIMAL, "animal");
    assert_eq!(hound.armor, 3, "its own and its hide's");
    assert_eq!(hound.traits, ["animal", "swift", "thick-hided"]);
    assert_eq!(hound.perception(), DEFAULT_PERCEPTION, "no trait says how far it sees");
    assert_eq!(hound.flee_at(), 0, "no trait says it runs");
}

/// A shot is the engine's shot part, a blow's roll and kind with a
/// range, and both name a kind the registries hold.
#[test]
fn a_shot_reads_its_dice_its_kind_and_its_range() {
    let library = library();
    let archer = library.bestiary.defs.get(library.bestiary.defs.expect("goblin archer"));
    let shot = archer.ranged.expect("the archer shoots");
    assert_eq!((shot.roll, shot.kind.id(), shot.range), (DiceRoll::new(1, 6), library.registries.damage_kinds.expect("pierce"), 6));
    assert_eq!(archer.attack.roll, DiceRoll::new(1, 3), "and still has a blow");
}

#[test]
fn an_attack_that_does_not_read_is_refused_naming_where_it_is() {
    let library = library();
    let errors = refused(
        &library,
        r#"[
            (name: "a", glyph: 'a', color: (1.0, 1.0, 1.0), hp: 1, attack: (roll: "1q4", kind: "slash"), faction: "beasts", traits: []),
            (name: "b", glyph: 'b', color: (1.0, 1.0, 1.0), hp: 1, attack: (roll: "1d4", kind: "slsh"), faction: "beasts", traits: []),
            (name: "c", glyph: 'c', color: (1.0, 1.0, 1.0), hp: 1, attack: (roll: "1d4"), faction: "beasts", traits: []),
            (name: "d", glyph: 'd', color: (1.0, 1.0, 1.0), hp: 1, attack: (roll: "1d4", kind: "slash"), ranged: (roll: "1d6", kind: "pierce"), faction: "beasts", traits: []),
            (name: "e", glyph: 'e', color: (1.0, 1.0, 1.0), hp: 1, attack: (roll: "1d4", kind: "slash", range: 3), faction: "beasts", traits: []),
        ]"#,
    );
    assert_eq!(errors.len(), 5, "{errors:#?}");
    let expected = ["entry 1: ", "b: unknown damage kind \"slsh\"", "entry 3: ", "entry 4: ", "entry 5: "];
    for (says, error) in expected.iter().zip(&errors) {
        assert!(error.starts_with(says), "{error}");
    }
}

#[test]
fn a_shot_that_reaches_no_tile_is_refused_naming_the_monster() {
    let library = library();
    let errors = refused(
        &library,
        r#"[(name: "f", glyph: 'f', color: (1.0, 1.0, 1.0), hp: 1, attack: (roll: "1d4", kind: "slash"), ranged: (range: 0, roll: "1d6", kind: "pierce"), faction: "beasts", traits: [])]"#,
    );
    assert_eq!(errors, ["f: ranged: a shot reaches one tile or more"]);
}

#[test]
fn an_unknown_trait_is_refused_by_name() {
    let library = library();
    let errors = refused(
        &library,
        r#"[(name: "jackal", glyph: 'j', color: (1.0, 1.0, 1.0), hp: 6, attack: (roll: "1d3", kind: "pierce"), faction: "beasts", traits: ["animal", "swfit"])]"#,
    );
    assert_eq!(errors, ["jackal: unknown trait \"swfit\""]);
}

/// Review Focus 5, where the game meets it: a monster whose two traits
/// set one value stops the load, naming the monster, the value and
/// both traits.
#[test]
fn a_monster_whose_traits_set_one_value_twice_is_refused_naming_it_the_field_and_both_traits() {
    let library = library();
    let errors = refused(
        &library,
        r#"[(name: "jackal", glyph: 'j', color: (1.0, 1.0, 1.0), hp: 6, attack: (roll: "1d3", kind: "pierce"), faction: "beasts", traits: ["animal", "swift", "slow"])]"#,
    );
    assert_eq!(errors, ["jackal: `speed` is set by both `swift` and `slow`"]);
}

#[test]
fn a_monster_lists_each_trait_once() {
    let library = library();
    let errors = refused(
        &library,
        r#"[(name: "rat", glyph: 'r', color: (1.0, 1.0, 1.0), hp: 4, attack: (roll: "1d3", kind: "pierce"), faction: "beasts", traits: ["animal", "regenerates", "regenerates"])]"#,
    );
    assert_eq!(errors, ["rat: lists `regenerates` twice"]);
}

/// The Drowned King's knights are the throne's alone: its piece stands
/// them on floor ten, at its slots, and a table that could draw one would
/// put his court out walking a floor. That every floor has something to
/// draw and no row draws a boss is the bestiary's load, which
/// [`spawn_problems`] holds it to.
#[test]
fn the_court_is_drawn_on_no_floor_of_the_spawn_table() {
    let library = library();
    let bestiary = &library.bestiary;
    let court = bestiary.defs.expect("skeleton knight");
    assert!((1..=DEEPEST).all(|floor| bestiary.spawns.at(floor).all(|row| row.item != court)), "the court is the throne's alone, drawn on no floor");
}

/// A floor with nothing to draw and a row drawing a boss are each refused
/// by name, and a boss row past the floors, or of nought weight, is not.
#[test]
fn a_spawn_table_with_a_floor_left_empty_or_a_boss_drawn_on_a_floor_is_refused_naming_each() {
    let library = library();
    let defs = &library.bestiary.defs;
    let (king, rat) = (defs.expect("the Drowned King"), defs.expect("rat"));
    let table = BandedTable::new(vec![
        BandedEntry::new(rat).bands(1, 9),
        BandedEntry::new(king).bands(4, 4).weight(1),
        BandedEntry::new(king).bands(11, 12).weight(1),
        BandedEntry::new(king).bands(1, 10).weight(0),
    ]);
    assert_eq!(spawn_problems(defs, &table), ["floor 10 has nothing to draw", "the Drowned King is a boss, and drawn on floors 4 to 4"]);
    assert_eq!(spawn_problems(defs, &library.bestiary.spawns), Vec::<String>::new(), "the shipped table");
}

#[test]
fn every_drop_and_every_item_spawn_names_a_real_item() {
    let library = library();
    let (bestiary, armory) = (&library.bestiary, &library.armory);
    let mut drops = 0;
    for (_, def) in bestiary.defs.iter() {
        for row in &def.parts.drops.0 {
            assert!(armory.defs.try_get(row.item).is_some(), "{} drops an item the armory does not have", def.name);
            drops += 1;
        }
    }
    assert!(drops > 0, "something drops something");
    let gold = armory.defs.expect("gold coin");
    for name in ["kobold", "goblin", "goblin archer", "ogre"] {
        let def = bestiary.defs.get(bestiary.defs.expect(name));
        assert!(def.parts.drops.0.iter().any(|row| row.item == gold), "{name} carries gold");
    }
    assert!(bestiary.defs.get(bestiary.defs.expect("the Drowned King")).parts.drops.0.is_empty(), "the run is over when he falls");
    for row in armory.table.rows() {
        assert!(armory.defs.try_get(row.item).is_some(), "a spawn row names an item the armory does not have");
    }
    let errors = refused(
        &library,
        r#"[(name: "magpie", glyph: 'm', color: (1.0, 1.0, 1.0), hp: 2, attack: (roll: "1d2", kind: "pierce"), faction: "beasts", traits: [], drops: [("goblet", 50)])]"#,
    );
    assert_eq!(errors, ["magpie: unknown item \"goblet\""]);
}

/// Each of the five mini-bosses drops its own artifact, and only its
/// own, certainly: every row of its `drops` names its artifact at a
/// hundred in a hundred.
#[test]
fn each_mini_boss_drops_its_own_artifact_and_only_its_own() {
    let library = library();
    let bestiary = &library.bestiary;
    let bosses = [
        ("Grubnak the goblin king", "the Crown of Grubnak"),
        ("the Widow", "the Widow's Fang"),
        ("the Choirmaster", "the Choirmaster's Staff"),
        ("the Drowned Abbess", "the Abbess's Chalice"),
        ("the Forgemaster", "the Forgemaster's Hammer"),
    ];
    for (boss, artifact) in bosses {
        let def = bestiary.defs.get(bestiary.defs.expect(boss));
        let item = library.armory.defs.expect(artifact);
        assert_eq!(def.parts.drops.0.iter().map(|row| row.item).collect::<Vec<_>>(), vec![item], "{boss} drops only {artifact}");
        assert_eq!(def.parts.drops.0[0].pct, 100, "{boss} always drops {artifact}");
    }
}

/// The traits reach the engine's own components: a glow is a light
/// the monster carries, and a coward's threshold is where its mind
/// turns from fighting to running, with an enemy beside it.
#[test]
fn a_spawned_monster_carries_what_its_traits_gave_it() {
    let library = library();
    let defs = fixture(
        &library,
        r#"[(name: "lamp beetle", glyph: 'b', color: (0.4, 0.9, 0.7), hp: 10, attack: (roll: "1d3", kind: "pierce"), faction: "beasts", traits: ["fungal glow", "cowardly"])]"#,
    )
    .expect("a beetle reads");
    let beetle = defs.get(defs.expect("lamp beetle"));
    let (world, e) = spawn(&library, beetle);
    let glow = library.traits.get(library.traits.expect("fungal glow")).light.map(LightSource).expect("fungal glow is a light");
    assert_eq!(world.get::<LightSource>(e), Some(&glow));
    let threshold = library.traits.get(library.traits.expect("cowardly")).flee_at.expect("cowardly runs at some share");
    let at = threshold * beetle.hp / 100;
    assert_eq!(decides(&world, e, at, beetle.hp, &library), Some("flee_when_hurt"), "at {at} of {}, it runs", beetle.hp);
    assert_eq!(decides(&world, e, at + 1, beetle.hp, &library), Some("melee_adjacent"), "above it, it fights");
}

/// The king, as the file writes him: every one of his traits reaches
/// what the engine reads, and his core's armor is his.
#[test]
fn the_drowned_king_spawns_with_everything_his_traits_give_him() {
    let library = library();
    let king = library.bestiary.defs.get(library.bestiary.defs.expect("the Drowned King"));
    let (world, e) = spawn(&library, king);
    let registries = &library.registries;
    assert_eq!(world.get::<Kind>(e), Some(&Kind(king.id())));
    assert_eq!(world.get::<Armor>(e).map(|a| a.0), Some(4));
    assert_eq!(world.get::<Health>(e).map(|h| h.max), Some(90));
    assert!(world.get::<LightSource>(e).is_some(), "grave light");
    assert!(world.get::<Hearing>(e).is_some(), "keen-eared");
    assert_eq!(world.get::<Resists>(e).map(|r| r.get(registries.damage_kinds.expect("poison"))), Some(100), "deathless");
    let grasp = library.abilities.expect("drowning grasp");
    assert_eq!(world.get::<Grants>(e).map(|g| g.0.clone()), Some(vec![grasp]), "drowning grasp");
    let regenerating = registries.statuses.expect("regenerating");
    let held = world.get::<Afflicted>(e).and_then(|a| a.0.iter().find(|s| s.id == regenerating).copied()).expect("he regenerates");
    assert_eq!(held.turns, FOR_LIFE, "for life");
    assert!(world.get::<Intelligence>(e).is_some_and(|w| w.0 == Wits::SAPIENT), "sapient");
    assert_ne!(decides(&world, e, 1, king.hp, &library), Some("flee_when_hurt"), "deathless: he never runs");
}

/// Every monster is surer of an adventurer standing in light, so the
/// lantern is seen coming, except one whose trait says how it
/// notices: a keen nose notices by smell, in the dark as in the light.
#[test]
fn every_monster_notices_a_lit_adventurer_sooner_unless_a_trait_says_how_it_notices() {
    let library = library();
    for (_, def) in library.bestiary.defs.iter() {
        let (world, e) = spawn(&library, def);
        let notice = world.get::<Notice>(e).expect("every monster notices").0;
        match def.folded.notice {
            Some(own) => assert_eq!(notice, own, "{}: its trait's", def.name),
            None => assert!(notice.lit_bonus > 0, "{}: sees a lantern coming", def.name),
        }
    }
    let ghoul = library.bestiary.defs.get(library.bestiary.defs.expect("ghoul"));
    assert_eq!(ghoul.folded.notice.map(|n| n.lit_bonus), Some(0), "keen-nosed");
}

/// A kind with the wits to use things has a pack and a slot for
/// everything the registries hold, so what it finds it can carry and
/// wear; a beast has neither.
#[test]
fn a_kind_that_uses_things_has_hands_and_a_beast_has_none() {
    let library = library();
    for (name, hands) in [("goblin", true), ("kobold", true), ("the Drowned King", true), ("rat", false), ("skeleton", false)] {
        let def = library.bestiary.defs.get(library.bestiary.defs.expect(name));
        let (world, e) = spawn(&library, def);
        assert_eq!(world.get::<Inventory>(e).is_some(), hands, "{name}: a pack");
        let slots = world.get::<Equipped>(e).map(|w| w.slot_count());
        assert_eq!(slots, hands.then_some(library.registries.slots.len()), "{name}: slots");
    }
}

/// An ogre's own two fists hit harder than any sword, so an ogre with
/// a sword lying beside it leaves it there, and the king, whose blow is
/// the hardest under the lake, would leave any weapon at all; a goblin's
/// own blow is worth less than the sword it would take up.
#[test]
fn an_ogre_leaves_a_sword_lying_that_would_hit_softer_than_its_fists() {
    use crate::testing::{adventurer, arrive, at, clear_floor, clear_monsters, lay, lay_floor, make, monster, pass_turns, sturdy};
    let library = library();
    let worth = |name: &str| {
        let (world, e) = spawn(&library, library.bestiary.defs.get(library.bestiary.defs.expect(name)));
        world.get::<OwnBlowScore>(e).map(|s| s.0)
    };
    let armory = &library.armory;
    let (world, made) = {
        let mut world = World::new();
        let mut queue = CommandQueue::default();
        let made = crate::armory::spawn_item(&mut Commands::new(&mut queue, &world), armory, armory.defs.expect("war hammer"));
        queue.apply(&mut world);
        (world, made)
    };
    let hammer = world.get::<GearScore>(made).expect("a war hammer is worth something").0;
    assert!(worth("the Drowned King").is_some_and(|king| king > hammer), "the king's blow is worth more than the heaviest weapon");
    assert!(worth("goblin").is_some_and(|goblin| goblin < hammer), "a goblin's is worth less");

    for s in 0..3 {
        let mut app = crate::testing::headless(RunSeed(s));
        let me = adventurer(&mut app);
        sturdy(&mut app, me);
        arrive(&mut app, crate::dungeon::map_of(1));
        clear_monsters(&mut app);
        clear_floor(&mut app);
        let from = at(&app, me);
        let width = app.world().resource::<WorldMap>().place(crate::dungeon::map_of(1)).unwrap().terrain.bounds().width;
        let step = if from.x < width / 2 { 1 } else { -1 };
        let lane: Vec<Point> = (1..=10).map(|d| from.offset(step * d, 0)).collect();
        lay_floor(&mut app, &lane);
        let ogre = monster(&mut app, "ogre", lane[6]);
        let sword = make(&mut app, "sword");
        lay(&mut app, sword, lane[8]);
        pass_turns(&mut app, 8);
        assert_eq!(app.world().get::<Position>(sword).map(|p| p.0), Some(lane[8]), "seed {s}: the sword lies where it lay");
        assert!(app.world().get::<Inventory>(ogre).is_some_and(|bag| bag.items.is_empty()), "seed {s}: and the ogre carries nothing");
    }
}

/// A goblin with a dagger in hand and a shield on its arm sees a sword
/// lying a few steps off and a second dagger nearer: it walks to the
/// sword, puts it on and keeps the dagger in its pack, and leaves the
/// dagger that would only replace its own where it lies. The shield is
/// what makes the second dagger worth nothing: a dagger fits either
/// hand, and a goblin with a hand free takes up any blade for it.
#[test]
fn a_goblin_picks_up_and_wears_a_better_weapon_it_sees() {
    use crate::testing::{adventurer, arrive, at, clear_floor, clear_monsters, lay, lay_floor, make, monster, pass_turns, sturdy, wear};
    for s in 0..3 {
        let mut app = crate::testing::headless(RunSeed(s));
        let me = adventurer(&mut app);
        sturdy(&mut app, me);
        arrive(&mut app, crate::dungeon::map_of(1));
        clear_monsters(&mut app);
        clear_floor(&mut app);
        // A lane of open floor away from the adventurer, toward the
        // side of the floor with more room, lit by the halls' own dim
        // light whatever stood there.
        let from = at(&app, me);
        let width = app.world().resource::<WorldMap>().place(crate::dungeon::map_of(1)).unwrap().terrain.bounds().width;
        let step = if from.x < width / 2 { 1 } else { -1 };
        let lane: Vec<Point> = (1..=10).map(|d| from.offset(step * d, 0)).collect();
        lay_floor(&mut app, &lane);
        let goblin = monster(&mut app, "goblin", lane[5]);
        let (dagger, shield) = (make(&mut app, "dagger"), make(&mut app, "shield"));
        wear(&mut app, goblin, dagger);
        wear(&mut app, goblin, shield);
        let (nearer, sword) = (make(&mut app, "dagger"), make(&mut app, "sword"));
        lay(&mut app, nearer, lane[7]);
        lay(&mut app, sword, lane[8]);
        pass_turns(&mut app, 8);
        let world = app.world();
        let main = world.resource::<Registries>().slots.expect("main hand");
        assert_eq!(world.get::<Equipped>(goblin).and_then(|w| w.in_slot(main)), Some(sword), "seed {s}: it wields the sword");
        assert!(world.get::<Inventory>(goblin).is_some_and(|bag| bag.contains(dagger)), "seed {s}: and keeps its dagger in its pack");
        assert_eq!(world.get::<Position>(nearer).map(|p| p.0), Some(lane[7]), "seed {s}: the dagger no better than its own lies where it lay");
    }
}

/// A cave spider's `venomous` trait: its bite lands `hit`, which
/// poisons whoever it struck, as its row in the content sheet says.
/// The spider sometimes spits its web instead of biting, so this
/// gives it turns enough for a bite to land at least once.
#[test]
fn a_cave_spiders_bite_poisons_the_adventurer() {
    use crate::testing::{arena, monster, pass_turns};
    let (mut app, me, here) = arena(3);
    monster(&mut app, "cave spider", here.offset(1, 0));
    let poisoned = app.world().resource::<Registries>().statuses.expect("poisoned");
    let mut bitten = false;
    for _ in 0..40 {
        pass_turns(&mut app, 1);
        if app.world().get::<Afflicted>(me).is_some_and(|a| a.0.has(poisoned)) {
            bitten = true;
            break;
        }
    }
    assert!(bitten, "the spider's bite never poisoned the adventurer in forty turns");
}

/// A myconid's `spore-burst`: a cloud of confusion where it died,
/// reaching the adventurer who stood beside it when it fell, read
/// straight off the gas field rather than off a chance-based status,
/// so the test does not itself depend on the chance a breath holds.
#[test]
fn a_myconid_bursts_into_a_confusing_cloud_exactly_where_it_dies() {
    use crate::testing::{arena, kill, monster};
    let (mut app, _, here) = arena(4);
    let at = here.offset(1, 0);
    let myconid = monster(&mut app, "myconid", at);
    kill(&mut app, myconid, None);
    let confusing = app.world().resource::<Registries>().gases.expect("confusing");
    assert!(app.world().resource::<Gases>().at(confusing, at) > 0, "a confusing cloud stands where the myconid died");
}

/// Review Focus 1's game half: a monster the test puts to sleep with
/// Undermere's own `asleep` is unaware of the adventurer standing
/// over it, takes their blow at the unaware bonus, and the harm wakes
/// it, as every sleeper wakes of a blow landed on it. Given health
/// enough to live through the blow, since a dagger's unaware least is
/// a goblin's whole health and a dead goblin wakes to nothing.
#[test]
fn a_sleeping_goblin_takes_the_unaware_blow_and_then_wakes() {
    use crate::testing::{arena, equip, make, monster, settle};
    let (mut app, me, here) = arena(5);
    let goblin = monster(&mut app, "goblin", here.offset(1, 0));
    app.world_mut().entity_mut(goblin).insert(Health::full(100));
    let asleep = app.world().resource::<Registries>().statuses.expect(crate::content::ASLEEP);
    app.world_mut().write_message(Afflict { target: goblin, status: asleep, turns: 20, by: None, held_by: None });
    settle(&mut app);
    assert!(app.world().get::<Afflicted>(goblin).is_some_and(|a| a.0.has(asleep)), "asleep before the blow lands");
    let unaware = app.world_mut().run_system_once(move |watchers: Watchers| watchers.unaware(goblin, me)).expect("the watchers");
    assert!(unaware, "a sleeper has not noticed the adventurer standing over it");
    let dagger = make(&mut app, "dagger");
    equip(&mut app, me, dagger);
    app.world_mut().write_message(Intent::new(me, Attack(goblin)));
    settle(&mut app);
    // The dagger's own least is 2, `1d4+1`; a blow at three times that
    // is past anything an aware goblin could take from it, and the
    // unaware bonus's five times makes its least 10.
    let lost = 100 - app.world().get::<Health>(goblin).expect("it lived through the blow").current;
    assert!(lost >= 3 * 2, "the blow landed at the unaware rate, at least three times the dagger's least: {lost}");
    assert!(app.world().get::<Afflicted>(goblin).is_none_or(|a| !a.0.has(asleep)), "the blow woke it");
}

/// Withering: a wraith's touch saps strength, a stack a hit, and
/// repeated touches stack rather than only refresh one instance's
/// clock, so a fight that runs on leaves whoever it catches weaker
/// than the first touch did.
#[test]
fn a_wraiths_touch_weakens_the_adventurer_more_the_more_it_lands() {
    use crate::testing::{arena, monster, pass_turns};
    let (mut app, me, here) = arena(6);
    monster(&mut app, "wraith", here.offset(1, 0));
    let weakened = app.world().resource::<Registries>().statuses.expect(crate::content::WEAKENED);
    let stacks_of = |app: &App| app.world().get::<Afflicted>(me).map_or(0, |a| a.0.iter().filter(|s| s.id == weakened).count());
    let mut saw_two = false;
    for _ in 0..60 {
        pass_turns(&mut app, 1);
        if stacks_of(&app) >= 2 {
            saw_two = true;
            break;
        }
    }
    assert!(saw_two, "the wraith never touched the adventurer twice in sixty turns");
}

/// Luring: a will-o'-wisp keeps its distance from a foe closer than
/// its trait says, backing away rather than closing to melee the
/// moment the adventurer stands beside it.
#[test]
fn a_will_o_wisp_keeps_its_distance_rather_than_closing_to_melee() {
    let library = library();
    let wisp = library.bestiary.defs.get(library.bestiary.defs.expect("will-o'-wisp"));
    assert_eq!(wisp.folded.keep_away, Some(KeepAway { within: 5, no_closer: 3 }), "its trait's span");
    let (world, e) = spawn(&library, wisp);
    let brain = world.get::<Mind>(e).expect("a mind").0.clone();
    let me = ActorView { id: e, pos: Point::ZERO, health: Some(Vitals { current: wisp.hp, max: wisp.hp }), faction: world.get::<Faction>(e).map(|f| f.0) };
    let foe = ActorView {
        id: Entity::PLACEHOLDER,
        pos: Point::new(1, 0),
        health: Some(Vitals { current: 10, max: 10 }),
        faction: Some(library.registries.factions.expect("adventurer")),
    };
    let mut snapshot = Snapshot::alone(me);
    snapshot.enemies.push(foe);
    snapshot.wits = world.get::<Intelligence>(e).expect("wits").0;
    let mut rng = rand::rngs::StdRng::seed_from_u64(1);
    let ctx = &mut TacticCtx {
        snapshot: &snapshot,
        fields: &mut NoFields,
        can_step: &|_| true,
        blocks_shot: &|_| false,
        blocks_burst: &|_| false,
        bounds: Rect::new(-20, -20, 40, 40),
        rng: &mut rng,
    };
    assert_eq!(brain.decide(ctx).1, Some("shadow"), "a foe one cell off, inside its no-closer-than, is backed away from rather than struck");
}

/// Sleep has both its halves in the shipped bestiary: the ogre, the
/// troll and the vampire bat sleep deeply, twice as often as a plain
/// kind, and the golem, Grubnak and the Ember Wyrm never sleep.
#[test]
fn the_shipped_bestiary_has_deep_sleepers_and_kinds_that_never_sleep() {
    let library = library();
    let tricks = |name: &str| library.bestiary.defs.get(library.bestiary.defs.expect(name)).tricks;
    for name in ["ogre", "troll", "vampire bat"] {
        assert!(tricks(name).sleepy && !tricks(name).wakeful, "{name} sleeps deeply");
    }
    for name in ["golem", "Grubnak the goblin king", "the Ember Wyrm"] {
        assert!(tricks(name).wakeful && !tricks(name).sleepy, "{name} never sleeps");
    }
}

/// A conjured blade struck down leaves no body, where a goblin struck
/// down beside it does: nothing of a blade was ever there to raise.
#[test]
fn a_conjured_blade_leaves_no_body_where_a_goblin_does() {
    use crate::testing::{arena, kill, monster, settle};
    let (mut app, _, here) = arena(21);
    let blade = monster(&mut app, "spectral blade", here.offset(1, 0));
    let goblin = monster(&mut app, "goblin", here.offset(-1, 0));
    kill(&mut app, blade, None);
    kill(&mut app, goblin, None);
    settle(&mut app);
    assert!(app.world().get_entity(blade).is_err(), "the blade is gone, with nothing left of it");
    assert!(app.world().get::<Remains>(goblin).is_some(), "the goblin lies where it fell");
}

/// Every named mini-boss struck down lies as the corpse of its name, `the
/// corpse of the Widow`, where a kind lies as its kind's, `goblin
/// corpse`; a boss leaves no body, since its death ends the run, but its
/// name is said the same way should one ever be left.
#[test]
fn every_named_mini_boss_lies_as_the_corpse_of_its_name_and_a_kind_as_its_kinds() {
    use crate::testing::{arena, kill, monster, name_of, settle};
    let (mut app, _, here) = arena(21);
    let defs = app.world().resource::<Bestiary>().defs.clone();
    let minibosses: Vec<&str> = defs.iter().filter(|(_, d)| d.miniboss).map(|(_, d)| d.name.as_str()).collect();
    assert!(minibosses.len() >= 5, "the five mini-bosses: {minibosses:?}");
    let naming = app.world().resource::<RemainsNaming>().clone();
    for (_, boss) in defs.iter().filter(|(_, d)| d.boss) {
        assert_eq!(naming.name_for(&boss.name), format!("the corpse of {}", boss.name));
    }
    for (step, name) in minibosses.iter().enumerate() {
        let body = monster(&mut app, name, here.offset(1, step as i32 - 2));
        kill(&mut app, body, None);
        settle(&mut app);
        assert_eq!(name_of(&app, body), format!("the corpse of {name}"));
    }
    let goblin = monster(&mut app, "goblin", here.offset(-1, 0));
    kill(&mut app, goblin, None);
    assert_eq!(name_of(&app, goblin), "goblin corpse", "a kind reads as before");
}

/// A burn is no blow: a pink jelly left burning loses health to every
/// tick of it and is still one jelly when the fire is done, where it
/// once split on every turn it burned.
#[test]
fn a_burning_jelly_does_not_multiply() {
    use crate::testing::{arena, monster, pass_turns, settle};
    let (mut app, _, here) = arena(19);
    let jelly = monster(&mut app, "pink jelly", here.offset(3, 0));
    app.world_mut().entity_mut(jelly).remove::<Mind>();
    let scorched = app.world().resource::<Registries>().statuses.expect("scorched");
    app.world_mut().write_message(Afflict { target: jelly, status: scorched, turns: 5, by: None, held_by: None });
    settle(&mut app);
    pass_turns(&mut app, 6);
    let jelly_id = app.world().resource::<Bestiary>().defs.expect("pink jelly");
    let world = app.world_mut();
    let jellies: Vec<i32> = world.query::<(&Kind, &Health)>().iter(world).filter(|(k, _)| k.0 == jelly_id).map(|(_, h)| h.current).collect();
    assert_eq!(jellies.len(), 1, "burned, and still the one jelly: {jellies:?}");
    assert!(jellies[0] < 30, "the burn did reach it: {jellies:?}");
}

/// A jelly with less than four left splits into nothing: its halves
/// floored at two each would come to more than it had.
#[test]
fn a_jelly_with_less_than_four_health_left_does_not_split() {
    use crate::testing::{arena, monster, settle};
    let (mut app, _, here) = arena(20);
    let jelly = monster(&mut app, "pink jelly", here.offset(1, 0));
    app.world_mut().entity_mut(jelly).insert(Health { current: 7, max: 30 });
    let crush = app.world().resource::<Registries>().damage_kinds.expect("crush");
    app.world_mut().write_message(DamageEvent::new(jelly, rl_engine::rl_rules::Hit::from_source(None, crush, 4)));
    settle(&mut app);
    let jelly_id = app.world().resource::<Bestiary>().defs.expect("pink jelly");
    let world = app.world_mut();
    let jellies: Vec<i32> = world.query::<(&Kind, &Health)>().iter(world).filter(|(k, _)| k.0 == jelly_id).map(|(_, h)| h.current).collect();
    assert_eq!(jellies, [3], "seven taken four leaves three, too little to split");
}

/// Splitting: a blow that leaves a pink jelly alive splits it into
/// two, each with half what was left, when four or more was left.
#[test]
fn a_jelly_struck_becomes_two_each_with_its_share() {
    use crate::testing::{arena, monster, settle};
    let (mut app, _, here) = arena(18);
    let jelly = monster(&mut app, "pink jelly", here.offset(1, 0));
    app.world_mut().entity_mut(jelly).insert(Health { current: 10, max: 30 });
    let crush = app.world().resource::<Registries>().damage_kinds.expect("crush");
    app.world_mut().write_message(DamageEvent::new(jelly, rl_engine::rl_rules::Hit::from_source(None, crush, 4)));
    settle(&mut app);
    let jelly_id = app.world().resource::<Bestiary>().defs.expect("pink jelly");
    let world = app.world_mut();
    let halves: Vec<i32> = world.query::<(&Kind, &Health)>().iter(world).filter(|(k, _)| k.0 == jelly_id).map(|(_, h)| h.current).collect();
    assert_eq!(halves.len(), 2, "the jelly and its half");
    assert!(halves.iter().all(|h| *h == 3), "ten taken four leaves six, split evenly: {halves:?}");
}

/// Stealing: a landed blow takes a random carried thing, not worn,
/// and marks the thief to flee with it; killed, it drops what it
/// took where it fell.
#[test]
fn a_monkey_takes_a_thing_and_runs_and_drops_it_when_killed() {
    use crate::testing::{arena, give, kill, make, monster, pass_turns};
    let (mut app, me, here) = arena(21);
    let dagger = make(&mut app, "dagger");
    give(&mut app, me, dagger);
    let monkey = monster(&mut app, "monkey", here.offset(1, 0));
    let mut stole = false;
    for _ in 0..40 {
        pass_turns(&mut app, 1);
        if app.world().get::<Inventory>(monkey).is_some_and(|bag| bag.items.contains(&dagger)) {
            stole = true;
            break;
        }
    }
    assert!(stole, "the monkey never stole the dagger in forty turns");
    assert!(app.world().get::<crate::tricks::Stole>(monkey).is_some(), "marked to flee with it");
    kill(&mut app, monkey, Some(me));
    assert_eq!(app.world().get::<Position>(dagger).map(|p| p.0), app.world().get::<Position>(monkey).map(|p| p.0), "the dagger lies where the monkey fell");
}

/// Raising: a necromancer raises a body in its sight and reach as its
/// own ally, which then fights for it rather than its raiser's kind.
#[test]
fn a_necromancer_raises_a_body_in_its_sight_and_it_fights_for_it() {
    use crate::testing::{arena, kill, monster, settle};
    let (mut app, me, here) = arena(25);
    let rat = monster(&mut app, "rat", here.offset(2, 0));
    kill(&mut app, rat, Some(me));
    assert!(app.world().get::<Actor>(rat).is_none(), "the rat is a body now, not an actor");
    let necromancer = monster(&mut app, "necromancer", here.offset(1, 0));
    // A pass so the necromancer's own sight is cast at least once:
    // `raise_dead` is called directly here, outside the turn its
    // `Viewshed` is normally cast within.
    settle(&mut app);
    let raised = crate::effects::bodies::raise_dead(app.world_mut(), necromancer);
    assert!(raised, "the necromancer found the rat's body to raise");
    assert!(app.world().get::<Actor>(rat).is_some(), "it stands again");
    assert_eq!(
        app.world().get::<Faction>(rat).map(|f| f.0),
        app.world().get::<Faction>(necromancer).map(|f| f.0),
        "fighting for its raiser rather than its own kind"
    );
    assert!(app.world().get::<Mind>(rat).is_some(), "a mind to fight with again");
}

/// Raising, played: a necromancer left to its own mind, with a foe in
/// sight and a body behind it, reaches for raise dead within forty
/// turns over a span of seeds, though blink is declared first and is
/// worth more on a turn both are ready, so it waits on blink's
/// cooldown and the mind's own two in five of reaching at all.
#[test]
fn a_necromancer_left_to_its_own_mind_raises_the_body_behind_it_over_a_span_of_seeds() {
    use crate::testing::{arena, kill, monster, pass_turns};
    for s in 0..6 {
        let (mut app, me, here) = arena(40 + s);
        let rat = monster(&mut app, "rat", here.offset(REACH_OF_RAISING, 0));
        kill(&mut app, rat, Some(me));
        monster(&mut app, "necromancer", here.offset(REACH_OF_RAISING - 1, 0));
        let mut raised = false;
        app.world_mut().entity_mut(me).insert(Health::full(1000));
        for _ in 0..40 {
            pass_turns(&mut app, 1);
            if app.world().get::<Actor>(rat).is_some() {
                raised = true;
                break;
            }
        }
        assert!(raised, "seed {s}: the necromancer never raised the rat in forty turns: {:#?}", crate::testing::said(&app));
    }
}

/// And the other half: a necromancer with no body in reach never spends
/// a turn on raise dead, which it once cast whenever it saw a foe and
/// raised nothing with, over the same span of seeds and turns.
#[test]
fn a_necromancer_with_no_body_in_reach_never_reaches_for_raise_dead() {
    use crate::testing::{arena, monster, pass_turns, said};
    for s in 0..6 {
        let (mut app, me, here) = arena(40 + s);
        monster(&mut app, "necromancer", here.offset(REACH_OF_RAISING - 1, 0));
        app.world_mut().entity_mut(me).insert(Health::full(1000));
        pass_turns(&mut app, 40);
        let lines = said(&app);
        assert!(!lines.iter().any(|l| l.contains("raise dead")), "seed {s}: raised nothing, and never tried: {lines:#?}");
    }
}

/// How far from the adventurer the body a necromancer is left to raise
/// lies: inside the arena's open ground, a step behind the necromancer.
const REACH_OF_RAISING: i32 = 4;

/// A bolt zapped at a golem and turned back reads in the log as it
/// plays: the golem turning it back, said before the burn it gave the
/// one who zapped it, and nothing saying the golem was burned by it.
#[test]
fn a_bolt_a_golem_turns_back_is_said_turned_back_before_it_burns_the_one_who_zapped_it() {
    use crate::testing::{arena, find, give, said, settle, still};
    let (mut app, me, here) = arena(33);
    still(&mut app, "golem", here.offset(3, 0));
    let mut turned = None;
    for _ in 0..16 {
        let staff = find(&mut app, "staff of firebolt", 2);
        give(&mut app, me, staff);
        settle(&mut app);
        let before = said(&app).len();
        app.world_mut().write_message(Intent::new(me, UseItemAt { item: staff, at: here.offset(3, 0) }));
        settle(&mut app);
        let lines = said(&app)[before..].to_vec();
        if lines.iter().any(|l| l.contains("turns it back")) {
            turned = Some(lines);
            break;
        }
    }
    let lines = turned.expect("one zap in sixteen was turned back");
    let back = lines.iter().position(|l| l == "The golem turns it back on you.").unwrap_or_else(|| panic!("said as turned back: {lines:#?}"));
    let burn = lines.iter().position(|l| l.starts_with("You hurt yourself for")).unwrap_or_else(|| panic!("the burn it gave you is said: {lines:#?}"));
    assert!(back < burn, "turned back, then the burn: {lines:#?}");
    assert!(!lines.iter().any(|l| l.contains("the golem for")), "nothing says the bolt hurt the golem: {lines:#?}");
}

/// Grubnak's `war cry`: every ally in sight hastened for the trait's
/// own four turns, Grubnak himself included, a foe beside them left as
/// it was.
#[test]
fn grubnaks_war_cry_hastens_every_ally_in_sight_itself_included() {
    use crate::testing::{arena, monster, settle};
    let (mut app, me, here) = arena(29);
    let grubnak = monster(&mut app, "Grubnak the goblin king", here.offset(1, 0));
    let guard = monster(&mut app, "goblin", here.offset(2, 0));
    settle(&mut app);
    crate::effects::bodies::rally(app.world_mut(), grubnak, 4);
    settle(&mut app);
    let hasted = app.world().resource::<Registries>().statuses.expect("hasted");
    let has = |e: Entity| app.world().get::<Afflicted>(e).is_some_and(|a| a.iter().any(|s| s.id == hasted));
    assert!(has(grubnak), "Grubnak rallies himself too");
    assert!(has(guard), "and his own guard beside him");
    assert!(!has(me), "never the adventurer, who is no ally of his");
}

/// Draining: a landed blow mends its own attacker what it dealt.
#[test]
fn a_draining_kinds_landed_blow_mends_it_what_it_dealt() {
    use crate::testing::{arena, monster, pass_turns};
    let (mut app, _, here) = arena(27);
    let bat = monster(&mut app, "vampire bat", here.offset(1, 0));
    app.world_mut().entity_mut(bat).insert(Health { current: 1, max: 9 });
    let mut healed = false;
    for _ in 0..40 {
        pass_turns(&mut app, 1);
        if app.world().get::<Health>(bat).is_some_and(|h| h.current > 1) {
            healed = true;
            break;
        }
    }
    assert!(healed, "the bat never mended itself off a landed blow in forty turns");
}

/// Fire-trail: a salamander's own step ignites the cell it left.
#[test]
fn a_fire_trail_kind_ignites_the_cell_it_steps_off() {
    use crate::testing::{arena, monster, settle};
    let (mut app, _, here) = arena(29);
    let salamander = monster(&mut app, "salamander", here.offset(1, 0));
    let (from, to) = (here.offset(1, 0), here.offset(1, 1));
    app.world_mut().write_message(Stepped { actor: salamander, from, to });
    settle(&mut app);
    assert!(app.world().resource::<Fire>().is_burning(from), "the cell it stepped off is alight");
}

/// Reflecting: a golem carries the engine's own `Reflects` the moment
/// it is spawned, so an aimed use or an ability that lands on it
/// turns back on whoever aimed it, one time in two, exactly as
/// `rl-bevy`'s own test of `Reflects` says.
#[test]
fn a_golem_carries_the_engines_own_reflects() {
    let library = library();
    let golem = library.bestiary.defs.get(library.bestiary.defs.expect("golem"));
    let (world, e) = spawn(&library, golem);
    assert!(world.get::<Reflects>(e).is_some(), "turns magic back");
}

/// Packing: a rat too far from its own kind, with nothing else in
/// sight, closes the gap rather than waiting or wandering off alone.
#[test]
fn a_packing_kind_closes_on_its_own_kind_when_nothing_else_is_in_sight() {
    let library = library();
    let rat = library.bestiary.defs.get(library.bestiary.defs.expect("rat"));
    let (world, e) = spawn(&library, rat);
    let brain = world.get::<Mind>(e).expect("a mind").0.clone();
    let me = ActorView { id: e, pos: Point::ZERO, health: Some(Vitals { current: rat.hp, max: rat.hp }), faction: world.get::<Faction>(e).map(|f| f.0) };
    let ally = ActorView { id: Entity::PLACEHOLDER, pos: Point::new(5, 0), health: Some(Vitals { current: rat.hp, max: rat.hp }), faction: me.faction };
    let mut snapshot = Snapshot::alone(me);
    snapshot.allies.push(ally);
    snapshot.wits = world.get::<Intelligence>(e).expect("wits").0;
    let mut rng = rand::rngs::StdRng::seed_from_u64(1);
    let ctx = &mut TacticCtx {
        snapshot: &snapshot,
        fields: &mut NoFields,
        can_step: &|_| true,
        blocks_shot: &|_| false,
        blocks_burst: &|_| false,
        bounds: Rect::new(-20, -20, 40, 40),
        rng: &mut rng,
    };
    assert_eq!(brain.decide(ctx).1, Some("follow"), "closes the gap on its own kind");
}

/// Led: a dar priestess with its leader sensed closes the gap on it
/// rather than waiting or wandering off alone.
#[test]
fn a_led_kind_closes_on_its_leader_when_nothing_else_is_in_sight() {
    let library = library();
    let priestess = library.bestiary.defs.get(library.bestiary.defs.expect("dar priestess"));
    let (world, e) = spawn(&library, priestess);
    let brain = world.get::<Mind>(e).expect("a mind").0.clone();
    let me =
        ActorView { id: e, pos: Point::ZERO, health: Some(Vitals { current: priestess.hp, max: priestess.hp }), faction: world.get::<Faction>(e).map(|f| f.0) };
    let mut snapshot = Snapshot::alone(me);
    snapshot.wits = world.get::<Intelligence>(e).expect("wits").0;
    snapshot.add_sense(rl_engine::rl_rules::ai::tactics::Led { at: Point::new(5, 0), in_sight: true });
    let mut rng = rand::rngs::StdRng::seed_from_u64(1);
    let ctx = &mut TacticCtx {
        snapshot: &snapshot,
        fields: &mut NoFields,
        can_step: &|_| true,
        blocks_shot: &|_| false,
        blocks_burst: &|_| false,
        bounds: Rect::new(-20, -20, 40, 40),
        rng: &mut rng,
    };
    assert_eq!(brain.decide(ctx).1, Some("follow"), "closes the gap on its leader");
}

/// Review Focus 5: a band whose leader has died keeps together
/// rather than freezing. The engine's own `Keep::leader` falls back
/// to keeping station on allies, exactly as `Keep::allies` would, the
/// moment nothing senses a `Led` to follow, which is what a dead
/// leader looks like to a follower's snapshot: a dar priestess with
/// an ally in sight and no leader sensed still closes the gap on its
/// ally, a real decision, rather than deciding nothing.
#[test]
fn a_dar_priestess_whose_leader_is_gone_keeps_station_on_its_allies_rather_than_freezing() {
    let library = library();
    let priestess = library.bestiary.defs.get(library.bestiary.defs.expect("dar priestess"));
    let (world, e) = spawn(&library, priestess);
    let brain = world.get::<Mind>(e).expect("a mind").0.clone();
    let me =
        ActorView { id: e, pos: Point::ZERO, health: Some(Vitals { current: priestess.hp, max: priestess.hp }), faction: world.get::<Faction>(e).map(|f| f.0) };
    let ally =
        ActorView { id: Entity::PLACEHOLDER, pos: Point::new(5, 0), health: Some(Vitals { current: priestess.hp, max: priestess.hp }), faction: me.faction };
    let mut snapshot = Snapshot::alone(me);
    snapshot.allies.push(ally);
    snapshot.wits = world.get::<Intelligence>(e).expect("wits").0;
    let mut rng = rand::rngs::StdRng::seed_from_u64(1);
    let ctx = &mut TacticCtx {
        snapshot: &snapshot,
        fields: &mut NoFields,
        can_step: &|_| true,
        blocks_shot: &|_| false,
        blocks_burst: &|_| false,
        bounds: Rect::new(-20, -20, 40, 40),
        rng: &mut rng,
    };
    let decision = brain.decide(ctx);
    assert_eq!(decision.1, Some("follow"), "keeps station on its own ally instead of freezing, a real decision and not a wait");
}

/// Review Focus 5, the brief's own case: Grubnak's band. A plain
/// goblin, which his own honor guard may draw, is `led` for exactly
/// this, a line of its own in `monsters.ron`'s row; the same engine
/// mechanism the dar's own test above already proves, read through
/// the one kind it was added to for a mini-boss rather than one that already
/// carried it.
#[test]
fn a_goblin_of_grubnaks_band_whose_leader_is_gone_keeps_station_on_its_allies_rather_than_freezing() {
    let library = library();
    let goblin = library.bestiary.defs.get(library.bestiary.defs.expect("goblin"));
    let (world, e) = spawn(&library, goblin);
    let brain = world.get::<Mind>(e).expect("a mind").0.clone();
    let me = ActorView { id: e, pos: Point::ZERO, health: Some(Vitals { current: goblin.hp, max: goblin.hp }), faction: world.get::<Faction>(e).map(|f| f.0) };
    let ally = ActorView { id: Entity::PLACEHOLDER, pos: Point::new(5, 0), health: Some(Vitals { current: goblin.hp, max: goblin.hp }), faction: me.faction };
    let mut snapshot = Snapshot::alone(me);
    snapshot.allies.push(ally);
    snapshot.wits = world.get::<Intelligence>(e).expect("wits").0;
    let mut rng = rand::rngs::StdRng::seed_from_u64(1);
    let ctx = &mut TacticCtx {
        snapshot: &snapshot,
        fields: &mut NoFields,
        can_step: &|_| true,
        blocks_shot: &|_| false,
        blocks_burst: &|_| false,
        bounds: Rect::new(-20, -20, 40, 40),
        rng: &mut rng,
    };
    let decision = brain.decide(ctx);
    assert_eq!(decision.1, Some("follow"), "a goblin of Grubnak's own band keeps station on its ally instead of freezing once its king is gone");
}

/// The lived-in dungeon's small life is worth nothing to kill: a flat
/// nought attack, whatever its dice would otherwise say, makes
/// `balance::threat` nought exactly, hostile health and armor besides.
#[test]
fn small_life_is_worth_nothing_to_kill() {
    let library = library();
    for name in ["moth", "blind cave rat"] {
        let def = library.bestiary.defs.get(library.bestiary.defs.expect(name));
        assert_eq!(rl_engine::rl_rules::balance::threat(def), 0.0, "{name}'s own threat");
    }
}

/// Withering stops at three: a fourth and a fifth touch before the first
/// runs out are refused by `weakened`'s own cap, whoever lands them.
#[test]
fn weakened_stops_stacking_at_three_however_many_touches_land() {
    use crate::testing::{arena, settle};
    let (mut app, me, _) = arena(6);
    let weakened = app.world().resource::<Registries>().statuses.expect(crate::content::WEAKENED);
    for _ in 0..5 {
        app.world_mut().write_message(Afflict { target: me, status: weakened, turns: 20, by: None, held_by: None });
        settle(&mut app);
    }
    let stacks = app.world().get::<Afflicted>(me).map_or(0, |a| a.0.iter().filter(|s| s.id == weakened).count());
    assert_eq!(stacks, crate::content::WEAKENED_MAX_STACKS as usize);
}

/// Every call of the drowned among the shipped abilities raises a kind
/// the bestiary has, and a call naming one it lacks is refused naming the
/// ability and the kind, rather than loading as a call that raises
/// nothing.
#[test]
fn every_call_of_the_drowned_raises_a_kind_of_the_bestiary_and_a_misspelt_kind_is_refused() {
    use crate::effects::bodies::check_calls;
    let library = library();
    assert_eq!(check_calls(library.abilities.defs(), &library.bestiary.defs), Ok(()));
    let text = r#"[(
        name: "call the drowned", description: "Calls them.", aim: SelfOnly, mode: Own, cooldown: 2000,
        effects: [(kind: "CallDrowned", args: (count: 1, range: 6, monster: "drowned pilgrm"))],
    )]"#;
    let misspelt = rl_engine::rl_bevy::Abilities::load(text, &crate::content::effect_kinds(), &library.registries.names()).expect("it reads as an ability");
    let refused = check_calls(misspelt.defs(), &library.bestiary.defs).expect_err("a kind the bestiary lacks");
    assert!(refused.contains("call the drowned") && refused.contains("drowned pilgrm"), "names the ability and the kind: {refused}");
}

/// The Drowned Abbess's call stands up drowned pilgrims, her own
/// congregation, out of the water round her: played, not read off her
/// file, so a call that raised nothing would fail here.
#[test]
fn the_drowned_abbess_calls_drowned_pilgrims_up_out_of_the_water() {
    use crate::testing::{arena_on, monster, pass_turns};
    let (mut app, _, here) = arena_on(1, 3);
    let abbess_at = here.offset(3, 0);
    let flooded = {
        let names = &app.world().resource::<WorldMap>().tables().names;
        TileId(names.iter().position(|n| n == "flooded floor").expect("flooded floor is a tile") as u16)
    };
    let water: Vec<Point> = geometry::square(abbess_at, 2).filter(|p| *p != abbess_at && *p != here).collect();
    rl_engine::testing::lay_tile(&mut app, &water, flooded);
    monster(&mut app, "the Drowned Abbess", abbess_at);
    let pilgrim = app.world().resource::<Bestiary>().defs.expect("drowned pilgrim");
    let risen = |app: &mut App| {
        let world = app.world_mut();
        world.query::<(&Kind, &Position)>().iter(world).filter(|(k, _)| k.0 == pilgrim).map(|(_, p)| p.0).collect::<Vec<Point>>()
    };
    assert!(risen(&mut app).is_empty(), "no pilgrim before she calls");
    for _ in 0..60 {
        pass_turns(&mut app, 1);
        if !risen(&mut app).is_empty() {
            break;
        }
    }
    let up = risen(&mut app);
    assert!(!up.is_empty(), "the Abbess called no pilgrim up in sixty turns");
    assert!(up.iter().all(|p| water.contains(p)), "each rose out of the water: {up:?}");
}
