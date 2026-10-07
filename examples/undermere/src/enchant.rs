//! Enchanting, and the curses it lifts.
//!
//! A weapon, armor, ring, staff or wand has a level, the engine's
//! [`Enchant`], set when it is made and changed only by
//! [`level_up`](crate::armory::level_up), which fits the thing to its new
//! level by the function that made it. The scroll of enchanting's
//! [`Enchanting`] raises the thing it is read on by one: armor one more,
//! a weapon's every roll one more and its wielder's aim five, a ring's
//! power one more, a staff's power and charges, a wand's charges.
//!
//! A thing made below nought is cursed: it carries the engine's [`Stuck`],
//! and once it is on it will not come off. [`bind_on_wear`] says so the
//! moment it is put on. Enchanted up to nought it comes unstuck, its level
//! no longer below nothing; the scroll of remove curse's [`RemoveCurse`]
//! unsticks everything its reader wears and leaves each level where it
//! was, so a cursed ring worth wearing anyway can be kept and still taken
//! off.

use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_core::noun::plural;
use rl_engine::rl_rules::ability::RawValue;

use crate::armory::{Armory, ItemKind};
use crate::lore::{Lore, Unfamiliar, called, kind_called};
use crate::run::Adventurer;

/// Raise the thing a use was made on by one level: the scroll of
/// enchanting's.
///
/// Reads [`Landing::on`]; a landing on nothing in particular does nothing.
/// A thing that takes no levels is left as it was, and the log says so.
#[derive(Debug, Clone, Copy, Default)]
pub struct Enchanting;

impl Effect for Enchanting {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        if let Some(thing) = landing.on {
            world.commands.queue(move |world: &mut World| enchant(world, thing));
        }
    }

    fn describe(&self, _: &Registries, _: EffectBonus) -> String {
        "one more level on the thing it is read on".to_string()
    }
}

impl FromArgs for Enchanting {
    const KIND: &'static str = "Enchant";

    fn from_args(_: &RawValue, _: &Names<'_>) -> Result<Self, String> {
        Ok(Self)
    }
}

/// Raises `thing` a level, and says what came of it in the words the
/// adventurer knows it by: its new level, when they know its level, and
/// that it will come off now, when it was stuck on. A curse lifted from a
/// thing nobody wears is lifted unsaid, since nothing has shown it.
pub fn enchant(world: &mut World, thing: Entity) {
    let Some(kind) = world.get::<ItemKind>(thing).map(|k| k.0) else { return };
    let worn = world.query::<&Equipped>().iter(world).any(|w| w.contains(thing));
    let stuck = worn && world.get::<Stuck>(thing).is_some();
    let level_known = world.get::<Unfamiliar>(thing).is_none();
    let count = world.get::<Stack>(thing).map_or(1, |s| s.count);
    let raised = crate::armory::level_up(world, thing, 1);
    let (armory, lore) = (world.resource::<Armory>(), world.resource::<Lore>());
    let (d, level) = (armory.defs.get(kind), raised.unwrap_or(0));
    let your = kind_called(lore, d);
    let told = level_known && armory.learned_by_wear(kind) && lore.knows(&d.name);
    let line = match raised {
        None if count > 1 => format!("Your {} glow for a moment, and are as they were.", plural(&called(armory, lore, kind, level, level_known))),
        None => format!("Your {} glows for a moment, and is as it was.", called(armory, lore, kind, level, level_known)),
        Some(level) if told => format!("Your {your} glows, and is {level:+} now."),
        Some(_) => format!("Your {your} glows."),
    };
    // A line that has just said what it is now says it will come off
    // without a second "now".
    let freed = stuck && world.get::<Stuck>(thing).is_none();
    let line = match (freed, told) {
        (true, true) => format!("{line} It will come off."),
        (true, false) => format!("{line} It will come off now."),
        (false, _) => line,
    };
    let tone = if raised.is_some() { Tones::GOOD } else { Tones::MUTED };
    world.write_message(Tell::new(line, tone));
}

/// Free everything the user wears to come off: the scroll of remove
/// curse's. Each thing keeps its level, so a cursed ring stays as bad as
/// it was, and can only now be taken off.
#[derive(Debug, Clone, Copy, Default)]
pub struct RemoveCurse;

impl Effect for RemoveCurse {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let reader = landing.user;
        world.commands.queue(move |world: &mut World| remove_curse(world, reader));
    }

    fn describe(&self, _: &Registries, _: EffectBonus) -> String {
        "frees whatever will not come off".to_string()
    }
}

impl FromArgs for RemoveCurse {
    const KIND: &'static str = "RemoveCurse";

