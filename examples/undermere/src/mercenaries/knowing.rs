//! What a hired mercenary carries and knows: its own kit, which it uses
//! freely, and anything else, which it uses only once the adventurer
//! knows it, so nothing it does tells the adventurer what they do not.

use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_core::noun::{Countability, counted_as};
use rl_engine::rl_rules::Equipment;
use rl_engine::rl_rules::ai::brain::{Choice, Tactic};

use super::{GIVE, Mercenary, TONE};
use crate::armory::{Armory, ItemKind};
use crate::lore::Lore;
use crate::run::Adventurer;

/// A thing a candidate was made with, which it knows whole: it was hired
/// with it, so it drinks its own potion whether or not the adventurer
/// knows the kind.
///
/// Put on by [`make_kit`](super::make_kit) and kept only while the thing
/// is carried: [`release_own_kit`] takes it off whatever lies on a floor,
/// dropped by the dead or thrown, so a potion picked up again is a find
/// like any other.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OwnKit;

/// Takes [`OwnKit`] off whatever lies on a floor.
///
/// In `CleanupSet::Requeue`, after the dead have dropped what they
/// carried, so nothing lies about still marked by the end of the pass.
pub fn release_own_kit(mut commands: Commands, lying: Query<Entity, (With<OwnKit>, With<Position>)>) {
    for thing in &lying {
        commands.entity(thing).remove::<OwnKit>();
    }
}

/// The adventurer as one who sees: who, their sight, and where.
type Onlooker = (Entity, Option<&'static Viewshed>, Option<&'static OnMap>);

/// Whether a hired mercenary stands where the adventurer can see it, on
/// the map they are on: what it does there is done before their eyes.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Witnessed<'w, 's> {
    adventurer: Query<'w, 's, Onlooker, With<Adventurer>>,
    hired: Query<'w, 's, (&'static Mercenary, &'static Position, Option<&'static OnMap>)>,
}

impl Witnessed<'_, '_> {
    /// Whether `who` is a hired mercenary the adventurer sees now.
    pub fn sees(&self, who: Entity) -> bool {
        let (Ok((_, Some(sight), here)), Ok((_, at, there))) = (self.adventurer.single(), self.hired.get(who)) else { return false };
        here.map(|m| m.0) == there.map(|m| m.0) && sight.can_see(at.0)
    }

    /// What the adventurer calls `actor` when they see it do something:
    /// `you` for themselves, its name for a hired mercenary they see, and
    /// `None` for anyone whose doings they do not learn from.
    pub fn called(&self, actor: Entity) -> Option<String> {
        if self.adventurer.single().is_ok_and(|(me, ..)| me == actor) {
            return Some("you".to_string());
        }
        self.sees(actor).then(|| self.hired.get(actor).map(|(hired, ..)| hired.name.clone()).ok()).flatten()
    }
}

/// What a thing's worth to wear is reckoned from, as the adventurer would
/// reckon it: its kind, its score, and whether it is known or the
/// wearer's own.
type Reckoned = (Option<&'static ItemKind>, Option<&'static GearScore>, Has<Unknown>, Has<OwnKit>);

/// What a thing seems worth to a mercenary wearing it: its true score
/// when the adventurer knows it or it is the mercenary's own, its kind's
/// plain score when only its level is unknown, and nothing when its kind
/// is unknown, since a ring of a kind nobody knows could be any ring.
///
/// The same correction [`seeming`](crate::seeming) makes for the vitals
/// and the forecast, made here for a mind: without it a mercenary that
/// stepped past a sword would say it was cursed.
#[derive(bevy::ecs::system::SystemParam)]
pub struct SeenWorth<'w, 's> {
    armory: Res<'w, Armory>,
    lore: Res<'w, Lore>,
    things: Query<'w, 's, Reckoned>,
    wearables: Query<'w, 's, &'static Wearable>,
    worn: Query<'w, 's, &'static Equipped>,
    stuck: Query<'w, 's, (), With<Stuck>>,
    blows: Query<'w, 's, (), (With<Item>, With<MeleeAttack>)>,
    own: Query<'w, 's, &'static OwnBlowScore>,
}

