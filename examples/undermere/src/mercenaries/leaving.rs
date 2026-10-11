//! A mercenary out of the party: let go, back to waiting to be hired, or
//! fallen, for good.

use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_core::noun::{Countability, counted_as};

use super::*;
use crate::allies::Ally;
use crate::armory::{Armory, ItemKind};
use crate::content::TOWNSFOLK;
use crate::shops::Ware;

/// Answers a hired mercenary's [`DISMISS`], already asked and answered
/// yes on the offers screen and its step spent: lets it go by [`dismiss`],
/// and says so in the mercenaries' tone, naming what it set down: `Brenna
/// leaves your service and sets down a sword.`
///
/// In `TurnSet::React`, beside hiring's own answer. A no never reaches
/// here: the engine takes nothing up on one.
pub fn answer_dismiss(mut commands: Commands, mut done: MessageReader<Interacted>, verbs: Res<Verbs>, hired: Query<(), With<Mercenary>>) {
    let Some(dismissing) = verbs.get(DISMISS) else { return };
    for ev in done.read().filter(|ev| ev.verb == dismissing && hired.contains(ev.target)) {
        let mercenary = ev.target;
        commands.queue(move |world: &mut World| let_go(world, mercenary));
    }
}

/// Lets `mercenary` go by [`dismiss`] and tells the log what it set down.
fn let_go(world: &mut World, mercenary: Entity) {
    let Some(name) = world.get::<Mercenary>(mercenary).map(|m| m.name.clone()) else { return };
    let set_down: Vec<String> = dismiss(world, mercenary).into_iter().map(|thing| said_of(world, thing)).collect();
    let line =
        if set_down.is_empty() { format!("{name} leaves your service.") } else { format!("{name} leaves your service and sets down {}.", and_list(&set_down)) };
    let tone = world.resource::<Tones>().get(TONE).unwrap_or(Tones::TEXT);
    world.write_message(Tell::new(line, tone));
}

/// `thing` as a list names it, by the name it carries and how many:
/// `2 potions of healing`, `chain mail`.
fn said_of(world: &World, thing: Entity) -> String {
    let Some(name) = world.get::<Name>(thing) else { return "something".to_string() };
    let count = world.get::<Stack>(thing).map_or(1, |s| s.count);
    let countability = if world.get::<MassNoun>(thing).is_some() { Countability::Mass } else { Countability::Count };
    counted_as(name.as_str(), count, countability)
}

/// Takes `mercenary` out of the party and sets it waiting to be hired
/// again, where it stands, and returns what it set down.
///
/// Back on the townsfolk's side, no [`Ally`], following nobody, with no
/// order, post or mind, as a candidate waits. It keeps its own kit, worn
/// and carried, its [`OwnKit`], and sets down on its cell everything else,
/// whatever it was given or picked up, since nothing a mercenary is handed
/// is taken back but this way. It is a [`Candidate`] again, offering
/// [`HIRE`] at the fee of the kit it keeps, as a candidate seated with
/// that kit would ask, and talk: a hire and a dismissal never buy a kit
/// for less than its fee. What it was hired for is spent, as it was when
/// it was paid. Nothing for one that is no mercenary.
///
/// It waits wherever it was let go: in Mereholt it is one more at the
/// Sunken Lantern until the next visit seats three others, and below it
/// stands at peace with every side until then too.
pub fn dismiss(world: &mut World, mercenary: Entity) -> Vec<Entity> {
    if world.get::<Mercenary>(mercenary).is_none() {
        return Vec::new();
    }
    let set_down = set_down_what_was_given(world, mercenary);
    let townsfolk = world.resource::<Registries>().factions.expect(TOWNSFOLK);
    let kit = kit_of(world, mercenary);
    let fee = world.resource::<Mercenaries>().fee(&kit, world.resource::<Armory>());
    let offers = candidate_offers(world.resource::<Verbs>(), fee);
    let mut e = world.entity_mut(mercenary);
    e.remove::<(Mercenary, Ally, Leader, Post, Mind)>().insert((Faction(townsfolk), Candidate { fee }, offers));
    if let Some(mut aware) = e.get_mut::<Aware>() {
        aware.0.clear();
    }
    set_down
}

/// What `who` carries of its own kit, as a kit is priced: each thing's
/// kind, its level and how many.
fn kit_of(world: &World, who: Entity) -> Vec<Ware> {
    let carried = world.get::<Inventory>(who).map(|bag| bag.items.clone()).unwrap_or_default();
    carried
        .into_iter()
        .filter(|thing| world.get::<OwnKit>(*thing).is_some())
        .filter_map(|thing| {
            let def = world.get::<ItemKind>(thing)?.0;
            let level = world.get::<Enchant>(thing).map_or(0, |e| e.level);
            let count = world.get::<Stack>(thing).map_or(1, |s| s.count);
            Some(Ware { def, count, level })
        })
        .collect()
}

