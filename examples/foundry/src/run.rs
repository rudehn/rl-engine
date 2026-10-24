//! The run's start: the foundry's decks, its combat rules, and the
//! commando dropped onto deck one.
//!
//! In the library rather than `main.rs`, since `main.rs` is only the
//! binary that opens the window; a later integration test reaches this
//! the same way `testing::headless` does, through the crate the `tests/`
//! directory links against.

use atglyph::prelude::all::*;
use atglyph::rules::Percent;
use atglyph::rules::ai::hearing::HearingStats;
use atglyph::rules::damage::{ApplyResistance, SubtractArmor};
use bevy::prelude::*;

use crate::content::{Profile, resistances};
use crate::decks::{Foundry, map_of};
use crate::droids::Roster;

/// The deck a run starts on, when it is not deck one: `main.rs` reads it
/// from `FOUNDRY_START` for a screenshot of a deeper deck. Absent, the run
/// starts on deck one, which is every real run.
#[derive(Resource, Debug, Clone, Copy)]
pub struct StartDeck(pub u32);

/// The player, as a kind the save can name.
///
/// A marker rather than the engine's `Player`, because a save names each
/// kind by its own component and restores it through that component's own
/// account of it; the engine's marker is every game's.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Commando;

/// Everything a run stands on that is the same however it began: the
/// decks, the combat rules and the damage pipeline, the dark, the fire,
/// the roster, the pieces, the loot stream. In [`PrepareRun`], which the
/// engine runs before [`start`] on a fresh run and before the restore on a
/// continued one, on the seed the save holds, so a continued run is built
/// on exactly the foundry a fresh one is.
pub fn prepare(mut commands: Commands, seed: Res<Seed>, registries: Res<Registries>) {
    let foundry = Foundry::new(seed.0);
    commands.insert_resource(foundry.appearance());
    commands.insert_resource(WorldMap::new(foundry.tiles().tables()));

    let (commando, droids, vermin) = (registries.factions.expect("commando"), registries.factions.expect("droids"), registries.factions.expect("vermin"));
    let combat = CombatRules::new().hostile(commando, droids).hostile(commando, vermin).hostile(droids, vermin);
    commands.insert_resource(combat);
    // Resistance first, then plate: the damage table in `content.rs` is
    // what makes ion undo a chassis and barely touch flesh, and the
    // engine's default list is armor alone, which would leave it written
    // down and never played.
    commands.insert_resource(DamageStages(vec![Box::new(ApplyResistance), Box::new(SubtractArmor)]));
    // ANCHOR: accuracy
    // A shot goes wide past its weapon's effective range and in poor light:
    // five points a tile past it, sixteen for a target in dim light, thirty
    // for one seen only by the helmet in the dark. Foundry has no stats, so
    // accuracy is the model's hundred and nobody evades.
    commands.insert_resource(HitRules(Box::new(Percent::new(5, 16, 30))));
    // ANCHOR_END: accuracy
    commands.insert_resource(Lighting::dark());
    // ANCHOR: fire_rules
    // Standing in fire scorches, and whatever burns smokes: an incendiary
    // is a fire and a screen at once.
    commands.insert_resource(FireRules::new().inflicts(registries.statuses.expect("scorched"), 3).smoke(registries.gases.expect("smoke"), 30));
    // ANCHOR_END: fire_rules
    commands.insert_resource(Roster::load(&registries));
    // The pieces the decks are stamped from, shared with the engine that
    // fills their slots, so a stamp's key names the same piece in both.
    commands.insert_resource(foundry.prefabs().clone());
    commands.insert_resource(PlaceRulesRes(Box::new(foundry)));
}

/// Spawns the commando with its lamp lit and nothing else, nowhere yet:
/// the warp onto a deck puts it somewhere, and a continued run's save
/// does.
pub fn spawn_commando(commands: &mut Commands, registries: &Registries) -> Entity {
    let commando = registries.factions.expect("commando");
    let kinetic = registries.damage_kinds.expect("kinetic");
    let player = commands
        .spawn((
            (Actor, Player, Commando, Blocks, Position(Point::ZERO), Viewshed::new(20), RevealsMap),
            (Health::full(30), Armor(0), Faction(commando), Resists(resistances(Profile::Organic, registries)), crate::light::SHOULDER_LAMP),
            (MeleeAttack::new(kinetic, DiceRoll::new(1, 3)), Name::new("you"), Glyph::new('@', COMMANDO).on_layer(10)),
            // Noticeable, not sneaky: no skill at hiding, but a subject a
            // droid has to notice rather than one it sees the instant it
            // comes into view. Without it nothing ever notices the
            // commando, so a probe that spots it never sounds its alarm,
            // and the lamp decides only whether a droid can see, never how
            // sure it is.
            Stealth(StealthStats::default()),
            // Ears, so the deck's noise reads on the vitals strip: a
            // commando hears what a droid hears, from a threshold of
            // nothing, and decides for themselves what it means.
            Hearing(HearingStats { threshold: 0, memory: 6 }),
        ))
        .id();
    // Empty hands and an empty bag: what the commando carries is what the
    // deck gave them. The innate `MeleeAttack` above is the whole opening
    // arsenal, which is why nothing on deck one shoots back and a pistol
    // lies somewhere on it most runs.
    commands.entity(player).insert((Inventory::default(), Equipped(Equipment::with_slot_count(registries.slots.len()))));
    player
}

