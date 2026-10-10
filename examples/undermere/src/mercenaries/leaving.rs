//! A mercenary out of the party: let go, back to waiting to be hired, or
//! fallen, for good.

use bevy::ecs::message::MessageCursor;
use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_core::noun::{Countability, counted_as};

use super::*;
use crate::allies::Ally;
use crate::armory::Armory;
use crate::content::TOWNSFOLK;

/// Answers a hired mercenary's [`DISMISS`], which cost nothing: asks
/// whether the adventurer means it, naming it, on the offers screen.
///
/// In `TurnSet::React`, beside hiring's own answer. Nothing is done until
/// the yes, so backing out of the question leaves the party as it was and
/// the turn unspent.
pub fn answer_dismiss(mut done: MessageReader<Interacted>, verbs: Res<Verbs>, hired: Query<&Mercenary>, mut asks: MessageWriter<AskToConfirm>) {
    let Some(dismiss) = verbs.get(DISMISS) else { return };
    for ev in done.read().filter(|ev| ev.verb == dismiss) {
        let Ok(hired) = hired.get(ev.target) else { continue };
        let name = &hired.name;
        let prompt = format!("Dismiss {name}? The fee is not given back, and {name} sets down everything {name} carries here.");
        asks.write(AskToConfirm { user: ev.actor, asker: ev.target, verb: dismiss, prompt });
    }
}