/// Takes everything `who` carries and wears that is not its own kit off
/// it and lays it on its cell, as the engine lets the dead's fall, and
/// returns what it laid.
fn set_down_what_was_given(world: &mut World, who: Entity) -> Vec<Entity> {
    let (Some(at), Some(map)) = (world.get::<Position>(who).copied(), world.get::<OnMap>(who).copied()) else { return Vec::new() };
    let carried: Vec<Entity> = world.get::<Inventory>(who).map(|bag| bag.items.clone()).unwrap_or_default();
    let given: Vec<Entity> = carried.into_iter().filter(|thing| world.get::<OwnKit>(*thing).is_none()).collect();
    if let Some(mut bag) = world.get_mut::<Inventory>(who) {
        bag.items.retain(|thing| !given.contains(thing));
    }
    if let Some(mut worn) = world.get_mut::<Equipped>(who) {
        for thing in &given {
            worn.unequip(*thing);
        }
    }
    for thing in &given {
        world.entity_mut(*thing).insert((at, map));
    }
    given
}

/// The hired mercenaries who died this frame, by the body each left, which
/// is the entity that died, for [`say_who_fell`] to find among the
/// narrator's rows.
#[derive(Resource, Debug, Default)]
pub struct Fell(Vec<Entity>);

/// The body of a hired mercenary, by the name it was hired under: what a
/// save keeps of it, since the body is otherwise written as a plain one of
/// its kind and a continue would lay it back down as "mercenary corpse".
/// The engine names it as remains from the name put back, "the corpse of
/// Brenna", as it did when it fell.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct Fallen(pub String);

/// Takes off a hired mercenary's body what it was in the party for, the
/// moment it dies: its [`Mercenary`], its leader, its post and its offers,
/// so the party, the order key and the offers screen pass it by, and marks
/// it [`Fallen`] by its name. Notes it in [`Fell`] for the log.
///
/// In `TurnSet::React`, after the tally has read it as the mercenary it
/// was. A body raised later is stood up as the twin the engine kept, which
/// [`forget_its_life`](crate::effects::bodies::forget_its_life) takes the
/// same things off.
pub fn strip_the_fallen(mut commands: Commands, mut deaths: MessageReader<DeathEvent>, hired: Query<&Mercenary>, mut fell: ResMut<Fell>) {
    for death in deaths.read() {
        let Ok(hired) = hired.get(death.entity) else { continue };
        fell.0.push(death.entity);
        commands.entity(death.entity).try_remove::<(Mercenary, Leader, Post, Offers)>().try_insert(Fallen(hired.name.clone()));
    }
}