    fn from_args(_: &RawValue, _: &Names<'_>) -> Result<Self, String> {
        Ok(Self)
    }
}

/// Takes [`Stuck`] off everything `wearer` wears, and tells them what came
/// free, or that nothing was held.
pub fn remove_curse(world: &mut World, wearer: Entity) {
    let Some(equipped) = world.get::<Equipped>(wearer) else { return };
    let mut held: Vec<Entity> = Vec::new();
    for (_, item) in equipped.0.worn() {
        if world.get::<Stuck>(item).is_some() && !held.contains(&item) {
            held.push(item);
        }
    }
    let mut lines: Vec<String> = Vec::new();
    for item in &held {
        world.entity_mut(*item).remove::<Stuck>();
        if let Some(kind) = world.get::<ItemKind>(*item).map(|k| k.0) {
            let your = kind_called(world.resource::<Lore>(), world.resource::<Armory>().defs.get(kind));
            lines.push(format!("Your {your} will come off now."));
        }
    }
    if world.get::<Player>(wearer).is_none() {
        return;
    }
    if lines.is_empty() {
        lines.push("A cleansing light passes over you, and finds nothing that holds you.".to_string());
    }
    let tone = if held.is_empty() { Tones::MUTED } else { Tones::GOOD };
    for line in lines {
        world.write_message(Tell::new(line, tone));
    }
}