/// How the commando is drawn: white, and faded while nothing can see them,
/// so the player sees the cloak working on the map and not only in a badge.
pub const COMMANDO: Color = Color::WHITE;
/// The commando while unseen.
pub const COMMANDO_UNSEEN: Color = Color::srgb(0.45, 0.5, 0.6);

/// Draws the commando faded while unseen and white otherwise.
///
/// In `TurnSet::React` rather than a drawing layer: the glyph's colour is
/// the run's state, set in the pass the cloak went on or came off, and the
/// map draws whatever it is.
pub fn fade_the_unseen(mut commando: Query<(&mut Glyph, Has<Unseen>), With<Commando>>) {
    for (mut glyph, unseen) in &mut commando {
        let fg = if unseen { COMMANDO_UNSEEN } else { COMMANDO };
        if glyph.fg != fg {
            glyph.fg = fg;
        }
    }
}

/// Spawns the commando into the foundry [`prepare`] built and warps it
/// onto deck one, or onto [`StartDeck`]'s. Runs in [`NewRun`], for every
/// run but one the title asked to continue, which the engine restores
/// from its save instead.
///
/// Asks nothing about a title: while one is up the engine runs no
/// `NewRun` at all, and runs it once the player picks.
///
/// Reads the registries from a resource rather than building them itself:
/// `main.rs` inserts them before the run starts, the way its content is
/// loaded before anything else runs.
pub fn start(mut commands: Commands, seed: Res<Seed>, registries: Res<Registries>, first: Option<Res<StartDeck>>, mut begin: Begin) {
    let player = spawn_commando(&mut commands, &registries);
    let deck = first.map_or(1, |f| f.0.clamp(1, crate::decks::DECKS));
    begin.log.notice(format!("Seed {}. The drop ship is gone. The first reactor is on deck three.", seed.0.0));
    begin.warps.write(WarpRequest::into_place(player, map_of(deck)));
}

/// What the run did to the commando, given again once the engine has
/// restored it, in [`RunResumed`]: its upgrades, given to a commando that
/// has none now that its gear is back on, so the uplink's tile of reach is
/// given to the gun it wears the way it was the first time, rather than
/// saved on the gun and given twice.
pub fn give_upgrades_again(world: &mut World) {
    let Ok(player) = world.query_filtered::<Entity, With<Player>>().single(world) else { return };
    let taken = world.resource::<crate::upgrades::Taken>().clone();
    for upgrade in taken.0.iter().copied() {
        crate::upgrades::apply(upgrade, player, world);
    }
}

/// Offers again a pick that was waiting when the run was saved, in
/// [`RunResumed`]: a pick is not a turn, so the save may have caught the
/// charge and not the choice, and the choice is offered again rather than
/// lost.
pub fn offer_the_waiting_pick(
    quests: Res<Quests>,
    mut modals: ResMut<Modals>,
    mut screen: ResMut<crate::upgrades::ChoiceScreen>,
    mut choosing: ResMut<crate::upgrades::Choosing>,
    taken: Res<crate::upgrades::Taken>,
) {
    let charged = crate::mission::CHARGE_QUESTS.iter().filter(|name| quests.tracker.state(quests.defs.expect(name)) == QuestState::Done).count();
    if charged > taken.0.len() {
        crate::upgrades::offer(&mut modals, &mut screen, &mut choosing, &taken);
    }
}

/// Says which deck a continued run goes on from, in [`RunResumed`], so
/// the line is stamped with the turn the run was saved on.
pub fn say_where_it_goes_on(player: Query<Option<&OnMap>, With<Player>>, mut log: Log) {
    let deck = crate::decks::deck_of(player.single().ok().flatten().map_or(MapId::ZERO, |m| m.0));
    log.notice(format!("Continuing on deck {deck}."));
}

/// What starting a run writes beyond the world itself: the log's first
/// line and the warp onto the first deck; play begins on its own once the
/// start has built the world.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Begin<'w> {
    warps: MessageWriter<'w, WarpRequest>,
    log: Log<'w>,
}

#[cfg(test)]
mod tests {
    use atglyph::ecs::plugin::{Turn, TurnSet};
    use atglyph::primitives::RunSeed;