/// Says a hired mercenary's death in the tone of the adventurer's own
/// losses, and always, seen or not, as a blow on the adventurer is: the
/// narrator would say `Brenna dies.` as good news, as it says any death
/// nobody is credited with.
///
/// In `ViewSet::Annotate`, where a game edits the rows before they are
/// spoken, over each death row [`Fell`] holds the dead of: its words kept,
/// its tone made the bad news it is.
pub fn say_who_fell(mut narration: ResMut<NarrationView>, mut fell: ResMut<Fell>) {
    for row in &mut narration.rows {
        let Words::Phrase(phrase) = &row.words else { continue };
        let (dead, text) = match phrase {
            Phrase::Dies => (row.who, "{Who} dies."),
            Phrase::Kills => (row.whom, "{Who} kills {whom}."),
            Phrase::YouKill => (row.whom, "You kill {whom}."),
            _ => continue,
        };
        if !dead.is_some_and(|dead| fell.0.contains(&dead)) {
            continue;
        }
        // The fallen one is named whoever saw it fall; what killed it,
        // only if it was seen.
        if *phrase == Phrase::Dies {
            row.who_seen = true;
        }
        row.words = Words::Own { text: text.to_string(), tone: Tones::BAD };
        row.seen = true;
    }
    fell.0.clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bestiary::Bestiary;
    use crate::mercenaries::fixtures::{hired, in_mereholt, offered, take, waiting};
    use crate::tally::{FallenMercenary, Tally, score};
    use crate::testing::{arena_on, at, gold, kill, make, monster, pack, said, set_gold, settle, worn};

    /// A candidate hired from the tavern with its kit, and its name.
    fn hire_one(app: &mut App, me: Entity) -> (Entity, String) {
        let candidate = waiting(app)[0];
        let fee = app.world().get::<Candidate>(candidate).expect("a candidate").fee;
        set_gold(app, me, fee + 500);
        take(app, me, candidate, HIRE);
        let name = app.world().get::<Mercenary>(candidate).expect("hired").name.clone();
        (candidate, name)
    }

    /// `dismiss` is offered asking first, in the mercenary's name; taken
    /// up, as a yes on the offers screen takes it up, it spends a step and
    /// lets the mercenary go: on the townsfolk's side again, following
    /// nobody with no mind, a candidate offering `hire` and talk, and the
    /// log says it left. Over a span of seeds.
    #[test]
    fn dismissing_asks_first_in_its_name_and_a_yes_lets_it_go_for_a_step_over_a_span_of_seeds() {
        for seed in 0..4 {
            let (mut app, me) = in_mereholt(seed);
            let (merc, name) = hire_one(&mut app, me);
            let dismissing = app.world().resource::<Verbs>().get(DISMISS).expect("dismiss");
            let offer = app.world().get::<Offers>(merc).and_then(|o| o.0.iter().find(|o| o.verb == dismissing).cloned()).expect("dismiss offered");
            assert!(
                offer.asks.as_ref().and_then(AsksFirst::question).is_some_and(|q| q.starts_with(&format!("Dismiss {name}?"))),
                "seed {seed}: asks in its name: {offer:?}"
            );
            let clock = app.world().resource::<Turns>().now();
            take(&mut app, me, merc, DISMISS);
            let world = app.world();
            assert_eq!(world.resource::<Turns>().now() - clock, DISMISS_TIME, "seed {seed}: a step");
            let townsfolk = world.resource::<Registries>().factions.expect(TOWNSFOLK);
            assert_eq!(world.get::<Faction>(merc).map(|f| f.0), Some(townsfolk), "seed {seed}: the townsfolk's side");
            assert!(world.get::<Mercenary>(merc).is_none() && world.get::<Ally>(merc).is_none(), "seed {seed}: out of the party");
            assert!(world.get::<Leader>(merc).is_none() && world.get::<Mind>(merc).is_none() && world.get::<Post>(merc).is_none(), "seed {seed}");
            assert_eq!(offered(&app, merc).iter().map(|(v, _)| v.as_str()).collect::<Vec<_>>(), [HIRE, "talk"], "seed {seed}");
            assert!(said(&app).iter().any(|l| l.starts_with(&format!("{name} leaves your service"))), "seed {seed}: {:?}", said(&app));
        }
    }

    /// Let go, a mercenary keeps the kit it was hired with, worn and
    /// carried, and sets down on its cell a dagger it was given; it asks
    /// the fee of the kit it keeps, the fee it was hired for, so hiring and
    /// letting it go never buys its kit cheap; hired again, it costs that
    /// and follows. Over a span of seeds.
    #[test]
    fn a_dismissed_mercenary_keeps_its_own_kit_sets_down_what_it_was_given_and_asks_its_kits_fee_over_a_span_of_seeds() {
        for seed in 0..4 {
            let (mut app, me) = in_mereholt(seed);
            let (merc, name) = hire_one(&mut app, me);
            let fee = app.world().get::<Mercenary>(merc).expect("hired").fee;
            let (kit, wearing) = (pack(&app, merc), worn(&app, merc));
            let dagger = make(&mut app, "dagger");
            app.world_mut().get_mut::<Inventory>(merc).expect("a pack").items.push(dagger);
            let cell = at(&app, merc);
            take(&mut app, me, merc, DISMISS);
            let world = app.world();
            assert_eq!(world.get::<Position>(dagger).map(|p| p.0), Some(cell), "seed {seed}: the dagger set down where it stood");
            assert_eq!((pack(&app, merc), worn(&app, merc)), (kit, wearing), "seed {seed}: its own kit kept, worn as it was");
            assert_eq!(world.get::<Candidate>(merc), Some(&Candidate { fee }), "seed {seed}: the fee of its kit");
            assert_eq!(offered(&app, merc)[0], (HIRE.to_string(), Some(fee)), "seed {seed}");
            assert!(said(&app).iter().any(|l| l == &format!("{name} leaves your service and sets down a dagger.")), "seed {seed}: {:?}", said(&app));

            set_gold(&mut app, me, fee + 7);
            take(&mut app, me, merc, HIRE);
            assert_eq!(gold(&app, me), 7, "seed {seed}: hired again for its kit's fee");
            assert_eq!(app.world().get::<Leader>(merc), Some(&Leader(me)), "seed {seed}: following again");
        }
    }

    /// A goblin kills a hired mercenary on floor three: the mercenary is
    /// gone from the party for good, its kit lies where it fell, the log
    /// says it in the tone of the adventurer's own losses, and the tally
    /// counts it lost, by name, floor and killer. The goblin's kill is no
    /// kill of the party's. Then the adventurer's own hand on a second is
    /// a loss too, and no kill.
    #[test]
    fn a_mercenarys_death_is_counted_with_its_name_floor_and_killer_and_said_as_a_loss() {
        let (mut app, me, here) = arena_on(3, 3);
        let brenna = hired(&mut app, me, here.offset(2, 0), "Brenna");
        let sword = make(&mut app, "dagger");
        app.world_mut().get_mut::<Inventory>(brenna).expect("a pack").items.push(sword);
        let goblin = monster(&mut app, "goblin", here.offset(3, 0));
        let fell_at = at(&app, brenna);
        kill(&mut app, brenna, Some(goblin));
        let world = app.world();
        assert!(world.get::<Mercenary>(brenna).is_none() && world.get::<Leader>(brenna).is_none(), "out of the party");
        assert_eq!(world.get::<Position>(sword).map(|p| p.0), Some(fell_at), "its kit lies where it fell");
        let line = world.resource::<MessageLog>().iter().find(|l| l.text == "The goblin kills Brenna.").map(|l| l.tone);
        assert_eq!(line, Some(Tones::BAD), "said as a loss: {:?}", said(&app));
        let tally = world.resource::<Tally>();
        assert_eq!(tally.mercenaries_lost(), 1);
        assert_eq!(tally.mercenaries_fallen, [FallenMercenary { name: "Brenna".into(), floor: 3, by: Some("a goblin".into()) }]);
        assert_eq!((tally.kills, tally.threat_slain), (0, 0.0), "no kill of the party's");

        let mareth = hired(&mut app, me, here.offset(-2, 0), "Mareth");
        kill(&mut app, mareth, Some(me));
        let tally = app.world().resource::<Tally>();
        assert_eq!(tally.mercenaries_lost(), 2);
        assert_eq!(tally.mercenaries_fallen[1].by.as_deref(), Some("your own hand"));
        assert_eq!((tally.kills, tally.threat_slain), (0, 0.0), "cutting down one's own hireling scores nothing");
    }

    /// A mercenary killed on floor three is named in the run's record,
    /// under its numbers, with where it fell and to what, and the numbers
    /// count it lost; the record kept beyond the run says the same. A
    /// continued run's tally still holds it.
    #[test]
    fn a_fallen_mercenary_is_named_in_the_record_and_counted_among_its_numbers() {
        let (mut app, me, here) = arena_on(4, 3);
        let brenna = hired(&mut app, me, here.offset(2, 0), "Brenna");
        let goblin = monster(&mut app, "goblin", here.offset(1, 0));
        kill(&mut app, brenna, Some(goblin));
        crate::testing::arrive(&mut app, crate::dungeon::map_of(4));
        let text = app.world().resource::<Saves>().load(crate::save::SLOT).expect("the slot reads").expect("the arrival wrote the run down");
        let continued = crate::testing::continued(&text);
        assert_eq!(continued.world().resource::<Tally>().mercenaries_fallen, app.world().resource::<Tally>().mercenaries_fallen, "kept by a save");

        let me = crate::testing::adventurer(&mut app);
        kill(&mut app, me, Some(goblin));
        let record = app.world().resource::<RunRecords>().this_run.clone().expect("the run's end filed a record");
        let headings: Vec<&str> = record.sections.iter().map(|s| s.heading.as_str()).collect();
        assert_eq!(headings, ["Score", "The run", crate::tally::FALLEN]);
        assert_eq!(record.sections[2].body, "Brenna, slain by a goblin on floor 3");
        let lost = record.sections[1].body.lines().find(|l| l.starts_with("Mercenaries lost ")).unwrap_or_default();
        assert!(lost.ends_with(" 1"), "counted among the numbers: {:?}", record.sections[1].body);
        let kept = app.world().resource::<Saves>().load(rl_engine::rl_save::RECORDS_SLOT).expect("the slot reads").expect("the records were written down");
        assert!(kept.contains("Brenna, slain by a goblin on floor 3"), "kept beyond the run: {kept}");
    }

    /// A rat killed by a hired mercenary is a kill of the run's: its
    /// threat on the Slain line, a kill, the deadliest and the damage it
    /// took among the run's numbers, and every other line of the score as
    /// it was; a mini-boss it kills is no mini-boss of the adventurer's.
    /// One killed by a freed captive counts nowhere, as before the party
    /// was paid for.
    #[test]
    fn a_hired_mercenarys_kill_counts_on_slain_kills_deadliest_and_damage_and_nothing_else_in_the_score_moves() {
        let (mut app, me, here) = arena_on(5, 1);
        let brenna = hired(&mut app, me, here.offset(2, 0), "Brenna");
        let before = app.world().resource::<Tally>().clone();
        let rat = monster(&mut app, "rat", here.offset(3, 0));
        let rat_health = app.world().get::<Health>(rat).expect("health").current as u32;
        kill(&mut app, rat, Some(brenna));
        let bestiary = app.world().resource::<Bestiary>();
        let rat_threat = threat(bestiary.defs.get(bestiary.defs.expect("rat")));
        let after = app.world().resource::<Tally>().clone();
        assert!((after.threat_slain - before.threat_slain - rat_threat).abs() < 1e-3, "the rat's threat: {after:?}");
        assert_eq!(after.kills, before.kills + 1, "a kill: {after:?}");
        assert_eq!(after.deadliest.as_ref().map(|(n, _)| n.as_str()), Some("rat"), "the deadliest: {after:?}");
        assert_eq!(after.damage_dealt, before.damage_dealt + rat_health, "what it took: {after:?}");
        assert_eq!(after.minibosses, before.minibosses);
        let (was, is) = (score(&before, false), score(&after, false));
        for (line, (old, new)) in was.iter().zip(&is).enumerate() {
            if old.0 != "Slain" {
                assert_eq!(old, new, "line {line} moved");
            }
        }
        assert_ne!(was[1], is[1], "the Slain line moved");

        let king = monster(&mut app, "Grubnak the goblin king", here.offset(4, 0));
        kill(&mut app, king, Some(brenna));
        let after = app.world().resource::<Tally>().clone();
        assert_eq!((after.kills, after.minibosses), (before.kills + 2, before.minibosses), "a kill, but no mini-boss of theirs: {after:?}");

        let captive = monster(&mut app, "goblin", here.offset(-2, 0));
        crate::captives::free(app.world_mut(), captive, me);
        let jackal = monster(&mut app, "jackal", here.offset(-3, 0));
        kill(&mut app, jackal, Some(captive));
        assert_eq!(*app.world().resource::<Tally>(), after, "a freed captive's kill counts nowhere");
    }

    /// A hired mercenary's blow on one of the party, a second mercenary, a
    /// freed captive or the adventurer, is no damage dealt, as an area
    /// blow that caught a friend is not the fighting the number counts;
    /// the same blow on a rat beside them is. A blow on the adventurer is
    /// still damage taken, whoever struck it.
    #[test]
    fn a_hired_mercenarys_blow_on_one_of_the_party_is_no_damage_dealt_and_on_a_foe_it_is() {
        let (mut app, me, here) = arena_on(5, 1);
        let brenna = hired(&mut app, me, here.offset(2, 0), "Brenna");
        let mareth = hired(&mut app, me, here.offset(-2, 0), "Mareth");
        let captive = monster(&mut app, "goblin", here.offset(0, 2));
        crate::captives::free(app.world_mut(), captive, me);
        let rat = monster(&mut app, "rat", here.offset(3, 0));
        let crush = app.world().resource::<Registries>().damage_kinds.expect("crush");
        let strike = |app: &mut App, target| {
            app.world_mut().write_message(DamageEvent::new(target, rl_engine::rl_rules::Hit::from_source(Some(brenna), crush, 3)));
            settle(app);
        };
        let before = app.world().resource::<Tally>().clone();
        let health = |app: &App, who| app.world().get::<Health>(who).expect("health").current;
        for (friend, who) in [(mareth, "a mercenary"), (captive, "a freed captive"), (me, "the adventurer")] {
            let was = health(&app, friend);
            strike(&mut app, friend);
            assert!(health(&app, friend) < was, "the blow on {who} landed");
            assert_eq!(app.world().resource::<Tally>().damage_dealt, before.damage_dealt, "a blow on {who} dealt nothing");
        }
        assert!(app.world().resource::<Tally>().damage_taken > before.damage_taken, "the adventurer still took it");
        strike(&mut app, rat);
        assert!(app.world().resource::<Tally>().damage_dealt > before.damage_dealt, "the rat's blow counted");
    }
}