impl SeenWorth<'_, '_> {
    /// What `thing` seems worth worn, or `None` for a thing worth nothing
    /// worn or of a kind not known.
    pub fn score(&self, thing: Entity) -> Option<i32> {
        let (kind, score, unknown, own) = self.things.get(thing).ok()?;
        if !unknown || own {
            return score.map(|s| s.0);
        }
        let kind = kind?.0;
        self.lore.knows(&self.armory.defs.get(kind).name).then(|| self.armory.score_at(kind, 0)).flatten()
    }

    /// How much better `wearer` seems to be for putting `thing` on than
    /// for what it would displace and, for a thing it would strike with
    /// where it strikes with its own now, than for its own blow: the
    /// engine's gain, by the rule the resolver equips by, counted at
    /// [`score`](Self::score). `None` when it wears nothing, the thing is
    /// not worn or seems worth nothing, or putting it on would push off a
    /// thing that will not come off.
    pub fn gain(&self, wearer: Entity, thing: Entity) -> Option<i32> {
        let worn = self.worn.get(wearer).ok()?;
        let shape = self.wearables.get(thing).ok()?;
        let score = self.score(thing)?;
        let mut after = worn.0.clone();
        let displaced = after.equip_keeping(thing, &shape.0, |other| self.stuck.contains(other)).ok()?;
        let given_up: i32 = displaced.iter().map(|d| self.score(*d).unwrap_or(0)).sum();
        let strikes_with = |worn: &Equipment<Entity>| worn.worn().map(|(_, t)| t).find(|t| self.blows.contains(*t));
        let own = match (strikes_with(&worn.0), strikes_with(&after)) {
            (None, Some(blow)) if blow == thing => self.own.get(wearer).map_or(0, |o| o.0),
            _ => 0,
        };
        Some(score - given_up - own)
    }
}

/// The thing in a mercenary's pack it would be best off putting on, worth
/// more than what it would displace, as [`SeenWorth`] reckons it: what
/// [`PutOnBetter`] reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BetterInPack(pub Entity);

/// Leaves a hired mercenary's mind knowing only what the adventurer knows
/// of what is not its own: it uses nothing it carries that is not yet
/// known and not [`OwnKit`], it counts gear lying in sight at what it
/// seems worth, and it is told the best thing in its pack to put on, if
/// any is better than what it wears.
///
/// In `DecideSet::Perceive`, after the engine has told the mind what it
/// carries and what lies in sight, in `PerceiveSet::Annotate`, as
/// [`throw_what_strikes`](crate::bestiary::throw_what_strikes) trims a
/// thrower's missiles. Without it a mercenary drinking a found potion when
/// hurt would say it mends.
pub fn know_as_the_adventurer_does(
    mut thinking: ResMut<Thinking>,
    hired: Query<(&Inventory, Option<&Equipped>), With<Mercenary>>,
    known: Query<(Has<Unknown>, Has<OwnKit>)>,
    worth: SeenWorth,
) {
    let Some((me, (bag, worn))) = thinking.actor().and_then(|me| hired.get(me).ok().map(|h| (me, h))) else { return };
    let better = bag
        .items
        .iter()
        .filter(|thing| worn.is_none_or(|w| !w.contains(**thing)))
        .filter_map(|thing| worth.gain(me, *thing).filter(|gain| *gain > 0).map(|gain| (gain, *thing)))
        .fold(None, |best: Option<(i32, Entity)>, (gain, thing)| if best.is_some_and(|(b, _)| b >= gain) { best } else { Some((gain, thing)) });
    let Some(snapshot) = thinking.snapshot_mut() else { return };
    snapshot.carried.retain(|c| known.get(c.item).is_ok_and(|(unknown, own)| !unknown || own));
    for lying in &mut snapshot.items {
        lying.gain = worth.gain(me, lying.id);
    }
    if let Some((_, thing)) = better {
        snapshot.senses.push(Box::new(BetterInPack(thing)));
    }
}

