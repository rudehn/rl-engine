//! What a hired mercenary carries and knows: its own kit, which it uses
//! freely, and anything else, which it uses only once the adventurer
//! knows it, so nothing it does tells the adventurer what they do not.

use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_core::noun::{Countability, counted_as};
use rl_engine::rl_rules::ai::brain::Tactic;
use rl_engine::rl_rules::equip;

use super::{GIVE, Mercenary, TONE};
use crate::armory::{Armory, ItemDef, ItemKind};
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
        seen_score(&self.armory, kind.map(|k| k.0), score.map(|s| s.0), !unknown || own, |name| self.lore.knows(name))
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
        let look = |worn: Entity| (self.score(worn).unwrap_or(0), self.blows.contains(worn));
        equip::gain(&worn.0, thing, &shape.0, |other| self.stuck.contains(other), score, look, self.own.get(wearer).map_or(0, |o| o.0))
    }
}

/// What a thing seems worth worn to the adventurer's eye: `score` when
/// `known` (it is known, or the wearer's own), the plain score of its kind
/// at level nought when only its level is unknown, and nothing when its
/// kind is not known to `knows`, since a ring of a kind nobody knows could
/// be any ring. Apart from the world so the rule is tested on the content
/// itself.
fn seen_score(armory: &Armory, kind: Option<Id<ItemDef>>, score: Option<i32>, known: bool, knows: impl Fn(&str) -> bool) -> Option<i32> {
    if known {
        return score;
    }
    let kind = kind?;
    knows(&armory.defs.get(kind).name).then(|| armory.score_at(kind, 0)).flatten()
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
///
/// When to arm, and with what, is Undermere's, by [`SeenWorth`]; the
/// putting on is the engine's `Decision::Equip`, so what is refused, worn
/// or narrated is the engine's rule for every wearer.
#[derive(Debug, Clone, Copy)]
pub struct PutOnBetter;

impl Tactic<Entity> for PutOnBetter {
    fn name(&self) -> &'static str {
        "arming"
    }

    fn evaluate(&self, ctx: &mut TacticCtx<'_, Entity>) -> Option<Decision<Entity>> {
        let better = ctx.snapshot.sense::<BetterInPack>()?;
        Some(Decision::Equip(better.0))
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
        let Ok((bag, _)) = self.bags.get(who) else { return Vec::new() };
        bag.items.iter().copied().filter(|t| self.why_not(who, *t).is_none()).collect()
    }

    /// Why `who` may not give `thing`, in the adventurer's words, or
    /// `None` when it is among what [`of`](Self::of) offers: the one rule,
    /// asked of the pack's choices and again of whatever the offer was
    /// taken up with, since an interaction need not have come from the
    /// pack.
    pub fn why_not(&self, who: Entity, thing: Entity) -> Option<&'static str> {
        let Ok((bag, worn)) = self.bags.get(who) else { return Some("You carry nothing.") };
        if !bag.contains(thing) {
            return Some("You do not carry that.");
        }
        if worn.is_some_and(|w| w.contains(thing)) {
            return Some("You are wearing that.");
        }
        let waystone = self.armory.defs.id(crate::waystone::WAYSTONE);
        let Ok((tagged, wearable, throwable, does, kind)) = self.things.get(thing) else { return Some(NO_USE) };
        let purse = self.currency.as_deref().is_some_and(|c| c.is(tagged));
        let usable = does.is_some_and(|d| d.on(Moments::USE).next().is_some() && d.asks().is_none());
        let fit = !purse && kind.is_none_or(|k| Some(k.0) != waystone) && (wearable || throwable || usable);
        (!fit).then_some(NO_USE)
    }
}

/// Why a thing a mercenary can neither wear, throw nor use, the purse or
/// the waystone, is not given.
const NO_USE: &str = "That is of no use to a hireling.";