/// Lets go every mercenary the adventurer said yes to dismissing, by
/// [`dismiss`], and says so in the mercenaries' tone, naming what it set
/// down: `Brenna leaves your service and sets down a sword and leather
/// armor.`
///
/// Every frame, after the offers screen's keys and before the turns: the
/// yes spends no time, as the offer that asked spent none, so it is
/// written to the log here rather than told inside a pass.
pub fn let_go(world: &mut World, mut cursor: Local<MessageCursor<Confirmed>>) {
    let (Some(dismissing), Some(messages)) = (world.resource::<Verbs>().get(DISMISS), world.get_resource::<Messages<Confirmed>>()) else { return };
    let yeses: Vec<Entity> = cursor.read(messages).filter(|yes| yes.verb == dismissing).map(|yes| yes.asker).collect();
    for mercenary in yeses {
        let Some(name) = world.get::<Mercenary>(mercenary).map(|m| m.name.clone()) else { continue };
        let set_down: Vec<String> = dismiss(world, mercenary).into_iter().map(|thing| said_of(world, thing)).collect();
        let line = if set_down.is_empty() {
            format!("{name} leaves your service.")
        } else {
            format!("{name} leaves your service and sets down {}.", and_list(&set_down))
        };
        let tone = world.resource::<Tones>().get(TONE).unwrap_or(Tones::TEXT);
        let turn = world.resource::<Turns>().turn_number();
        world.resource_mut::<MessageLog>().push(line, tone, turn);
    }
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
/// order, post or mind, as a candidate waits; everything it carried and
/// wore set down on its cell, own kit and given things alike, since
/// nothing a mercenary is handed is taken back but this way; and a
/// [`Candidate`] offering [`HIRE`] at the fee of what it now carries, which
/// is nothing, and talk. What it was hired for is spent, as it was when it
/// was paid. Nothing for one that is no mercenary.
///
/// It waits wherever it was let go: in Mereholt it is one more at the
/// Sunken Lantern until the next visit seats three others, and below it
/// stands at peace with every side until then too.
pub fn dismiss(world: &mut World, mercenary: Entity) -> Vec<Entity> {
    if world.get::<Mercenary>(mercenary).is_none() {
        return Vec::new();
    }
    let set_down = set_down_everything(world, mercenary);
    let townsfolk = world.resource::<Registries>().factions.expect(TOWNSFOLK);
    let fee = world.resource::<Mercenaries>().fee(&[], world.resource::<Armory>());
    let offers = candidate_offers(world.resource::<Verbs>(), fee);
    let mut e = world.entity_mut(mercenary);
    e.remove::<(Mercenary, Ally, Leader, Post, Mind)>().insert((Faction(townsfolk), Candidate { fee }, offers));
    if let Some(mut aware) = e.get_mut::<Aware>() {
        aware.0.clear();
    }
    set_down
}

/// Takes everything `who` carries and wears off it and lays it on its
/// cell, as the engine lets the dead's fall, and returns what it laid.
fn set_down_everything(world: &mut World, who: Entity) -> Vec<Entity> {
    let (Some(at), Some(map)) = (world.get::<Position>(who).copied(), world.get::<OnMap>(who).copied()) else { return Vec::new() };
    let carried: Vec<Entity> = world.get_mut::<Inventory>(who).map(|mut bag| std::mem::take(&mut bag.items)).unwrap_or_default();
    if let Some(mut worn) = world.get_mut::<Equipped>(who) {
        for thing in &carried {
            worn.unequip(*thing);
        }
    }
    for thing in &carried {
        world.entity_mut(*thing).insert((at, map));
    }
    carried
}

/// The hired mercenaries who died this frame, by the body each left, which
/// is the entity that died, for [`say_who_fell`] to find among the
/// narrator's rows.
#[derive(Resource, Debug, Default)]
pub struct Fell(Vec<Entity>);

/// Takes off a hired mercenary's body what it was in the party for, the
/// moment it dies: its [`Mercenary`], its leader, its post and its offers,
/// so the party, the order key and the offers screen pass it by. Notes it
/// in [`Fell`] for the log.
///
/// In `TurnSet::React`, after the tally has read it as the mercenary it
/// was. A body raised later is stood up as the twin the engine kept, which
/// [`forget_its_life`](crate::effects::bodies::forget_its_life) takes the
/// same things off.
pub fn strip_the_fallen(mut commands: Commands, mut deaths: MessageReader<DeathEvent>, hired: Query<(), With<Mercenary>>, mut fell: ResMut<Fell>) {
    for death in deaths.read().filter(|d| hired.contains(d.entity)) {
        fell.0.push(death.entity);
        commands.entity(death.entity).try_remove::<(Mercenary, Leader, Post, Offers)>();
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
    use crate::testing::{arena_on, at, gold, kill, make, monster, pack, said, set_gold, worn};

    /// A candidate hired from the tavern with its kit, and its name.
    fn hire_one(app: &mut App, me: Entity) -> (Entity, String) {
        let candidate = waiting(app)[0];
        let fee = app.world().get::<Candidate>(candidate).expect("a candidate").fee;
        set_gold(app, me, fee + 500);
        take(app, me, candidate, HIRE);
        let name = app.world().get::<Mercenary>(candidate).expect("hired").name.clone();
        (candidate, name)
    }

    /// Every question put to the player since the last look.
    fn asked(app: &mut App) -> Vec<AskToConfirm> {
        app.world_mut().resource_mut::<Messages<AskToConfirm>>().drain().collect()
    }

    /// Says yes to letting `mercenary` go, as the offers screen's Yes does.
    fn say_yes(app: &mut App, me: Entity, mercenary: Entity) {
        let verb = app.world().resource::<Verbs>().get(DISMISS).expect("dismiss");
        app.world_mut().write_message(Confirmed { user: me, asker: mercenary, verb });
        app.update();
    }

    /// Taking up `dismiss` puts one question, naming the mercenary, and
    /// does nothing else: the turn clock stands where it stood, and with
    /// no yes the mercenary stays as it was hired, following, on the
    /// adventurer's side, carrying its kit and offering `give` and
    /// `dismiss`. Over a span of seeds.
    #[test]
    fn dismissing_asks_once_naming_the_mercenary_and_backing_out_changes_nothing_and_costs_nothing_over_a_span_of_seeds() {
        for seed in 0..4 {
            let (mut app, me) = in_mereholt(seed);
            let (merc, name) = hire_one(&mut app, me);
            let _ = asked(&mut app);
            let (clock, kit, hired_as) = (app.world().resource::<Turns>().now(), pack(&app, merc), app.world().get::<Mercenary>(merc).cloned());
            take(&mut app, me, merc, DISMISS);
            let questions = asked(&mut app);
            assert_eq!(questions.len(), 1, "seed {seed}: asked once: {questions:?}");
            let question = &questions[0];
            assert_eq!((question.user, question.asker), (me, merc), "seed {seed}");
            assert!(question.prompt.starts_with(&format!("Dismiss {name}?")), "seed {seed}: in its name: {}", question.prompt);
            app.update();
            let world = app.world();
            assert_eq!(world.resource::<Turns>().now(), clock, "seed {seed}: no time spent");
            assert_eq!(world.get::<Mercenary>(merc).cloned(), hired_as, "seed {seed}: still hired, on its order");
            assert_eq!(world.get::<Leader>(merc), Some(&Leader(me)), "seed {seed}");
            assert!(world.get::<Ally>(merc).is_some() && world.get::<Candidate>(merc).is_none(), "seed {seed}");
            assert_eq!(pack(&app, merc), kit, "seed {seed}: carrying its kit");
            assert_eq!(offered(&app, merc), [(GIVE.to_string(), None), (DISMISS.to_string(), None)], "seed {seed}");
        }
    }

    /// A yes lets the mercenary go where it stands: everything it carried
    /// and wore lies on its cell, it is on the townsfolk's side again,
    /// following nobody with no mind, and it waits as a candidate asking
    /// the fee of nothing, the base, with talk beside `hire`; the log says
    /// it left and what it set down. Hired again, it costs the base and
    /// follows. Over a span of seeds.
    #[test]
    fn dismissing_sets_down_the_kit_and_offers_hire_again_at_the_base_fee_over_a_span_of_seeds() {
        for seed in 0..4 {
            let (mut app, me) = in_mereholt(seed);
            let (merc, name) = hire_one(&mut app, me);
            let carried: Vec<Entity> = app.world().get::<Inventory>(merc).expect("a pack").items.clone();
            assert!(!carried.is_empty(), "seed {seed}: hired with a kit");
            let cell = at(&app, merc);
            say_yes(&mut app, me, merc);
            let world = app.world();
            let base = world.resource::<Mercenaries>().fee(&[], world.resource::<Armory>());
            for thing in &carried {
                assert_eq!(world.get::<Position>(*thing).map(|p| p.0), Some(cell), "seed {seed}: set down where it stood");
            }
            assert!(pack(&app, merc).is_empty() && worn(&app, merc).iter().all(Option::is_none), "seed {seed}: carrying and wearing nothing");
            let townsfolk = world.resource::<Registries>().factions.expect(TOWNSFOLK);
            assert_eq!(world.get::<Faction>(merc).map(|f| f.0), Some(townsfolk), "seed {seed}: the townsfolk's side");
            assert!(world.get::<Mercenary>(merc).is_none() && world.get::<Ally>(merc).is_none(), "seed {seed}: out of the party");
            assert!(world.get::<Leader>(merc).is_none() && world.get::<Mind>(merc).is_none() && world.get::<Post>(merc).is_none(), "seed {seed}");
            assert_eq!(world.get::<Candidate>(merc), Some(&Candidate { fee: base }), "seed {seed}");
            assert_eq!(offered(&app, merc), [(HIRE.to_string(), Some(base)), ("talk".to_string(), None)], "seed {seed}");
            let line = said(&app).into_iter().find(|l| l.starts_with(&format!("{name} leaves your service and sets down "))).unwrap_or_default();
            assert!(!line.is_empty(), "seed {seed}: {:?}", said(&app));

            set_gold(&mut app, me, base + 7);
            take(&mut app, me, merc, HIRE);
            assert_eq!(gold(&app, me), 7, "seed {seed}: hired again for the base");
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

    /// A rat killed by a hired mercenary adds its threat to the Slain line
    /// and moves nothing else: no kill, no deadliest, no mini-boss, and
    /// every other line of the score as it was. One killed by a freed
    /// captive counts nowhere, as before the party was paid for.
    #[test]
    fn a_hired_mercenarys_kill_counts_on_slain_and_nothing_else_in_the_score_moves() {
        let (mut app, me, here) = arena_on(5, 1);
        let brenna = hired(&mut app, me, here.offset(2, 0), "Brenna");
        let before = app.world().resource::<Tally>().clone();
        let rat = monster(&mut app, "rat", here.offset(3, 0));
        kill(&mut app, rat, Some(brenna));
        let bestiary = app.world().resource::<Bestiary>();
        let rat_threat = threat(bestiary.defs.get(bestiary.defs.expect("rat")));
        let after = app.world().resource::<Tally>().clone();
        assert!((after.threat_slain - before.threat_slain - rat_threat).abs() < 1e-3, "the rat's threat: {after:?}");
        assert_eq!((after.kills, after.deadliest.clone(), after.minibosses), (before.kills, before.deadliest.clone(), before.minibosses));
        let (was, is) = (score(&before, false), score(&after, false));
        for (line, (old, new)) in was.iter().zip(&is).enumerate() {
            if old.0 != "Slain" {
                assert_eq!(old, new, "line {line} moved");
            }
        }
        assert_ne!(was[1], is[1], "the Slain line moved");

        let captive = monster(&mut app, "goblin", here.offset(-2, 0));
        crate::captives::free(app.world_mut(), captive, me);
        let jackal = monster(&mut app, "jackal", here.offset(-3, 0));
        kill(&mut app, jackal, Some(captive));
        assert_eq!(*app.world().resource::<Tally>(), after, "a freed captive's kill counts nowhere");
    }
}