/// Putting on the thing in its pack it is best off wearing, when
/// [`know_as_the_adventurer_does`] found one: a sword handed to it is worn
/// on its next turn with nothing more pressing to do. Every brain asks it
/// after striking and throwing, so one handed a sword mid-fight strikes
/// with what it holds and arms once the fight lets it.
#[derive(Debug, Clone, Copy)]
pub struct PutOnBetter;

impl Tactic<Entity> for PutOnBetter {
    fn name(&self) -> &'static str {
        "arming"
    }

    fn evaluate(&self, ctx: &mut TacticCtx<'_, Entity>) -> Option<Decision<Entity>> {
        let better = ctx.snapshot.sense::<BetterInPack>()?;
        Some(Decision::own(PutOn(better.0)))
    }
}

/// A mind's choice to put on a thing it carries: the engine's `Equip` by
/// another name, since a mind has no decision of the engine's for it and
/// a game cannot make the engine's action its choice. [`put_on`] hands it
/// to the engine's resolver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PutOn(pub Entity);

impl Action for PutOn {}

impl Choice for PutOn {
    fn name(&self) -> &'static str {
        "put on"
    }
}

/// Hands each [`PutOn`] to the engine as the `Equip` it is, so what is
/// refused, worn or narrated is the engine's rule for every wearer.
///
/// In `ResolveSet::Redirect`, after the minds have chosen and before the
/// engine's resolver reads its intents.
pub fn put_on(mut chosen: MessageReader<Intent<PutOn>>, mut equips: MessageWriter<Intent<Equip>>) {
    for intent in chosen.read() {
        equips.write(Intent::new(intent.actor, Equip(intent.action.0)));
    }
}

/// What deciding whether a thing may be given reads of it: what it is
/// tagged, whether it is worn, thrown or used, and its kind.
type Offered = (Option<&'static Tagged>, Has<Wearable>, Has<Throwable>, Option<&'static Triggers>, Option<&'static ItemKind>);

/// What someone has to give a mercenary: everything in their pack they do
/// not wear, that is not their purse, and that a mercenary can put on,
/// throw or use, by the engine's own rule for a thing a mind is told it
/// carries: a use of its own that asks for no other thing.
///
/// Never a waystone, though it is used from the pack: what it calls is
/// the gate home for the adventurer, and in a hireling's hands it would
/// be the adventurer's way back given to someone who can do nothing with
/// it.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Givable<'w, 's> {
    bags: Query<'w, 's, (&'static Inventory, Option<&'static Equipped>)>,
    things: Query<'w, 's, Offered>,
    currency: Option<Res<'w, Currency>>,
    armory: Res<'w, Armory>,
}

impl Givable<'_, '_> {
    /// What `who` has to give, in their pack's order.
    pub fn of(&self, who: Entity) -> Vec<Entity> {
        let Ok((bag, worn)) = self.bags.get(who) else { return Vec::new() };
        let waystone = self.armory.defs.id(crate::waystone::WAYSTONE);
        let fit = |thing: Entity| {
            let Ok((tagged, wearable, throwable, does, kind)) = self.things.get(thing) else { return false };
            let purse = self.currency.as_deref().is_some_and(|c| c.is(tagged));
            let usable = does.is_some_and(|d| d.on(Moments::USE).next().is_some() && d.asks().is_none());
            !purse && kind.is_none_or(|k| Some(k.0) != waystone) && (wearable || throwable || usable)
        };
        bag.items.iter().copied().filter(|t| worn.is_none_or(|w| !w.contains(*t)) && fit(*t)).collect()
    }
}