    use super::*;

    /// A run rolls to hit: five a tile past a gun's effective range, and
    /// sixteen for a droid in dim light, thirty for one in the dark.
    #[test]
    fn a_shot_goes_wide_past_its_effective_range_and_in_poor_light() {
        use atglyph::grid::LightBand;
        use atglyph::rules::accuracy::{Delivery, Shot};
        let mut app = crate::testing::headless(RunSeed(1));
        // The run's rules are laid down as it starts, on the first frames.
        app.update();
        app.update();
        let rules = app.world().resource::<HitRules>();
        let shot = |distance, light| Shot { delivery: Delivery::Shot, distance, effective: 3, range: 8, light, accuracy: None, evasion: None };
        let chance = |distance, light| rules.0.odds(&shot(distance, light)).map(|o| o.percent());
        assert_eq!(chance(3, LightBand::Lit), Some(100), "inside effective range, in light, a sure thing");
        assert_eq!(chance(5, LightBand::Dim), Some(100 - 10 - 16));
        assert_eq!(chance(8, LightBand::Dark), Some(100 - 25 - 30));
    }

    /// Every actor ever dealt a turn, in the order it was dealt, whether
    /// player or not: a resource a test-only system fills from inside the
    /// `Turn` schedule itself, in `TurnSet::Decide`, right after a pass's
    /// own `TurnSet::Schedule` deals it and well before that same pass's
    /// `TurnSet::Cleanup` could take `MyTurn` away again. A reader placed
    /// in the outer `Update` schedule instead would miss every actor dealt
    /// and resolved within the same `app.update()` the runner in
    /// `crates/atglyph-bevy/src/plugin.rs` can spend several `Turn` passes on.
    #[derive(Resource, Default)]
    struct Dealt(Vec<bool>);

    fn record_deals(mut dealt: ResMut<Dealt>, fresh: Query<Has<Player>, Added<MyTurn>>) {
        dealt.0.extend(fresh.iter());
    }

    #[test]
    fn a_new_run_leaves_the_commando_empty_handed_but_able_to_carry_and_to_punch_over_a_span_of_seeds() {
        for s in 0..4u64 {
            let mut app = crate::testing::headless(RunSeed(s));
            crate::testing::settle(&mut app);
            let me = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
            let worn: Vec<Entity> = app.world().get::<Equipped>(me).unwrap().0.worn().map(|(_, item)| item).collect();
            assert!(worn.is_empty(), "seed {s}: nothing in hand and nothing worn");
            assert!(app.world().get::<Inventory>(me).unwrap().items.is_empty(), "seed {s}: and an empty bag");
            // The bag and the slots are there all the same, or the first
            // thing the commando steps on could not be picked up or worn.
            assert!(app.world().get::<MeleeAttack>(me).is_some(), "seed {s}: bare hands are still an attack");
        }
    }

    /// A run begun after another in the same launch, by the menu's A new
    /// run or by Main menu and New Game, keeps nothing of the run before
    /// it: no upgrade taken, and no deck reached but the first.
    #[test]
    fn a_run_begun_after_another_keeps_neither_its_upgrades_nor_its_depth() {
        use atglyph::testing::{KeyScriptPlugin, press};
        let mut app = crate::testing::headless(RunSeed(2));
        app.add_plugins(KeyScriptPlugin);
        let me = crate::testing::beside_the_console(&mut app);
        crate::testing::settle(&mut app);
        let into_it = crate::testing::key_toward_the_console(&app, me);
        press(&mut app, into_it);
        crate::testing::settle(&mut app);
        press(&mut app, KeyCode::Enter);
        crate::testing::settle(&mut app);
        let kept = |app: &App| (app.world().resource::<crate::upgrades::Taken>().0.len(), app.world().resource::<crate::climb::Deepest>().0);
        assert_eq!(kept(&app), (1, 3), "the run charged deck three's reactor and took an upgrade for it");

        app.world_mut().write_message(Restart::fresh());
        crate::testing::settle(&mut app);
        crate::testing::settle(&mut app);
        assert_eq!(kept(&app), (0, 1), "the next run has taken nothing and been nowhere below deck one");
    }

    #[test]
    fn on_a_fresh_run_the_player_is_dealt_the_first_turn_over_a_span_of_seeds() {
        for s in 0..8u64 {
            let mut app = crate::testing::headless(RunSeed(s));
            app.init_resource::<Dealt>();
            app.add_systems(Turn, record_deals.in_set(TurnSet::Decide));
            app.update();
            app.update();
            let dealt = &app.world().resource::<Dealt>().0;
            assert!(!dealt.is_empty(), "seed {s}: nobody was ever dealt a turn");
            assert_eq!(dealt.first(), Some(&true), "seed {s}: {dealt:?} dealt someone else the first turn before the player");
        }
    }
}