/// The system naming what the adventurer may give a hired mercenary, by
/// [`Givable`], registered once by the plugin: what [`GIVE`] chooses
/// among, as the engine's choosing offer asks it.
#[derive(Resource, Debug, Clone, Copy)]
pub struct GiveAmong(pub Among);

/// What a hired mercenary's [`GIVE`] chooses among: what [`Givable`]
/// offers of the taker's pack. With nothing to give, says so, since the
/// pack then stays shut and nothing else would.
///
/// Run by the engine as the offer is taken up, before any time is spent,
/// so a pack with nothing in it to give costs nothing to find so.
pub fn give_choices(In(chooser): In<Chooser>, givable: Givable, hired: Query<&Mercenary>, mut tell: MessageWriter<Tell>) -> Vec<Entity> {
    let choices = givable.of(chooser.taker);
    if choices.is_empty()
        && let Ok(hired) = hired.get(chooser.offerer)
    {
        tell.write(Tell::new(format!("You have nothing to give {}.", hired.name), Tones::MUTED));
    }
    choices
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

/// Answers a hired mercenary's [`GIVE`], taken up with the thing the
/// pack chose and its step spent: moves the thing from the adventurer's
/// pack to the mercenary's and says so in the mercenaries' tone. Nothing
/// is taken back: what a mercenary carries comes back only when it drops
/// it.
///
/// The thing is asked again by [`Givable`]'s rule, since the engine checks
/// only that it is still carried and an interaction may come from
/// anywhere: worn gear, the purse or the waystone is never handed over,
/// and the log says why.
///
/// In `TurnSet::React`, beside hiring's own answer, so the mercenary
/// finds the thing in its pack on its next turn.
pub fn answer_give(
    mut done: MessageReader<Interacted>,
    verbs: Res<Verbs>,
    hired: Query<&Mercenary>,
    mut packs: ParamSet<(Givable, Handing)>,
    tones: Res<Tones>,
    turns: Res<Turns>,
    mut log: ResMut<MessageLog>,
) {
    let Some(give) = verbs.get(GIVE) else { return };
    for ev in done.read().filter(|ev| ev.verb == give) {
        let (Ok(hired), Some(thing)) = (hired.get(ev.target), ev.thing) else { continue };
        if let Some(why) = packs.p0().why_not(ev.actor, thing) {
            log.push(why.to_string(), Tones::MUTED, turns.turn_number());
            continue;
        }
        let Some(said) = packs.p1().hand(ev.actor, ev.target, thing) else { continue };
        log.push(format!("You give {} {said}.", hired.name), tones.get(TONE).unwrap_or(Tones::TEXT), turns.turn_number());
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::world::CommandQueue;
    use rl_engine::rl_ui::InspectViewPlugin;

    use super::*;
    use crate::mercenaries::fixtures::{hired, take_with};
    use crate::testing::{arena, arena_with, at, clear_floor, find, give, kill, lay, lay_floor, make, name_of, pass_turns, said, settle, wall_off, wear};

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
            crate::armory::spawn_item_at(&mut Commands::new(&mut queue, world), armory, armory.defs.expect(name), level)
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

    /// What `give` on `hired` offers `me` to choose among, asked as the
    /// engine asks it when the offer is taken up.
    fn choices(app: &mut App, me: Entity, hired: Entity) -> Vec<Entity> {
        let among = app.world().resource::<GiveAmong>().0;
        let verb = app.world().resource::<Verbs>().get(GIVE).expect("give");
        app.world_mut().run_system_with(among, Chooser { taker: me, offerer: hired, verb }).expect("the choices run")
    }

    /// A mercenary drinks its own potion when hurt whether or not the
    /// adventurer knows the kind; one drunk where they see it teaches them
    /// the kind as their own drinking would, and one drunk behind a wall
    /// teaches them nothing, and the log names nothing. One run plays the
    /// three cases in turn, the walled-off one first, before anything has
    /// taught the kind; the town is staged, so no seed changes the play.
    #[test]
    fn it_drinks_its_own_potion_when_hurt_unknown_or_not_and_the_adventurer_learns_the_kind_in_sight_and_not_out_of_it() {
        let (mut app, me, here) = arena(0);
        assert!(!knows(&app, UNKNOWN_MENDING) && knows(&app, "potion of healing"), "only the healing is set out with");
        let ring: Vec<Point> = (0..8).map(|i| here + Direction::from_index(i).offset()).collect();
        wall_off(&mut app, &ring);
        let behind_a_wall = hired(&mut app, me, here.offset(2, 0), "Brenna");
        let potion = make(&mut app, UNKNOWN_MENDING);
        own(&mut app, behind_a_wall, potion);
        hurt(&mut app, behind_a_wall);
        pass_turns(&mut app, 2);
        assert!(!carries(&app, behind_a_wall, potion), "drunk out of sight");
        assert!(!knows(&app, UNKNOWN_MENDING) && said(&app).iter().all(|line| !line.contains("That was a")), "and nothing learned: {:?}", said(&app));

        lay_floor(&mut app, &ring);
        let seen = hired(&mut app, me, here.offset(2, 1), "Tolvar");
        let unknown = make(&mut app, UNKNOWN_MENDING);
        own(&mut app, seen, unknown);
        hurt(&mut app, seen);
        let healing = hired(&mut app, me, here.offset(2, -1), "Corwin");
        let known = make(&mut app, "potion of healing");
        own(&mut app, healing, known);
        hurt(&mut app, healing);
        pass_turns(&mut app, 2);
        assert!(!carries(&app, seen, unknown) && !carries(&app, healing, known), "both drunk in sight");
        let told = |kind: &str| said(&app).iter().any(|line| line.contains(&format!("That was a {kind}.")));
        assert!(knows(&app, UNKNOWN_MENDING) && told(UNKNOWN_MENDING), "the kind drunk in sight is learned and told: {:?}", said(&app));
        assert!(!told("potion of healing"), "a kind already known is told of nothing: {:?}", said(&app));
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
    /// mace two levels better than the one it wields, found, its level
    /// unknown, counts as a plain one and is passed over, so nothing it
    /// does says it is better; the same mace told is fetched and put on
    /// in its place. A mace, since it fills the one hand, where a second
    /// sword would go to the free one beside the first. One run plays the
    /// unknown one first and the told one after; the staging is the same
    /// whatever the seed.
    #[test]
    fn it_puts_on_a_better_weapon_lying_in_sight_and_passes_over_an_unknown_one_counted_at_plain_numbers() {
        let (mut app, me, here) = arena(0);
        clear_floor(&mut app);
        let brenna = hired(&mut app, me, here.offset(1, 0), "Brenna");
        let mine = told(&mut app, "mace", 1);
        app.world_mut().entity_mut(mine).insert(OwnKit);
        wear(&mut app, brenna, mine);
        let unknown = find(&mut app, "mace", 3);
        lay(&mut app, unknown, here.offset(2, 1));
        settle(&mut app);
        pass_turns(&mut app, 6);
        assert!(wears(&app, brenna, mine) && !wears(&app, brenna, unknown), "an unknown one is passed over");
        assert!(app.world().get::<Position>(unknown).is_some(), "and still lies there");

        let better = told(&mut app, "mace", 3);
        lay(&mut app, better, here.offset(2, -1));
        settle(&mut app);
        pass_turns(&mut app, 6);
        assert!(wears(&app, brenna, better) && !wears(&app, brenna, mine), "a told one is put on in its place");
        assert!(app.world().get::<Position>(better).is_none(), "and no longer lies there");
        assert!(app.world().get::<Position>(unknown).is_some(), "while the unknown one lies untouched");
    }

    /// What a thing of unknown level seems worth to a mercenary, read off
    /// the content itself: its true score when known or its own, its
    /// kind's plain score at level nought when only the level is unknown,
    /// nothing when the kind is unknown too or wears for nothing.
    #[test]
    fn a_thing_of_unknown_level_seems_worth_its_kinds_plain_score_and_a_thing_of_unknown_kind_nothing() {
        let library = crate::content::Library::load(&crate::content::effect_kinds(), &Moments::default());
        let armory = &library.armory;
        let (mace, potion) = (armory.defs.expect("mace"), armory.defs.expect("potion of healing"));
        let (plain, deep) = (armory.score_at(mace, 0).expect("a mace is worth wearing"), armory.score_at(mace, 3).expect("and at depth"));
        assert!(deep > plain, "a level makes it worth more, {plain} to {deep}");
        let name = armory.defs.get(mace).name.clone();
        assert_eq!(seen_score(armory, Some(mace), Some(deep), true, |_| false), Some(deep), "known or its own: the true score, even of an unknown kind");
        assert_eq!(seen_score(armory, Some(mace), Some(deep), false, |n| n == name), Some(plain), "known kind, unknown level: the plain score");
        assert_eq!(seen_score(armory, Some(mace), Some(deep), false, |_| false), None, "unknown kind: nothing");
        assert_eq!(seen_score(armory, None, Some(deep), false, |_| true), None, "no kind: nothing");
        assert_eq!(seen_score(armory, Some(potion), None, false, |_| true), None, "a thing nobody wears is worth nothing worn");
        assert_eq!(seen_score(armory, Some(potion), None, true, |_| true), None, "and known has no score to give");
    }

    /// Each pass's actor, and whether Brenna carries and wears the sword
    /// once the pass has answered what it caused.
    #[derive(Resource, Default)]
    struct Passes {
        sword: Option<(Entity, Entity)>,
        seen: Vec<(Entity, bool, bool)>,
    }

    /// `give` chooses among what the adventurer carries and does not
    /// wear; taken up with a sword, it spends the adventurer's turn and
    /// moves the sword to the mercenary's pack with a line saying so, not
    /// worn at the hand-over, and the sword, better than its fists, is worn
    /// on the mercenary's next turn.
    #[test]
    fn a_sword_given_takes_a_turn_and_is_worn_on_the_mercenarys_next_turn() {
        let (mut app, me, here) = arena(3);
        let brenna = hired(&mut app, me, here.offset(2, 0), "Brenna");
        let sword = told(&mut app, "sword", 0);
        give(&mut app, me, sword);
        let offered = choices(&mut app, me, brenna);
        let worn = app.world().get::<Equipped>(me).expect("slots").clone();
        assert!(offered.contains(&sword) && offered.iter().all(|t| !worn.contains(*t)), "what is carried and not worn: {offered:?}");
        app.insert_resource(Passes { sword: Some((brenna, sword)), seen: Vec::new() });
        app.add_systems(
            Turn,
            (|mut passes: ResMut<Passes>, acting: Query<Entity, With<MyTurn>>, bags: Query<&Inventory>, worn: Query<&Equipped>| {
                let Some((brenna, sword)) = passes.sword else { return };
                let carried = bags.get(brenna).is_ok_and(|b| b.contains(sword));
                let on = worn.get(brenna).is_ok_and(|w| w.contains(sword));
                for actor in &acting {
                    passes.seen.push((actor, carried, on));
                }
            })
            .in_set(TurnSet::React)
            .after(answer_give),
        );
        let before = app.world().resource::<Turns>().now();
        take_with(&mut app, me, brenna, GIVE, Some(sword));
        assert!(app.world().resource::<Turns>().now() > before, "for a turn");
        assert!(!carries(&app, me, sword), "handed over");
        assert!(said(&app).contains(&"You give Brenna a sword.".to_string()), "{:?}", said(&app));
        let seen = app.world().resource::<Passes>().seen.clone();
        let given = seen.iter().position(|(who, carried, _)| *who == me && *carried).expect("handed over on the adventurer's pass");
        assert!(!seen[given].2, "not worn at the hand-over: {seen:?}");
        let next = seen[given + 1..].iter().find(|(who, ..)| *who == brenna).expect("the mercenary's next turn");
        assert!(next.2, "worn on its next turn: {seen:?}");
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
        let offered = choices(&mut app, me, brenna);
        assert!(offered.contains(&potions) && offered.contains(&darts), "what it can drink and throw: {offered:?}");
        assert!(!offered.contains(&stone), "but never the way home: {offered:?}");

        take_with(&mut app, me, brenna, GIVE, Some(potions));
        let count = |app: &App, thing: Entity| app.world().get::<Stack>(thing).map(|s| s.count);
        assert!(carries(&app, me, potions) && count(&app, potions) == Some(2), "two kept");
        let theirs = app.world().get::<Inventory>(brenna).expect("a pack").items.iter().copied().find(|t| name_of(&app, *t) == name_of(&app, potions));
        assert_eq!(theirs.map(|t| count(&app, t)), Some(Some(1)), "and one of its own handed over");
        assert!(said(&app).iter().any(|l| l.starts_with("You give Brenna a potion")), "{:?}", said(&app));

        take_with(&mut app, me, brenna, GIVE, Some(darts));
        assert!(carries(&app, brenna, darts) && !carries(&app, me, darts) && count(&app, darts) == Some(5), "the darts whole");
        assert!(said(&app).contains(&"You give Brenna 5 darts.".to_string()), "{:?}", said(&app));
    }

    /// An interaction for `give` written by anything but the pack, with
    /// worn gear or the waystone, hands nothing over and says why: the
    /// mercenary's answer asks the pack's own rule again.
    #[test]
    fn give_taken_up_with_worn_gear_or_the_waystone_hands_nothing_over_and_says_why() {
        let (mut app, me, here) = arena(3);
        let brenna = hired(&mut app, me, here.offset(2, 0), "Brenna");
        let (worn, stone) = {
            let world = app.world();
            let id = world.resource::<Armory>().defs.expect(crate::waystone::WAYSTONE);
            let bag = &world.get::<Inventory>(me).expect("a pack").items;
            let slots = world.get::<Equipped>(me).expect("slots");
            let worn = *bag.iter().find(|i| slots.contains(**i)).expect("something worn");
            let stone = *bag.iter().find(|i| world.get::<ItemKind>(**i).is_some_and(|k| k.0 == id)).expect("a waystone in the pack");
            (worn, stone)
        };
        for (thing, why) in [(worn, "You are wearing that."), (stone, NO_USE)] {
            take_with(&mut app, me, brenna, GIVE, Some(thing));
            assert!(carries(&app, me, thing) && !carries(&app, brenna, thing), "{why}: kept");
            assert!(said(&app).contains(&why.to_string()), "{why}: said {:?}", said(&app));
        }
        assert!(wears(&app, me, worn), "and still worn");
        assert!(!said(&app).iter().any(|l| l.starts_with("You give Brenna")), "nothing given: {:?}", said(&app));
    }

    /// With nothing in the pack to give, `give` chooses among nothing, so
    /// the pack stays shut, and says so.
    #[test]
    fn give_with_nothing_to_give_offers_nothing_and_says_so() {
        let (mut app, me, here) = arena(3);
        let brenna = hired(&mut app, me, here.offset(2, 0), "Brenna");
        let keep: Vec<Entity> = {
            let world = app.world();
            let id = world.resource::<Armory>().defs.expect(crate::waystone::WAYSTONE);
            let bag = &world.get::<Inventory>(me).expect("a pack").items;
            bag.iter().copied().filter(|i| world.get::<ItemKind>(*i).is_some_and(|k| k.0 == id)).collect()
        };
        app.world_mut().get_mut::<Inventory>(me).expect("a pack").items = keep;
        assert!(choices(&mut app, me, brenna).is_empty(), "nothing but the way home");
        app.update();
        assert!(said(&app).contains(&"You have nothing to give Brenna.".to_string()), "{:?}", said(&app));
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