/// Answers a hired mercenary's [`GIVE`], its step already spent: asks the
/// bag which of the adventurer's things to hand over, among what
/// [`Givable`] offers. With nothing to give, says so.
///
/// In `TurnSet::React`, beside hiring's own answer.
pub fn answer_give(
    mut done: MessageReader<Interacted>,
    verbs: Res<Verbs>,
    hired: Query<&Mercenary>,
    givable: Givable,
    mut asks: MessageWriter<AskForThing>,
    mut tell: MessageWriter<Tell>,
) {
    let Some(give) = verbs.get(GIVE) else { return };
    for ev in done.read().filter(|ev| ev.verb == give) {
        let Ok(hired) = hired.get(ev.target) else { continue };
        let choices = givable.of(ev.actor);
        if choices.is_empty() {
            tell.write(Tell::new(format!("You have nothing to give {}.", hired.name), Tones::MUTED));
            continue;
        }
        asks.write(AskForThing { user: ev.actor, asker: ev.target, verb: give, prompt: format!("Give {} which?", hired.name), choices });
    }
}

/// What handing a thing over reads of it: its name, how many, and what it
/// does when used, which says whether it goes over whole.
type Handed = (&'static Name, Option<&'static mut Stack>, Has<MassNoun>, Option<&'static Triggers>);

/// The packs a thing is handed between, and what it is called.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Handing<'w, 's> {
    commands: Commands<'w, 's>,
    bags: Query<'w, 's, &'static mut Inventory>,
    things: Query<'w, 's, Handed>,
}

impl Handing<'_, '_> {
    /// Moves `thing` from `from`'s pack to `to`'s, and says what went,
    /// `a potion of healing`; `None`, and nothing moved, when `from` does
    /// not carry it or either has no pack.
    ///
    /// One off the top of a stack of anything with a use, drunk or read, a
    /// copy of its own, since nothing handed over comes back and a whole
    /// stack given for one drink is a stack lost; a stack of anything else,
    /// darts and javelins, goes whole, since a thrower with one dart
    /// throws once. A potion can be thrown too, so whether it is thrown is
    /// not the question.
    pub fn hand(&mut self, from: Entity, to: Entity, thing: Entity) -> Option<String> {
        let [mut giver, mut taker] = self.bags.get_many_mut([from, to]).ok()?;
        let at = giver.items.iter().position(|t| *t == thing)?;
        let Ok((name, stack, mass, does)) = self.things.get_mut(thing) else {
            giver.items.remove(at);
            taker.items.push(thing);
            return Some("something".to_string());
        };
        let countable = if mass { Countability::Mass } else { Countability::Count };
        let (handed, count) = match stack {
            Some(mut stack) if stack.count > 1 && does.is_some_and(|d| d.on(Moments::USE).next().is_some()) => {
                stack.count -= 1;
                let key = stack.key;
                (self.commands.entity(thing).clone_and_spawn().insert(Stack { key, count: 1 }).id(), 1)
            }
            stack => {
                giver.items.remove(at);
                (thing, stack.map_or(1, |s| s.count))
            }
        };
        taker.items.push(handed);
        Some(counted_as(name.as_str(), count, countable))
    }
}