/// Says so when the adventurer puts on a cursed thing: it clings to
/// them, and will not come off. And how deep the curse runs is plain from
/// then on, so the thing is no longer unfamiliar and reads `-2 ring of
/// light`, rather than a `+?` beside the line that has just said it is
/// cursed; it is also what they need to know to plan how to get it off,
/// how many scrolls of enchanting bring it up to nought.
///
/// In `TurnSet::React`, after [`learn_by_use`](crate::lore::learn_by_use),
/// so a ring whose kind putting it on made known is called by its kind.
pub fn bind_on_wear(
    mut commands: Commands,
    mut events: MessageReader<ItemEvent>,
    adventurer: Query<Entity, With<Adventurer>>,
    stuck: Query<&ItemKind, With<Stuck>>,
    armory: Res<Armory>,
    lore: Res<Lore>,
    mut tell: MessageWriter<Tell>,
) {
    let Ok(me) = adventurer.single() else { return };
    for ev in events.read() {
        let ItemEvent::Equipped { actor, item } = *ev else { continue };
        let Ok(kind) = stuck.get(item) else { continue };
        if actor == me {
            let the = kind_called(&lore, armory.defs.get(kind.0));
            tell.write(Tell::new(format!("The {the} clings to you, and will not come off."), Tones::BAD));
            commands.entity(item).try_remove::<Unfamiliar>();
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;
    use rl_engine::rl_core::RunSeed;

    use super::*;
    use crate::dungeon::map_of;
    use crate::save::SLOT;
    use crate::testing::{adventurer, arrive, continued, equip, find, give, headless, make, name_of, read_on, said, unequip, worn};

    /// What `who`'s blows and armor come to, as the engine reads them when
    /// a blow is struck or met.
    fn fights(app: &mut App, who: Entity) -> (Option<DiceRoll>, i32) {
        app.world_mut().run_system_once(move |loadout: Loadout| (loadout.melee(who).map(|m| m.dice), loadout.armor(who))).expect("a loadout")
    }

    /// `who`'s accuracy, as the hit model reads it.
    fn accuracy(app: &App, who: Entity) -> i32 {
        let registries = app.world().resource::<Registries>();
        app.world().get::<StatBlock>(who).expect("stats").0.value(registries.stats.expect(crate::content::ACCURACY), &registries.stats)
    }

    /// The run as written down by an arrival on floor one, and carried on
    /// in a fresh app: the adventurer there and the app.
    fn saved_and_continued(app: &mut App) -> (App, Entity) {
        app.world().resource::<Saves>().delete(SLOT).expect("the slot can be emptied");
        arrive(app, map_of(1));
        let text = app.world().resource::<Saves>().load(SLOT).expect("the slot reads").expect("the arrival wrote the run down");
        let mut carried = continued(&text);
        let me = adventurer(&mut carried);
        (carried, me)
    }

    /// The worn thing in `slot` on `who`, by the slot's name.
    fn in_slot(app: &App, who: Entity, slot: &str) -> Entity {
        let slot = app.world().resource::<Registries>().slots.expect(slot);
        app.world().get::<Equipped>(who).and_then(|w| w.in_slot(slot)).expect("something worn there")
    }

    /// A scroll of enchanting read onto chain mail makes it one better, a
    /// point of armor, and fitted again, as a continued run fits it, it is
    /// still one better and not two.
    #[test]
    fn a_scroll_of_enchanting_read_onto_chain_mail_makes_it_one_better_and_never_twice() {
        let mut app = headless(RunSeed(4));
        let me = adventurer(&mut app);
        let mail = make(&mut app, "chain mail");
        equip(&mut app, me, mail);
        let (_, before) = fights(&mut app, me);
        let scroll = find(&mut app, "scroll of enchanting", 0);
        give(&mut app, me, scroll);
        read_on(&mut app, me, scroll, mail);
        assert_eq!(fights(&mut app, me).1, before + 1, "one better");
        assert_eq!((app.world().get::<Armor>(mail).map(|a| a.0), name_of(&app, mail)), (Some(4), "+1 chain mail".to_string()));
        let (mut carried, me) = saved_and_continued(&mut app);
        assert_eq!(fights(&mut carried, me).1, before + 1, "and still one better, not two, carried on");
        let mail = in_slot(&carried, me, "body");
        assert_eq!((carried.world().get::<Armor>(mail).map(|a| a.0), name_of(&carried, mail)), (Some(4), "+1 chain mail".to_string()));
    }

    /// Enchanting what is worn changes its wearer at once, in the pass the
    /// scroll is read: the sword's every roll and its wielder's accuracy,
    /// a ring's light on the lantern. A continued run keeps every level,
    /// what the adventurer knows of each thing and what they do not, and
    /// the wearer's numbers with them.
    #[test]
    fn enchanting_what_is_worn_changes_its_wearer_at_once_and_a_continued_run_keeps_the_level_and_what_is_known() {
        let mut app = headless(RunSeed(5));
        let me = adventurer(&mut app);
        let sword = in_slot(&app, me, "main hand");
        let ring = find(&mut app, "ring of light", 1);
        equip(&mut app, me, ring);
        let lantern = |app: &App, who: Entity| app.world().get::<LightSource>(in_slot(app, who, "light")).map(|l| l.radius);
        let (blow, armor) = fights(&mut app, me);
        let (aim, reach) = (accuracy(&app, me), lantern(&app, me));
        assert_eq!((name_of(&app, ring), reach), ("+? ring of light".to_string(), Some(8)), "a ring worn shows its kind and not its level");
        for on in [sword, ring] {
            let scroll = find(&mut app, "scroll of enchanting", 0);
            give(&mut app, me, scroll);
            read_on(&mut app, me, scroll, on);
        }
        let plus = |dice: Option<DiceRoll>, n: i32| dice.map(|d| DiceRoll { bonus: d.bonus + n, ..d });
        assert_eq!(fights(&mut app, me), (plus(blow, 1), armor), "the sword strikes one harder at once");
        assert_eq!((accuracy(&app, me), lantern(&app, me)), (aim + 5, reach.map(|r| r + 1)), "and aims five better, and the ring lights a tile further");
        assert_eq!((name_of(&app, sword), name_of(&app, ring)), ("+1 sword".to_string(), "+? ring of light".to_string()));

        let (mut carried, me) = saved_and_continued(&mut app);
        assert_eq!(fights(&mut carried, me), (plus(blow, 1), armor), "carried on, the sword is as it was");
        assert_eq!((accuracy(&carried, me), lantern(&carried, me)), (aim + 5, reach.map(|r| r + 1)), "and so are the aim and the light");
        let (sword, ring) = (in_slot(&carried, me, "main hand"), in_slot(&carried, me, "left finger"));
        assert_eq!((name_of(&carried, sword), name_of(&carried, ring)), ("+1 sword".to_string(), "+? ring of light".to_string()), "and what is known of each");
        assert_eq!(carried.world().get::<Enchant>(ring).map(|e| e.level), Some(2));
    }

    /// However a worn thing's level changes, its wearer's stats follow in
    /// the next pass: a sword raised two levels with nothing else worn,
    /// taken off or spent, and its wielder aims ten better by the next
    /// wait.
    #[test]
    fn a_level_changed_on_a_worn_thing_reaches_its_wearers_stats_in_the_next_pass() {
        let mut app = headless(RunSeed(5));
        let me = adventurer(&mut app);
        let sword = in_slot(&app, me, "main hand");
        let aim = accuracy(&app, me);
        assert_eq!(crate::armory::level_up(app.world_mut(), sword, 2), Some(2));
        crate::testing::pass_turns(&mut app, 1);
        assert_eq!(accuracy(&app, me), aim + 10);
    }

    /// A cursed ring put on is known by how deep its curse runs the moment
    /// it clings, and reads `-2 ring of light` beside the line that says it
    /// will not come off, where a ring that is not cursed still reads `+?`
    /// until it has been worn a while.
    #[test]
    fn a_cursed_ring_put_on_is_known_by_its_level_the_moment_it_clings() {
        let mut app = headless(RunSeed(6));
        let me = adventurer(&mut app);
        let (cursed, plain) = (find(&mut app, "ring of light", -2), find(&mut app, "ring of stealth", 2));
        equip(&mut app, me, cursed);
        equip(&mut app, me, plain);
        assert_eq!(name_of(&app, cursed), "-2 ring of light", "{:#?}", said(&app));
        assert_eq!(name_of(&app, plain), "+? ring of stealth");
    }

    /// A cursed ring will not come off, by the key or by anything put on
    /// over it, until it is enchanted up to nought; and another, freed by
    /// a scroll of remove curse, comes off at the level it was.
    #[test]
    fn a_cursed_ring_will_not_come_off_until_enchanted_to_nought_or_uncursed() {
        let mut app = headless(RunSeed(6));
        let me = adventurer(&mut app);
        let ring = find(&mut app, "ring of stealth", -1);
        equip(&mut app, me, ring);
        let lines = said(&app);
        assert!(lines.iter().any(|l| l == "The ring of stealth clings to you, and will not come off."), "{lines:#?}");
        unequip(&mut app, me, ring);
        assert!(worn(&app, me).contains(&Some(name_of(&app, ring))), "it would not come off");
        let scroll = find(&mut app, "scroll of enchanting", 0);
        give(&mut app, me, scroll);
        read_on(&mut app, me, scroll, ring);
        assert_eq!(app.world().get::<Enchant>(ring).map(|e| e.level), Some(0));
        assert!(said(&app).iter().any(|l| l == "Your ring of stealth glows, and is +0 now. It will come off."), "{:#?}", said(&app));
        unequip(&mut app, me, ring);
        assert!(app.world().get::<Equipped>(me).is_some_and(|w| !w.contains(ring)), "enchanted to nought, it came off");

        let cursed = find(&mut app, "ring of light", -2);
        equip(&mut app, me, cursed);
        unequip(&mut app, me, cursed);
        assert!(app.world().get::<Equipped>(me).is_some_and(|w| w.contains(cursed)), "a second will not come off either");
        let scroll = find(&mut app, "scroll of remove curse", 0);
        give(&mut app, me, scroll);
        crate::testing::use_thing(&mut app, me, scroll);
        unequip(&mut app, me, cursed);
        assert!(app.world().get::<Equipped>(me).is_some_and(|w| !w.contains(cursed)), "freed, it came off");
        assert_eq!(app.world().get::<Enchant>(cursed).map(|e| e.level), Some(-2), "at the level it was");
    }

    /// A cursed thing in the pack, never put on, is enchanted up to nought
    /// as any other is, and its curse lifted unsaid: nothing has shown it.
    #[test]
    fn a_curse_lifted_from_a_thing_never_put_on_is_lifted_unsaid() {
        let mut app = headless(RunSeed(6));
        let me = adventurer(&mut app);
        let ring = find(&mut app, "ring of stealth", -1);
        give(&mut app, me, ring);
        let scroll = find(&mut app, "scroll of enchanting", 0);
        give(&mut app, me, scroll);
        read_on(&mut app, me, scroll, ring);
        assert_eq!((app.world().get::<Enchant>(ring).map(|e| e.level), app.world().get::<Stuck>(ring).is_some()), (Some(0), false));
        let lines = said(&app);
        assert!(lines.iter().any(|l| l.ends_with(" ring glows.")), "{lines:#?}");
        assert!(!lines.iter().any(|l| l.contains("come off")), "nothing says it was ever held: {lines:#?}");
    }

    /// Read onto a thing that takes no levels, a scroll of enchanting is
    /// spent on nothing, and the log says so.
    #[test]
    fn a_scroll_of_enchanting_read_onto_a_potion_is_spent_on_nothing() {
        let mut app = headless(RunSeed(7));
        let me = adventurer(&mut app);
        let potion = in_pack(&app, me, "potion of healing");
        let scroll = find(&mut app, "scroll of enchanting", 0);
        give(&mut app, me, scroll);
        read_on(&mut app, me, scroll, potion);
        let lines = said(&app);
        assert!(lines.iter().any(|l| l == "Your potions of healing glow for a moment, and are as they were."), "{lines:#?}");
        assert!(app.world().get_entity(scroll).is_err(), "the scroll is spent");
    }

    /// The first thing in `who`'s pack called `name`.
    fn in_pack(app: &App, who: Entity, name: &str) -> Entity {
        let bag = &app.world().get::<Inventory>(who).expect("a pack").items;
        *bag.iter().find(|i| name_of(app, **i) == name).expect("it is in the pack")
    }
}