/// Moves the thing the bag chose for a [`GIVE`] from the adventurer's pack
/// to the mercenary's, and says so in the mercenaries' tone. Nothing is
/// taken back: what a mercenary carries comes back only when it drops it.
///
/// Every frame, after the bag's keys: the choice spends no time, since
/// the `give` that asked spent its step.
pub fn hand_over(
    mut chosen: MessageReader<ThingChosen>,
    verbs: Res<Verbs>,
    hired: Query<&Mercenary>,
    mut handing: Handing,
    tones: Res<Tones>,
    turns: Res<Turns>,
    mut log: ResMut<MessageLog>,
) {
    let Some(give) = verbs.get(GIVE) else { return };
    for c in chosen.read().filter(|c| c.verb == give) {
        let Ok(hired) = hired.get(c.asker) else { continue };
        let Some(said) = handing.hand(c.user, c.asker, c.thing) else { continue };
        log.push(format!("You give {} {said}.", hired.name), tones.get(TONE).unwrap_or(Tones::TEXT), turns.turn_number());
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::world::CommandQueue;
    use rl_engine::rl_ui::InspectViewPlugin;

    use super::*;
    use crate::mercenaries::fixtures::{hired, take};
    use crate::testing::{arena, arena_with, at, clear_floor, find, give, kill, lay, make, name_of, pass_turns, said, settle, wall_off, wear};

    /// The potion a mercenary mends with that the adventurer does not know
    /// at the start of a run, as they know the healing they set out with.
    const UNKNOWN_MENDING: &str = "potion of extra healing";

    /// Makes `name` at `level` as a thing told rather than found, so its
    /// level is known, placed nowhere: what a hireling's kit is made as.
    fn told(app: &mut App, name: &str, level: i32) -> Entity {
        let mut queue = CommandQueue::default();
        let thing = {
            let world = app.world();
            let armory = world.resource::<Armory>();
            crate::armory::spawn_item_at(&mut Commands::new(&mut queue, world), armory, armory.defs.expect(name), level, world.resource::<Registries>())
        };
        queue.apply(app.world_mut());
        thing
    }

    /// `thing` made `who`'s own kit and put in its pack.
    fn own(app: &mut App, who: Entity, thing: Entity) {
        app.world_mut().entity_mut(thing).insert(OwnKit);
        give(app, who, thing);
    }

    /// Brings `who` down to two fifths of its health: hurt past the half a
    /// mind mends at, and short of the quarter it runs at.
    fn hurt(app: &mut App, who: Entity) {
        let max = app.world().get::<Health>(who).expect("health").max;
        app.world_mut().entity_mut(who).insert(Health { current: max * 2 / 5, max });
    }

    fn carries(app: &App, who: Entity, thing: Entity) -> bool {
        app.world().get::<Inventory>(who).is_some_and(|bag| bag.items.contains(&thing))
    }

    fn wears(app: &App, who: Entity, thing: Entity) -> bool {
        app.world().get::<Equipped>(who).is_some_and(|worn| worn.contains(thing))
    }

    fn knows(app: &App, kind: &str) -> bool {
        app.world().resource::<Lore>().knows(kind)
    }

    /// A mercenary drinks its own potion when hurt whether or not the
    /// adventurer knows the kind; one drunk where they see it teaches them
    /// the kind as their own drinking would, and one drunk behind a wall
    /// teaches them nothing, and the log names nothing.
    #[test]
    fn it_drinks_its_own_potion_when_hurt_unknown_or_not_and_the_adventurer_learns_the_kind_in_sight_and_not_out_of_it_over_a_span_of_seeds() {
        for seed in 0..4 {
            for (kind, seen) in [(UNKNOWN_MENDING, true), (UNKNOWN_MENDING, false), ("potion of healing", true)] {
                let (mut app, me, here) = arena(seed);
                assert_eq!(knows(&app, kind), kind != UNKNOWN_MENDING, "seed {seed}: {kind} known only when set out with");
                if !seen {
                    let ring: Vec<Point> = (0..8).map(|i| here + Direction::from_index(i).offset()).collect();
                    wall_off(&mut app, &ring);
                }
                let brenna = hired(&mut app, me, here.offset(2, 0), "Brenna");
                let potion = make(&mut app, kind);
                own(&mut app, brenna, potion);
                hurt(&mut app, brenna);
                pass_turns(&mut app, 2);
                assert!(!carries(&app, brenna, potion), "seed {seed}: {kind}, seen {seen}: drunk");
                let told = said(&app).iter().any(|line| line.contains(&format!("That was a {kind}.")));
                assert_eq!(
                    (knows(&app, kind), told),
                    (seen || kind != UNKNOWN_MENDING, seen && kind == UNKNOWN_MENDING),
                    "seed {seed}: {kind}, seen {seen}: {:?}",
                    said(&app)
                );
            }
        }
    }

    /// A potion it found or was handed is drunk only once the adventurer
    /// knows its kind, however hurt it is: before, it would tell them the
    /// draught mends.
    #[test]
    fn it_never_drinks_a_found_potion_the_adventurer_has_not_identified_and_does_once_it_is_identified_over_a_span_of_seeds() {
        for seed in 0..4 {
            let (mut app, me, here) = arena(seed);
            let brenna = hired(&mut app, me, here.offset(2, 0), "Brenna");
            let potion = make(&mut app, UNKNOWN_MENDING);
            give(&mut app, brenna, potion);
            hurt(&mut app, brenna);
            pass_turns(&mut app, 4);
            assert!(carries(&app, brenna, potion) && !knows(&app, UNKNOWN_MENDING), "seed {seed}: kept, and nothing learned");
            crate::lore::identify(app.world_mut(), potion);
            pass_turns(&mut app, 2);
            assert!(!carries(&app, brenna, potion), "seed {seed}: drunk once known");
        }
    }

    /// Gear lying in sight is weighed as the adventurer would weigh it: a
    /// mace two levels better than the one it wields, told, is fetched and
    /// put on in its place, and the same mace found, its level unknown,
    /// counts as a plain one and is passed over, so nothing it does says it
    /// is better. A mace, since it fills the one hand, where a second sword
    /// would go to the free one beside the first.
    #[test]
    fn it_puts_on_a_better_weapon_lying_in_sight_and_passes_over_an_unknown_one_counted_at_plain_numbers_over_a_span_of_seeds() {
        for seed in 0..4 {
            for known in [true, false] {
                let (mut app, me, here) = arena(seed);
                clear_floor(&mut app);
                let brenna = hired(&mut app, me, here.offset(1, 0), "Brenna");
                let mine = told(&mut app, "mace", 1);
                app.world_mut().entity_mut(mine).insert(OwnKit);
                wear(&mut app, brenna, mine);
                let better = if known { told(&mut app, "mace", 3) } else { find(&mut app, "mace", 3) };
                lay(&mut app, better, here.offset(2, 1));
                settle(&mut app);
                pass_turns(&mut app, 6);
                let took = wears(&app, brenna, better);
                assert_eq!((took, wears(&app, brenna, mine)), (known, !known), "seed {seed}, known {known}: what it wields");
                assert_eq!(app.world().get::<Position>(better).is_some(), !known, "seed {seed}, known {known}: what lies there still");
            }
        }
    }

    /// `give` asks the bag which thing, among what the adventurer carries
    /// and does not wear; the thing chosen moves to the mercenary's pack
    /// with a line saying so, and a sword better than its fists is worn on
    /// its next turn.
    #[test]
    fn a_sword_given_through_the_bag_is_worn_on_the_mercenarys_next_turn() {
        let (mut app, me, here) = arena(3);
        let brenna = hired(&mut app, me, here.offset(2, 0), "Brenna");
        let sword = told(&mut app, "sword", 0);
        give(&mut app, me, sword);
        take(&mut app, me, brenna, GIVE);
        let asked: Vec<AskForThing> = app.world_mut().resource_mut::<Messages<AskForThing>>().drain().collect();
        let [ask] = asked.as_slice() else { panic!("one question: {asked:?}") };
        assert_eq!((ask.user, ask.asker, ask.prompt.as_str()), (me, brenna, "Give Brenna which?"));
        let worn = app.world().get::<Equipped>(me).expect("slots").clone();
        assert!(ask.choices.contains(&sword) && ask.choices.iter().all(|t| !worn.contains(*t)), "what is carried and not worn: {ask:?}");
        app.world_mut().write_message(ThingChosen { user: me, asker: brenna, verb: ask.verb, thing: sword });
        app.update();
        assert!(carries(&app, brenna, sword) && !carries(&app, me, sword), "handed over");
        assert!(said(&app).contains(&"You give Brenna a sword.".to_string()), "{:?}", said(&app));
        assert!(!wears(&app, brenna, sword), "not worn before its turn");
        pass_turns(&mut app, 1);
        assert!(wears(&app, brenna, sword), "worn on its next turn");
    }

    /// A sword handed over with a foe beside it is not put on while the
    /// fight lasts: the mercenary strikes with what it holds, and arms
    /// once nothing is beside it to strike.
    #[test]
    fn a_sword_handed_over_mid_fight_waits_until_the_fight_lets_it() {
        let (mut app, me, here) = arena(3);
        let brenna = hired(&mut app, me, here.offset(2, 0), "Brenna");
        let foe = crate::testing::still(&mut app, "goblin", here.offset(3, 0));
        let sword = told(&mut app, "sword", 0);
        give(&mut app, brenna, sword);
        pass_turns(&mut app, 1);
        assert!(!wears(&app, brenna, sword), "not armed while a foe is beside it");
        assert_eq!(app.world().get::<rl_engine::rl_bevy::Doing>(brenna).and_then(|d| d.0), Some("melee_adjacent"), "it struck instead");
        app.world_mut().despawn(foe);
        pass_turns(&mut app, 1);
        assert!(wears(&app, brenna, sword), "armed once the fight is over");
    }

    /// `give` never offers the waystone, the adventurer's way home; a
    /// potion off a stack goes over one at a time, a copy of its own, and
    /// a stack of darts goes whole.
    #[test]
    fn give_offers_no_waystone_and_hands_one_potion_off_a_stack_and_darts_whole() {
        let (mut app, me, here) = arena(3);
        let brenna = hired(&mut app, me, here.offset(2, 0), "Brenna");
        let stacked = |app: &mut App, name: &str, count: u32| {
            let thing = make(app, name);
            app.world_mut().get_mut::<Stack>(thing).expect("a stack").count = count;
            give(app, me, thing);
            thing
        };
        let potions = stacked(&mut app, "potion of healing", 3);
        let darts = stacked(&mut app, "dart", 5);
        let stone = {
            let world = app.world();
            let id = world.resource::<Armory>().defs.expect(crate::waystone::WAYSTONE);
            let bag = &world.get::<Inventory>(me).expect("a pack").items;
            *bag.iter().find(|i| world.get::<ItemKind>(**i).is_some_and(|k| k.0 == id)).expect("a waystone in the pack")
        };
        take(&mut app, me, brenna, GIVE);
        let asked: Vec<AskForThing> = app.world_mut().resource_mut::<Messages<AskForThing>>().drain().collect();
        let [ask] = asked.as_slice() else { panic!("one question: {asked:?}") };
        assert!(ask.choices.contains(&potions) && ask.choices.contains(&darts), "what it can drink and throw: {ask:?}");
        assert!(!ask.choices.contains(&stone), "but never the way home: {ask:?}");

        let verb = ask.verb;
        app.world_mut().write_message(ThingChosen { user: me, asker: brenna, verb, thing: potions });
        app.update();
        let count = |app: &App, thing: Entity| app.world().get::<Stack>(thing).map(|s| s.count);
        assert!(carries(&app, me, potions) && count(&app, potions) == Some(2), "two kept");
        let theirs = app.world().get::<Inventory>(brenna).expect("a pack").items.iter().copied().find(|t| name_of(&app, *t) == name_of(&app, potions));
        assert_eq!(theirs.map(|t| count(&app, t)), Some(Some(1)), "and one of its own handed over");
        assert!(said(&app).iter().any(|l| l.starts_with("You give Brenna a potion")), "{:?}", said(&app));

        app.world_mut().write_message(ThingChosen { user: me, asker: brenna, verb, thing: darts });
        app.update();
        assert!(carries(&app, brenna, darts) && !carries(&app, me, darts) && count(&app, darts) == Some(5), "the darts whole");
        assert!(said(&app).contains(&"You give Brenna 5 darts.".to_string()), "{:?}", said(&app));
    }

    /// A cursed thing a mercenary puts on stays on, as the adventurer's
    /// does: the curse is seen to cling when it goes on in sight, and a
    /// better mace handed over after is carried and never worn in its
    /// place.
    #[test]
    fn a_cursed_thing_it_puts_on_stays_on_and_is_seen_to_cling() {
        let (mut app, me, here) = arena(4);
        let brenna = hired(&mut app, me, here.offset(2, 0), "Brenna");
        let cursed = find(&mut app, "mace", -2);
        give(&mut app, brenna, cursed);
        pass_turns(&mut app, 1);
        assert!(wears(&app, brenna, cursed), "a found mace at plain numbers beats its fists");
        assert!(said(&app).contains(&"The mace clings to Brenna, and will not come off.".to_string()), "{:?}", said(&app));
        let better = told(&mut app, "mace", 3);
        give(&mut app, brenna, better);
        pass_turns(&mut app, 4);
        assert!(wears(&app, brenna, cursed) && !wears(&app, brenna, better), "the cursed mace stays on");
        assert!(app.world().get::<Stuck>(cursed).is_some(), "stuck");
    }

    /// What a mercenary dies carrying falls where it died no longer its own
    /// kit, so a potion of it picked up again is a find like any other.
    #[test]
    fn its_own_kit_dropped_where_it_dies_is_no_longer_its_own() {
        let (mut app, me, here) = arena(6);
        let brenna = hired(&mut app, me, here.offset(2, 0), "Brenna");
        let potion = make(&mut app, UNKNOWN_MENDING);
        own(&mut app, brenna, potion);
        kill(&mut app, brenna, None);
        assert!(app.world().get::<Position>(potion).is_some(), "dropped where it died");
        assert!(app.world().get::<OwnKit>(potion).is_none(), "and nobody's own");
    }

    /// The look panel of a hired mercenary says it is in the adventurer's
    /// pay, in place of `a sword for hire`, beside its order, and what it
    /// holds as the adventurer knows it: its own sword at the level it
    /// told, and a potion of a kind they do not know by its face.
    #[test]
    fn the_look_panel_of_a_hired_mercenary_says_it_is_in_your_pay_and_lists_its_kit_an_unknown_potion_by_its_face() {
        let (mut app, me, here) = arena_with(5, InspectViewPlugin);
        app.add_systems(Update, (crate::bestiary_lore::annotate_inspect, crate::mercenaries::show_orders).chain().in_set(ViewSet::Annotate));
        let brenna = hired(&mut app, me, here.offset(-1, 0), "Brenna");
        let sword = told(&mut app, "sword", 1);
        app.world_mut().entity_mut(sword).insert(OwnKit);
        wear(&mut app, brenna, sword);
        let potion = make(&mut app, UNKNOWN_MENDING);
        own(&mut app, brenna, potion);
        settle(&mut app);
        let face = name_of(&app, potion);
        assert!(!face.contains("healing"), "a face, not the kind: {face}");
        let modal = rl_engine::rl_ui::view::inspect::inspect_modal(app.world().resource::<Modals>());
        app.world_mut().resource_mut::<Modals>().open(modal);
        app.world_mut().resource_mut::<InspectView>().cursor = at(&app, brenna);
        app.update();
        let look = app.world().resource::<InspectView>();
        let facets: Vec<&str> = look.facets.iter().map(|f| f.text.as_str()).collect();
        let carried = format!("carries a {face}");
        assert_eq!(facets, ["in your pay", "following", "wields a +1 sword", carried.as_str()], "the look");
    }
}
