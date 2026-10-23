//! The narrator: what the turns did, in words, in the log.
//!
//! Every game with a log writes the same system: read [`DamageDealt`] and
//! [`DeathEvent`], branch on whether the player did it or had it done to
//! it, pick a tone, push a line. Ten copies of that, each getting the order
//! wrong when two monsters act in one frame and each narrating blows nobody
//! saw. So it is a view, a collector and a presenter, the split every panel
//! has, with one twist in where the collector runs.
//!
//! The view, [`NarrationView`], is rows of [`Said`]: its [`Words`], an
//! engine [`Phrase`] or a game's own line, who did it, to whom, with what,
//! how much, where, and whether the player saw it. No string the game did not supply. The collector runs in
//! [`TurnSet::Record`], once per pass rather than
//! once per frame, because one pass is one actor's action and reading that
//! pass's events in a fixed order gives the true order across a frame of
//! many turns: the look round, then the cast, then the blow it landed, then
//! the next actor's step. A collector in the drawing phase sees a whole
//! frame's buffers at once and cannot know which blow followed which cast.
//!
//! The presenter, [`NarratorPlugin`], turns each row into a line through
//! the [`Phrasebook`]: one template per phrase, split by perspective so
//! "you hit" and "hits you" are two entries with two tones and no grammar
//! lives in the engine. A name in a template wears the colour of the thing
//! it names, made readable by the panel that draws it. A game changes any
//! phrase, silences any, narrates the unseen or not, edits or removes rows
//! in [`ViewSet::Annotate`](crate::ViewSet) before they are spoken, or
//! drops the presenter and reads the view itself.
//!
//! A thing is named as its template asks: `{what}` as any thing, `a
//! sword`, and `{your what}` as the player's own, `your sword`, which is
//! how the player's putting on, taking off and dropping read. A thing whose
//! name takes no `a`, `chain mail` or `gauntlets`, carries [`MassNoun`],
//! put on it by the game, since only the game knows which of its names are
//! a stuff. A proper name, one written with a capital or with `the` and a
//! capital, is said as written either way, `the Widow's Fang`, by
//! [`atglyph_core::noun`]'s one rule.
//!
//! Nothing the player cannot see is named. Whether it can is asked of
//! [`PlayerSight::shows`], the question the map and every list of what is
//! in sight ask, so a thing [`Concealed`] in the dark, its cell in sight
//! but itself not, is `something` in the log as it is nothing on the map,
//! whichever side of the line it stands on: "Something hits you." and
//! "You shoot something."
//!
//! An ability is named once. A use that harmed someone is told by the harm
//! line alone, "Your bolt hits the rat for 3.", which names the bolt and
//! says what it did; "You use bolt on the rat." before it said the bolt
//! twice. A use that harmed nothing keeps its own line.
//!
//! What using an item means, what a quest said, and how a game's own events
//! read stay the game's words, but not its order to keep. A game writes a
//! [`Tell`] from inside the pass, in [`TurnSet::React`] where it answers
//! what the pass did, and the collector reads it with that pass's events,
//! after them, as a row whose [`Words`] are the game's own. A line pushed
//! straight to the [`MessageLog`] from inside a pass lands ahead of every
//! row the frame has yet to speak, so a game's answer to a sighting
//! reads above the sighting itself. A line from outside the turns, the one a run opens
//! with or a key refused before any turn is spent, has no pass to wait
//! for and still goes to the log directly.

use std::collections::BTreeMap;

use atglyph_bevy::prelude::*;
use atglyph_core::Point;
use atglyph_core::noun::Countability;
use atglyph_render::Glyph;
use bevy::prelude::*;

use crate::log::{MessageLog, Span};
use crate::tone::{ToneId, Tones};

/// What can be said. Closed, because it enumerates the events the engine
/// itself raises; a game's own events are the game's to narrate.
///
/// Split by perspective, so each entry is a template with no grammar in it
/// and a tone of its own: what you did, what was done to you, and what
/// happened between others.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Phrase {
    /// You hit someone for something.
    YouHit,
    /// You hit someone for nothing.
    YouHitNothing,
    /// Someone hit you for something.
    HitsYou,
    /// Someone hit you for nothing.
    HitsYouForNothing,
    /// Someone hit someone else for something.
    OthersFight,
    /// Someone hit someone else for nothing.
    OthersHitNothing,
    /// You shot someone for something.
    YouShoot,
    /// You shot someone for nothing.
    YouShootNothing,
    /// Someone shot you for something.
    ShootsYou,
    /// Someone shot you for nothing.
    ShootsYouForNothing,
    /// Someone shot someone else for something.
    OthersShoot,
    /// Someone shot someone else for nothing.
    OthersShootNothing,
    /// Something you used or an ability of yours hurt someone for
    /// something: a rod's bolt, a burst. Named, since it was no blow.
    YourUseHits,
    /// The same, for nothing.
    YourUseHitsNothing,
    /// Something someone used, or their ability, hurt you for something.
    UseHitsYou,
    /// The same, for nothing.
    UseHitsYouForNothing,
    /// Something someone used, or their ability, hurt someone else for
    /// something.
    OthersUseHits,
    /// The same, for nothing.
    OthersUseHitsNothing,
    /// You attacked someone and missed.
    YouMiss,
    /// Someone attacked you and missed.
    MissesYou,
    /// Someone attacked someone else and missed.
    OthersMiss,
    /// A reflecting body turned your own aim back onto you.
    ReflectsOntoYou,
    /// You turned someone else's aim back onto them.
    YouReflect,
    /// A reflecting body turned someone else's aim back onto them.
    OthersReflect,
    /// You hurt yourself.
    YouHurtYourself,
    /// You were mended.
    YouMend,
    /// Someone else was mended.
    Mends,
    /// A status on you dealt its damage.
    YouTakeFromStatus,
    /// A status on someone else dealt its damage.
    TakesFromStatus,
    /// You killed someone.
    YouKill,
    /// You broke a prop: a crate, a barrel, a lamp. Nothing is killed
    /// that was never alive.
    YouDestroy,
    /// Somebody else broke one.
    Destroys,
    /// One broke with nobody to credit.
    IsDestroyed,
    /// Someone killed someone else.
    Kills,
    /// Someone died of no one in particular.
    Dies,
    /// One on the player's side died of no one in particular: [`Dies`](Phrase::Dies)
    /// for one the game's [`CombatRules`] say is allied to the player,
    /// which is a loss rather than good news.
    AllyDies,
    /// Someone killed one on the player's side.
    KillsAlly,
    /// You killed one on your own side.
    YouKillAlly,
    /// You died.
    YouDie,
    /// You picked something up.
    YouPickUp,
    /// Someone else did.
    PicksUp,
    /// You dropped something.
    YouDrop,
    /// Someone else did.
    Drops,
    /// You put something on that is not a weapon.
    YouWear,
    /// Someone else did.
    Wears,
    /// You took up something that strikes or shoots.
    YouWield,
    /// Someone else did.
    Wields,
    /// You took something off.
    YouTakeOff,
    /// Someone else did.
    TakesOff,
    /// Something you wear would not come off: you tried to take it off,
    /// drop or throw it, or to put on something that would have pushed it
    /// off.
    WillNotComeOff,
    /// You threw something.
    YouThrow,
    /// Someone else did.
    Throws,
    /// You used a thing that does something when used: drank it, lit it.
    YouUseThing,
    /// Someone else did.
    UsesThing,
    /// You opened a door.
    YouOpen,
    /// You closed one.
    YouClose,
    /// Someone else opened one.
    OpensDoor,
    /// Someone else closed one.
    ClosesDoor,
    /// Someone you could see left for another map.
    Leaves,
    /// Someone came through to another map after you.
    FollowsYou,
    /// You walked into someone you would not strike, and who offers you
    /// nothing; one who offers something is asked about on a screen.
    YouBumpInto,
    /// You changed places with someone.
    YouSwapWith,
    /// Someone noticed you.
    NoticesYou,
    /// You used an ability on nobody in particular, or aimed it at the
    /// ground and caught one there.
    YouUse,
    /// You used one on someone.
    YouUseOn,
    /// You used one on several.
    YouUseOnMany,
    /// Someone else used one on nobody in particular, or aimed it at the
    /// ground and caught one there.
    Uses,
    /// Someone else used one on someone.
    UsesOn,
    /// Someone else used one on several.
    UsesOnMany,
    /// You tried an ability and were refused.
    YouCannotUse,
    /// A status was put on you.
    YouAreAfflicted,
    /// A status that is a boon was put on you: the same words as
    /// [`Phrase::YouAreAfflicted`], told as good news.
    YouGainABoon,
    /// A status on you ran out, or a boon on you was broken off.
    YouAreNoLonger,
    /// A status on you that is not a boon was cured: the same words as
    /// [`Phrase::YouAreNoLonger`], told as good news.
    YourAfflictionPasses,
    /// A status was put on someone else.
    IsAfflicted,
    /// Your light went out.
    YourLightGoesOut,
    /// Something else's light went out.
    LightGoesOut,
    /// You stood in fire.
    YouStandInFire,
    /// Someone else did.
    StandsInFire,
}

/// What a row says: one of the engine's phrases, worded by the
/// [`Phrasebook`], or a game's own words from a [`Tell`].
///
/// Two kinds rather than a phrase a game adds to, because [`Phrase`]
/// enumerates what the engine raises and nothing else; a game's line
/// arrives already worded and only needs its place in the order.
#[derive(Debug, Clone, PartialEq)]
pub enum Words {
    /// An engine event, said as the phrasebook says it.
    Phrase(Phrase),
    /// A game's template, said in its own tone.
    Own {
        /// The template, with the phrasebook's placeholders.
        text: String,
        /// The tone.
        tone: ToneId,
    },
}

/// One thing that happened, as the narrator reads it.
#[derive(Debug, Clone, PartialEq)]
pub struct Said {
    /// What is said of it.
    pub words: Words,
    /// Who did it, if anyone: `{who}` in a template.
    pub who: Option<Entity>,
    /// To whom, if anyone: `{whom}`.
    pub whom: Option<Entity>,
    /// With what, if anything: `{what}`, named as the thing is named.
    pub what: Option<Entity>,
    /// A registry's name for the status, kind or ability involved:
    /// `{named}`.
    pub named: String,
    /// What `who` and `whom` were called when the row was made, filled in
    /// by the collector, and [`called_thing`](Self::called_thing) the same
    /// for `what`.
    ///
    /// A row is made inside the pass and spoken after it, and things
    /// change in between: what dies becomes remains and is renamed, what
    /// is thrown merges into a stack, what is picked up leaves the floor.
    /// A line about a blow that landed on a coolant rat should say so,
    /// not say it landed on the coolant rat's remains, so the names are
    /// read when the thing happened rather than when the line is spoken.
    /// Empty falls back to reading them now, which is what a line made
    /// outside the turns wants.
    pub called: [Option<String>; 2],
    /// What `what` was called when the row was made, and how many of it
    /// the row is about, read as [`called`](Self::called) is.
    pub called_thing: Option<Called>,
    /// More words, where there are any: the reasons a use was refused.
    pub detail: String,
    /// How much: `{n}`.
    pub amount: i32,
    /// Where, when it happened somewhere.
    pub at: Option<Point>,
    /// Whether the player saw it: it happened to the player, or in the
    /// player's sight.
    ///
    /// Whether the line is spoken at all. Being shot from the dark is
    /// always worth telling you, so this is true whenever the player is
    /// one of the two; [`who_seen`](Self::who_seen) is the narrower
    /// question of whether the doer may be named.
    pub seen: bool,
    /// Whether the player could see whoever did it.
    ///
    /// A fact, not a decision: whether the player saw the doer itself, by
    /// [`PlayerSight::shows`], the question the map and every list of what
    /// is in sight ask, so the log never names a thing the map left out:
    /// one out of sight, and one [`Concealed`] in the dark though its cell
    /// is in sight. `speak` is what turns it into `something`, and only
    /// while [`Phrasebook::speak_unseen`] is off, since a game that asked
    /// for the unseen to be narrated asked for it named.
    ///
    /// Without this a shot out of an unlit room read as "The line droid
    /// shoots you", naming a thing the player had never seen, because
    /// `seen` answered yes on the grounds that the player was the one
    /// being shot. True by default, for a row with no doer to hide.
    pub who_seen: bool,
    /// Whether the player could see whoever it was done to: the same fact
    /// as [`who_seen`](Self::who_seen), for `{whom}`.
    ///
    /// Its own question, because a line is spoken for what the player saw
    /// of either one: a shot the player loosed into the dark is told,
    /// and what it struck there is `something` as surely as a shooter in
    /// the dark is. True by default, for a row with nobody to hide.
    pub whom_seen: bool,
    /// The whole turn it happened on.
    pub turn: u32,
}

impl Said {
    /// A row with nothing but its phrase and turn.
    pub fn new(phrase: Phrase, turn: u32) -> Self {
        Self::worded(Words::Phrase(phrase), turn)
    }

    /// The row a game's [`Tell`] makes, on `turn`.
    pub(crate) fn told(tell: &Tell, turn: u32) -> Self {
        let mut said = Self::worded(Words::Own { text: tell.text.clone(), tone: tell.tone }, turn);
        said.who = tell.who;
        said.whom = tell.whom;
        said.what = tell.what;
        said
    }

    fn worded(words: Words, turn: u32) -> Self {
        Self {
            words,
            who: None,
            whom: None,
            what: None,
            named: String::new(),
            called: [None, None],
            called_thing: None,
            detail: String::new(),
            amount: 0,
            at: None,
            seen: true,
            who_seen: true,
            whom_seen: true,
            turn,
        }
    }

    /// The engine phrase, when the row is one.
    pub(crate) fn phrase(&self) -> Option<Phrase> {
        match self.words {
            Words::Phrase(phrase) => Some(phrase),
            Words::Own { .. } => None,
        }
    }
}

/// What a thing was called when a row was made: its name, how many of it
/// the row is about, and whether the name is said with `a`, which the
/// presenter puts together as the template asks, `a sword` or `your
/// sword`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Called {
    /// Its [`Name`], one of it.
    pub name: String,
    /// How many: what its [`Stack`] held, or one.
    pub count: u32,
    /// [`Countability::Mass`] for a thing carrying [`MassNoun`].
    pub countability: Countability,
}

impl Called {
    /// As a sentence says it: `a sword`, `5 arrows`, `chain mail`.
    pub(crate) fn counted(&self) -> String {
        atglyph_core::noun::counted_as(&self.name, self.count, self.countability)
    }

    /// As the player's own: `your sword`, `your 5 arrows`.
    pub(crate) fn yours(&self) -> String {
        atglyph_core::noun::owned("your", &self.name, self.count, self.countability)
    }
}

/// What `name` is called, `count` of it, said with no `a` when `mass`.
fn called(name: &Name, count: u32, mass: bool) -> Called {
    let countability = if mass { Countability::Mass } else { Countability::Count };
    Called { name: name.as_str().to_string(), count, countability }
}

/// A thing whose name is said with no `a`: a stuff, `chain mail` or
/// `leather armor`, or a thing named in the plural, `gauntlets`. So a line
/// reads `You pick up chain mail.` and not `You pick up a chain mail.`
///
/// On the thing rather than a list of words the engine keeps, because
/// whether a name is a stuff is a fact about the game's content that its
/// spelling does not give away, `mail` being one and `nail` not, and the
/// game that named the thing is the one that knows. It is about the name
/// the thing carries, so a game that renames a thing to a name that is not
/// a stuff takes it off.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MassNoun;

/// A line of a game's own, told in its place among what the turns did.
///
/// Written from inside a pass, usually in [`TurnSet::React`] as the
/// game's answer to what the pass did: what a sighting sets off, what an
/// attack costs the thing it was made with. The collector reads it with the
/// pass's own events, after them, and the presenter speaks it with them,
/// so it lands in the log below what it answers and above whatever the
/// next actor does. `text` is a template with the [`Phrasebook`]'s
/// placeholders, so a name in it wears its colour as an engine line's
/// does, and a line with no braces is spoken as written.
///
/// Always spoken: the game chose to say it, so whether the player saw who
/// it names is the game's to have weighed. A line written outside the
/// turns, such as the one a run opens with, is read on the next pass, so
/// it goes to the [`MessageLog`] directly instead.
#[derive(Message, Debug, Clone, PartialEq)]
pub struct Tell {
    /// What to say, with placeholders as a phrase has.
    pub text: String,
    /// The tone it is said in.
    pub tone: ToneId,
    /// `{who}`, if anyone.
    pub who: Option<Entity>,
    /// `{whom}`, if anyone.
    pub whom: Option<Entity>,
    /// `{what}`, if anything.
    pub what: Option<Entity>,
}

impl Tell {
    /// `text`, said in `tone`, naming nobody.
    pub fn new(text: impl Into<String>, tone: ToneId) -> Self {
        Self { text: text.into(), tone, who: None, whom: None, what: None }
    }

    /// Names `who`, for `{who}`.
    pub fn by(mut self, who: Entity) -> Self {
        self.who = Some(who);
        self
    }

    /// Names `whom`, for `{whom}`.
    pub fn to(mut self, whom: Entity) -> Self {
        self.whom = Some(whom);
        self
    }

    /// Names `what`, for `{what}`.
    pub fn about(mut self, what: Entity) -> Self {
        self.what = Some(what);
        self
    }
}

/// What the turns did this frame, in the order they did it.
#[derive(Resource, Debug, Default)]
pub struct NarrationView {
    /// The rows, oldest first. Spoken and cleared once a frame.
    pub rows: Vec<Said>,
}

/// Keeps [`NarrationView`] current: one collector in the turn's reaction
/// phase, reading every event the engine raises.
///
/// Registers every message it reads, so a game without the plugin that
/// raises one still has an empty buffer to read.
pub struct NarrationViewPlugin;

impl Plugin for NarrationViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NarrationView>()
            .add_message::<DamageDealt>()
            .add_message::<Healed>()
            .add_message::<Missed>()
            .add_message::<DeathEvent>()
            .add_message::<ItemEvent>()
            .add_message::<DoorEvent>()
            .add_message::<Bumped>()
            .add_message::<Swapped>()
            .add_message::<StatusEvent>()
            .add_message::<Noticed>()
            .add_message::<LightEvent>()
            .add_message::<FireEvent>()
            .add_message::<AbilityEvent>()
            .add_message::<Reflected>()
            .add_message::<Travelled>()
            .add_message::<Tell>()
            .add_systems(Turn, collect_narration.in_set(TurnSet::Record));
    }

    fn finish(&self, app: &mut App) {
        atglyph_bevy::depends_on::<crate::UiPlugin>(app, "NarrationViewPlugin");
    }
}

/// Every event the narrator reads, in the order it reads them within a
/// pass: a use before the blows it landed, blows before the deaths they
/// caused, and a game's own lines last, since they answer what the pass
/// did.
#[derive(bevy::ecs::system::SystemParam)]
pub struct NarratedEvents<'w, 's> {
    abilities: MessageReader<'w, 's, AbilityEvent>,
    bumps: MessageReader<'w, 's, Bumped>,
    swaps: MessageReader<'w, 's, Swapped>,
    doors: MessageReader<'w, 's, DoorEvent>,
    travels: MessageReader<'w, 's, Travelled>,
    items: MessageReader<'w, 's, ItemEvent>,
    reflected: MessageReader<'w, 's, Reflected>,
    dealt: MessageReader<'w, 's, DamageDealt>,
    healed: MessageReader<'w, 's, Healed>,
    missed: MessageReader<'w, 's, Missed>,
    statuses: MessageReader<'w, 's, StatusEvent>,
    deaths: MessageReader<'w, 's, DeathEvent>,
    noticed: MessageReader<'w, 's, Noticed>,
    lights: MessageReader<'w, 's, LightEvent>,
    fires: MessageReader<'w, 's, FireEvent>,
    tells: MessageReader<'w, 's, Tell>,
}

/// Items that strike or shoot when wielded, which are wielded rather than
/// worn.
type Weapons<'w, 's> = Query<'w, 's, (), (With<Item>, Or<(With<MeleeAttack>, With<RangedAttack>)>)>;

/// What the collector reads about the world to fill a row.
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct Witness<'w, 's> {
    turns: Res<'w, Turns>,
    registries: Option<Res<'w, Registries>>,
    abilities: Option<Res<'w, Abilities>>,
    /// What the player sees, asked as the map and every view ask it.
    sight: PlayerSight<'w, 's>,
    /// The map being played, which a thing that stands nowhere is judged
    /// on.
    map: Res<'w, WorldMap>,
    positions: Query<'w, 's, &'static Position>,
    /// Which map a thing is on, and whether it hides in the dark.
    looks: Query<'w, 's, (Option<&'static OnMap>, Has<Concealed>)>,
    /// Where each remnant's moment happened, for a remnant that did
    /// something in its own name.
    remnants: Query<'w, 's, &'static atglyph_bevy::Remnant>,
    weapons: Weapons<'w, 's>,
    holding: Query<'w, 's, (), With<MyTurn>>,
    props: Query<'w, 's, (), With<atglyph_bevy::Prop>>,
    /// What a thing does at its moments, to tell a use the engine landed
    /// from one the game answers itself.
    does: Query<'w, 's, &'static atglyph_bevy::Triggers>,
    /// What things are called while the pass is still running.
    names: Query<'w, 's, (Option<&'static Name>, Option<&'static Stack>, Has<MassNoun>)>,
    /// Who stands with whom, for a death on the player's side; absent in
    /// a game without combat, where nobody is anybody's ally.
    rules: Option<Res<'w, CombatRules>>,
    /// Each side, the dead's among them until they are buried at the end
    /// of the frame.
    sides: Query<'w, 's, &'static Faction>,
}

impl Witness<'_, '_> {
    fn is_you(&self, e: Entity) -> bool {
        self.sight.viewer().is_some_and(|(me, _, _)| me == e)
    }

    /// Whether using `e` did something the engine landed: it carries a
    /// `use` trigger. A thing without one is the game's to answer, in its
    /// own words, since the engine cannot know what eating a crust means.
    fn used_to_effect(&self, e: Entity) -> bool {
        self.does.get(e).is_ok_and(|t| t.on(atglyph_bevy::TriggerKinds::USE).next().is_some())
    }

    /// Whether `e` is on the player's side, as the game's [`CombatRules`]
    /// say: false with no rules, and for anyone without a side.
    fn allied_to_you(&self, e: Entity) -> bool {
        let (Some(rules), Some((me, ..))) = (self.rules.as_deref(), self.sight.viewer()) else { return false };
        let (Ok(mine), Ok(theirs)) = (self.sides.get(me), self.sides.get(e)) else { return false };
        rules.factions.relation(mine.0, theirs.0) == atglyph_rules::Relation::Allied
    }

    /// Whether `e` is a prop rather than something that was alive: a
    /// crate, a lamp, a barrel. What breaks is destroyed, not killed.
    fn is_a_prop(&self, e: Entity) -> bool {
        self.props.contains(e)
    }

    /// Whether `e` holds this pass's turn: the one that looked round and
    /// then acted.
    fn holds_the_turn(&self, e: Entity) -> bool {
        self.holding.contains(e)
    }

    /// Where `e` stands, or for a remnant, which stands nowhere, where its
    /// moment happened: a dead actor's last burst is seen, and its doer
    /// named, where the actor fell.
    fn at(&self, e: Entity) -> Option<Point> {
        self.positions.get(e).ok().map(|p| p.0).or_else(|| self.remnants.get(e).ok().map(|r| r.at))
    }

    /// Whether the player's sight reaches `at`, and nothing else: no
    /// allowance for having been the one it happened to.
    fn sees(&self, at: Option<Point>) -> bool {
        let (Some((_, from, sight)), Some(p)) = (self.sight.viewer(), at) else { return false };
        p == from || sight.can_see(p)
    }

    /// Whether the player saw `at`, or was one of `these`.
    fn seen(&self, at: Option<Point>, these: &[Option<Entity>]) -> bool {
        these.iter().flatten().any(|e| self.is_you(*e)) || self.sees(at)
    }

    /// Whether the player sees `e` itself, which is whether a line may
    /// name it: by [`PlayerSight::shows`], so the log names nothing the
    /// map does not draw. A cell in sight is not enough, since a thing
    /// [`Concealed`] in the dark stands in one unseen. The player always
    /// sees itself.
    ///
    /// A remnant stands nowhere, and is seen or not where its moment
    /// happened, as the actor it stands for would have been there.
    fn shows(&self, e: Entity) -> bool {
        if self.is_you(e) {
            return true;
        }
        match self.positions.get(e) {
            Ok(at) => {
                let (on, concealed) = self.looks.get(e).unwrap_or((None, false));
                self.sight.shows(at.0, on, concealed)
            }
            Err(_) => self.remnants.get(e).is_ok_and(|r| self.shows_at(e, r.at)),
        }
    }

    /// Whether the player would see `e` at `at` on the map being played:
    /// where a remnant's moment happened, or the cell one that has gone to
    /// another map left by, which is the last place the player could have
    /// seen it.
    fn shows_at(&self, e: Entity, at: Point) -> bool {
        let concealed = self.looks.get(e).is_ok_and(|(_, concealed)| concealed);
        self.sight.shows(at, Some(&OnMap(self.map.current())), concealed)
    }

    /// What `e` is called right now as a line names an actor, in the
    /// words [`Names::actor`] would use: `None` for the player, who is
    /// always "you", and for anything unnamed.
    fn called_actor(&self, e: Option<Entity>) -> Option<String> {
        let e = e?;
        if self.is_you(e) {
            return None;
        }
        let (name, ..) = self.names.get(e).ok()?;
        Some(atglyph_core::noun::with_the(name?.as_str()))
    }

    /// What a thing is called right now, as [`Names::thing`] reads it,
    /// counted as its stack stood when the row was made.
    fn called_thing(&self, e: Option<Entity>) -> Option<Called> {
        let (name, stack, mass) = self.names.get(e?).ok()?;
        Some(called(name?, stack.map_or(1, |s| s.count), mass))
    }

    /// The same for one of a thing, whatever its stack holds: a use takes
    /// one, and by the time the row is made the stack already counts one
    /// fewer.
    fn called_one(&self, e: Option<Entity>) -> Option<Called> {
        let (name, _, mass) = self.names.get(e?).ok()?;
        Some(called(name?, 1, mass))
    }

    fn status_name(&self, id: atglyph_rules::StatusId) -> String {
        self.registries.as_deref().map(|r| r.statuses.name(id).to_string()).unwrap_or_default()
    }

    fn is_boon(&self, id: atglyph_rules::StatusId) -> bool {
        self.registries.as_deref().is_some_and(|r| r.statuses.get(id).boon)
    }

    /// Whether `id` is aimed at someone rather than at the ground, so a
    /// line may say whom it was used on. True with no abilities to ask,
    /// since then the targets are all there is to go on.
    fn aims_at_someone(&self, id: atglyph_rules::AbilityId) -> bool {
        self.abilities.as_deref().is_none_or(|a| a.get(id).aim != atglyph_rules::ability::Aim::Ground)
    }

    fn ability_name(&self, id: atglyph_rules::AbilityId) -> String {
        self.abilities.as_deref().map(|a| a.get(id).name.clone()).unwrap_or_default()
    }

    /// What landed harm that arrived by `reach`, as a line names it: the
    /// ability, or the thing by its own name, `ash rod`, since the line
    /// says whose it was. `None` for a blow, a shot, a throw or a plain
    /// effect, and for a thing with no name to say.
    fn landed_by(&self, reach: DamageRoute) -> Option<String> {
        match reach {
            DamageRoute::Ability(id) => Some(self.ability_name(id)).filter(|n| !n.is_empty()),
            DamageRoute::Thing(thing) => self.names.get(thing).ok()?.0.map(|n| n.as_str().to_string()),
            _ => None,
        }
    }
}

/// Fills [`NarrationView`] with what this pass did.
///
/// Runs in [`TurnSet::Record`], after every reaction, so a game's lines
/// told in [`TurnSet::React`] are read in the pass they answer. Reads by
/// kind, in [`NarratedEvents`]'s order, with one exception: a notice by whoever
/// held the turn is read first, since it was rolled as that actor looked
/// round, before it did anything; a notice by anyone else came of what the
/// turn did, a blow that woke it, and keeps its place after the blows.
pub(crate) fn collect_narration(mut view: ResMut<NarrationView>, mut heard: NarratedEvents, witness: Witness) {
    let turn = witness.turns.turn_number();
    let say = |phrase: Phrase, who: Option<Entity>, whom: Option<Entity>| {
        let at = who.and_then(|e| witness.at(e)).or_else(|| whom.and_then(|e| witness.at(e)));
        let mut said = Said::new(phrase, turn);
        said.who = who;
        said.whom = whom;
        said.at = at;
        said.seen = witness.seen(at, &[who, whom]);
        // Whether the line is spoken and whether the doer may be named are
        // two questions: the player is told it was shot either way, but a
        // shooter it never saw is `something`, and so is what it shot.
        said.who_seen = who.is_none_or(|e| witness.shows(e));
        said.whom_seen = whom.is_none_or(|e| witness.shows(e));
        said
    };

    let mut rows: Vec<Said> = Vec::new();
    // Each use's row, by who used which ability, and each use whose harm
    // was told by the ability's name: a use that harmed something is told
    // by that harm alone, so the ability is named once.
    let mut uses: Vec<(usize, Entity, atglyph_rules::AbilityId)> = Vec::new();
    let mut harmed_by: Vec<(Entity, atglyph_rules::AbilityId)> = Vec::new();
    for ev in heard.abilities.read() {
        match ev {
            AbilityEvent::Used { user, ability, targets, .. } => {
                let you = witness.is_you(*user);
                let others: Vec<Entity> = targets.iter().copied().filter(|t| t != user).collect();
                // One caught by a use aimed at the ground is told by the
                // harm it did them, not as whom it was used on: a step
                // through the air pointed past the player is not cast on
                // them, and the cell is all it was aimed at. Several caught
                // are still counted, which says nothing of being aimed at.
                let (phrase, whom) = match others.as_slice() {
                    [] => (if you { Phrase::YouUse } else { Phrase::Uses }, None),
                    [_] if !witness.aims_at_someone(*ability) => (if you { Phrase::YouUse } else { Phrase::Uses }, None),
                    [one] => (if you { Phrase::YouUseOn } else { Phrase::UsesOn }, Some(*one)),
                    _ => (if you { Phrase::YouUseOnMany } else { Phrase::UsesOnMany }, None),
                };
                let mut said = say(phrase, Some(*user), whom);
                said.named = witness.ability_name(*ability);
                said.amount = others.len() as i32;
                uses.push((rows.len(), *user, *ability));
                rows.push(said);
            }
            AbilityEvent::Refused { user, ability, why } if witness.is_you(*user) => {
                let mut said = say(Phrase::YouCannotUse, Some(*user), None);
                said.named = witness.ability_name(*ability);
                said.detail = why.iter().map(crate::view::ability::plain).collect::<Vec<_>>().join(", ");
                rows.push(said);
            }
            AbilityEvent::Refused { .. } => {}
        }
    }
    for b in heard.bumps.read() {
        // A bump that came to nothing because of what it offers is the
        // offers screen's to ask or explain, not something in the way:
        // saying both read as a shopkeeper blocking the door to the shop.
        if witness.is_you(b.actor) && !b.offers {
            rows.push(say(Phrase::YouBumpInto, Some(b.actor), Some(b.into)));
        }
    }
    for s in heard.swaps.read() {
        if witness.is_you(s.actor) {
            rows.push(say(Phrase::YouSwapWith, Some(s.actor), Some(s.with)));
        }
    }
    for d in heard.doors.read() {
        let (actor, at, opened) = match *d {
            DoorEvent::Opened { actor, at } => (actor, at, true),
            DoorEvent::Closed { actor, at } => (actor, at, false),
        };
        let phrase = match (witness.is_you(actor), opened) {
            (true, true) => Phrase::YouOpen,
            (true, false) => Phrase::YouClose,
            (false, true) => Phrase::OpensDoor,
            (false, false) => Phrase::ClosesDoor,
        };
        let mut said = say(phrase, Some(actor), None);
        said.at = Some(at);
        said.seen = witness.seen(Some(at), &[Some(actor)]);
        rows.push(said);
    }
    for t in heard.travels.read() {
        // The player's own going is the game's to say, since what a way
        // is called is. Anyone else is told of by where the player was
        // looking: one that left is told of by the cell it left, and one
        // that came after the player came with them, and is told of where
        // it stands now, seen and named. Not by the player's sight, which
        // a journey the same pass as theirs has not yet cast on the map
        // they arrived on, and which would judge a follower beside them by
        // the map they left.
        if witness.is_you(t.actor) {
            continue;
        }
        let followed = t.after.is_some_and(|led| witness.is_you(led));
        let mut said = say(if followed { Phrase::FollowsYou } else { Phrase::Leaves }, Some(t.actor), None);
        if followed {
            said.seen = true;
            said.who_seen = true;
        } else {
            said.at = Some(t.left);
            said.seen = witness.sees(Some(t.left));
            said.who_seen = witness.shows_at(t.actor, t.left);
        }
        rows.push(said);
    }
    for ev in heard.items.read() {
        let (actor, item, phrase) = match *ev {
            ItemEvent::PickedUp { actor, item, merged_into } => {
                (actor, merged_into.unwrap_or(item), if witness.is_you(actor) { Phrase::YouPickUp } else { Phrase::PicksUp })
            }
            ItemEvent::Dropped { actor, item, .. } => (actor, item, if witness.is_you(actor) { Phrase::YouDrop } else { Phrase::Drops }),
            ItemEvent::Equipped { actor, item } => {
                let wielded = witness.weapons.contains(item);
                let phrase = match (witness.is_you(actor), wielded) {
                    (true, true) => Phrase::YouWield,
                    (true, false) => Phrase::YouWear,
                    (false, true) => Phrase::Wields,
                    (false, false) => Phrase::Wears,
                };
                (actor, item, phrase)
            }
            ItemEvent::Unequipped { actor, item } => (actor, item, if witness.is_you(actor) { Phrase::YouTakeOff } else { Phrase::TakesOff }),
            // Why the player's try came to nothing; anyone else's refusal
            // is a wait nobody needs told.
            ItemEvent::WouldNotComeOff { actor, item } if witness.is_you(actor) => (actor, item, Phrase::WillNotComeOff),
            ItemEvent::WouldNotComeOff { .. } => continue,
            ItemEvent::Thrown { actor, item, .. } => (actor, item, if witness.is_you(actor) { Phrase::YouThrow } else { Phrase::Throws }),
            // A use the engine landed is said; what using anything else
            // means is the game's, and so are the words.
            ItemEvent::Used { actor, item } if witness.used_to_effect(item) => {
                (actor, item, if witness.is_you(actor) { Phrase::YouUseThing } else { Phrase::UsesThing })
            }
            ItemEvent::Used { .. } => continue,
        };
        let mut said = say(phrase, Some(actor), None);
        said.what = Some(item);
        rows.push(said);
    }
    // A miss is said where a hit would have been, ahead of the damage the
    // same pass dealt, since both come from attacks made in that pass.
    for m in heard.missed.read() {
        let phrase = if witness.is_you(m.attacker) {
            Phrase::YouMiss
        } else if witness.is_you(m.target) {
            Phrase::MissesYou
        } else {
            Phrase::OthersMiss
        };
        rows.push(say(phrase, Some(m.attacker), Some(m.target)));
    }
    // Said before the harm it caused, so a line that reads the caster hurt
    // is never met with no reason: the row the reflection explains.
    for r in heard.reflected.read() {
        let phrase = if witness.is_you(r.onto) {
            Phrase::ReflectsOntoYou
        } else if witness.is_you(r.by) {
            Phrase::YouReflect
        } else {
            Phrase::OthersReflect
        };
        rows.push(say(phrase, Some(r.by), Some(r.onto)));
    }
    for d in heard.dealt.read() {
        let target = d.target;
        let you_target = witness.is_you(target);
        if let Some(status) = d.hit.status {
            if d.dealt > 0 {
                let mut said = say(if you_target { Phrase::YouTakeFromStatus } else { Phrase::TakesFromStatus }, Some(target), None);
                said.named = witness.status_name(status);
                said.amount = d.dealt;
                rows.push(said);
            }
            continue;
        }
        // A hit the target absorbed mended it, and reads as a mend: the
        // one who dealt it did not strike to no effect.
        if d.dealt < 0 {
            let mut said = say(if you_target { Phrase::YouMend } else { Phrase::Mends }, Some(target), None);
            said.amount = -d.dealt;
            rows.push(said);
            continue;
        }
        if d.hit.attacker == Some(target) {
            if you_target && d.dealt > 0 {
                let mut said = say(Phrase::YouHurtYourself, Some(target), None);
                said.amount = d.dealt;
                rows.push(said);
            }
            continue;
        }
        let you_attacker = d.hit.attacker.is_some_and(|a| witness.is_you(a));
        let landed = d.dealt > 0;
        // Harm an ability or a thing landed names what landed it: a rod's
        // bolt is not a blow struck by whoever zapped it, and "You hit the
        // rat" after "You use an ash rod" reads as a second action.
        if let Some(named) = witness.landed_by(d.reach) {
            let phrase = match (you_attacker, you_target, landed) {
                (true, _, true) => Phrase::YourUseHits,
                (true, _, false) => Phrase::YourUseHitsNothing,
                (false, true, true) => Phrase::UseHitsYou,
                (false, true, false) => Phrase::UseHitsYouForNothing,
                (false, false, true) => Phrase::OthersUseHits,
                (false, false, false) => Phrase::OthersUseHitsNothing,
            };
            let mut said = say(phrase, d.hit.attacker, Some(target));
            said.named = named;
            said.amount = d.dealt;
            rows.push(said);
            if let (DamageRoute::Ability(id), Some(by)) = (d.reach, d.hit.attacker) {
                harmed_by.push((by, id));
            }
            continue;
        }
        // A shot is not a blow: it came from somewhere the player may not
        // even be able to see, and "hits you" reads as something standing
        // over you. Anything else that did not travel as a weapon keeps
        // the blow wording, since a poison is already narrated by whatever
        // inflicted it.
        let shot = d.reach == DamageRoute::Shot;
        let phrase = match (shot, you_attacker, you_target, landed) {
            (false, true, _, true) => Phrase::YouHit,
            (false, true, _, false) => Phrase::YouHitNothing,
            (false, false, true, true) => Phrase::HitsYou,
            (false, false, true, false) => Phrase::HitsYouForNothing,
            (false, false, false, true) => Phrase::OthersFight,
            (false, false, false, false) => Phrase::OthersHitNothing,
            (true, true, _, true) => Phrase::YouShoot,
            (true, true, _, false) => Phrase::YouShootNothing,
            (true, false, true, true) => Phrase::ShootsYou,
            (true, false, true, false) => Phrase::ShootsYouForNothing,
            (true, false, false, true) => Phrase::OthersShoot,
            (true, false, false, false) => Phrase::OthersShootNothing,
        };
        let mut said = say(phrase, d.hit.attacker, Some(target));
        said.amount = d.dealt;
        rows.push(said);
    }
    for h in heard.healed.read() {
        // A heal that restored nothing is no line at all, and neither is
        // a status mending each turn, which was said when it was gained.
        if h.restored > 0 && h.status.is_none() {
            let mut said = say(if witness.is_you(h.target) { Phrase::YouMend } else { Phrase::Mends }, Some(h.target), None);
            said.amount = h.restored;
            rows.push(said);
        }
    }
    for ev in heard.statuses.read() {
        let (target, status, phrase) = match *ev {
            StatusEvent::Applied { target, status } if !witness.is_you(target) => (target, status, Phrase::IsAfflicted),
            StatusEvent::Applied { target, status } => (target, status, if witness.is_boon(status) { Phrase::YouGainABoon } else { Phrase::YouAreAfflicted }),
            // Kept up rather than begun, as a cloud breathed again keeps its
            // status every turn: it was said when it began.
            StatusEvent::Renewed { .. } => continue,
            StatusEvent::Expired { target, status } if witness.is_you(target) => (target, status, Phrase::YouAreNoLonger),
            // A boon broken off, by a blow or by the worn thing that held it
            // coming off, is lost rather than cured: the player is not glad
            // of it.
            StatusEvent::Cured { target, status } if witness.is_you(target) => {
                (target, status, if witness.is_boon(status) { Phrase::YouAreNoLonger } else { Phrase::YourAfflictionPasses })
            }
            _ => continue,
        };
        let mut said = say(phrase, Some(target), None);
        said.named = witness.status_name(status);
        rows.push(said);
    }
    for d in heard.deaths.read() {
        // A prop is broken, not killed: nothing dies that was never alive,
        // and a log that says a supply crate was killed reads as a bug.
        // One of the player's own side is a loss, told in its own words
        // and tone whoever is to blame.
        let thing = witness.is_a_prop(d.entity);
        let ally = !d.was_player && !thing && witness.allied_to_you(d.entity);
        let mut said = match (d.was_player, d.credit, thing) {
            (true, _, _) => say(Phrase::YouDie, Some(d.entity), None),
            (false, Some(by), false) if witness.is_you(by) => say(if ally { Phrase::YouKillAlly } else { Phrase::YouKill }, Some(by), Some(d.entity)),
            (false, Some(by), true) if witness.is_you(by) => say(Phrase::YouDestroy, Some(by), Some(d.entity)),
            (false, Some(by), false) => say(if ally { Phrase::KillsAlly } else { Phrase::Kills }, Some(by), Some(d.entity)),
            (false, Some(by), true) => say(Phrase::Destroys, Some(by), Some(d.entity)),
            (false, None, false) => say(if ally { Phrase::AllyDies } else { Phrase::Dies }, Some(d.entity), None),
            (false, None, true) => say(Phrase::IsDestroyed, Some(d.entity), None),
        };
        // The dead have left their cell already; the event says where.
        said.at = Some(d.at);
        said.seen = d.was_player || witness.seen(Some(d.at), &[d.credit]);
        rows.push(said);
    }
    let mut first: Vec<Said> = Vec::new();
    for n in heard.noticed.read() {
        if witness.is_you(n.subject) {
            let said = say(Phrase::NoticesYou, Some(n.observer), Some(n.subject));
            if witness.holds_the_turn(n.observer) { first.push(said) } else { rows.push(said) }
        }
    }
    for l in heard.lights.read() {
        let LightEvent::BurntOut { entity } = *l;
        if witness.is_you(entity) {
            rows.push(say(Phrase::YourLightGoesOut, Some(entity), None));
        } else {
            let mut said = say(Phrase::LightGoesOut, None, None);
            said.what = Some(entity);
            said.at = witness.at(entity);
            said.seen = witness.seen(said.at, &[]);
            rows.push(said);
        }
    }
    for f in heard.fires.read() {
        if let FireEvent::Scorched { entity, at } = *f {
            let mut said = say(if witness.is_you(entity) { Phrase::YouStandInFire } else { Phrase::StandsInFire }, Some(entity), None);
            said.at = Some(at);
            rows.push(said);
        }
    }
    for tell in heard.tells.read() {
        let mut said = Said::told(tell, turn);
        said.at = tell.who.or(tell.whom).and_then(|e| witness.at(e));
        rows.push(said);
    }
    // "You use bolt on the rat." and then "Your bolt hits the rat for 3."
    // say the bolt twice; the harm line says all the first did and how
    // much, so the use goes. A use that harmed nothing, a mend or a step
    // through the air, keeps its own line, since it is the only one.
    // Within the pass, where a use and what it landed are always found
    // together, so nothing spoken earlier is ever taken back.
    let unsaid: Vec<usize> = uses.iter().filter(|(_, user, ability)| harmed_by.contains(&(*user, *ability))).map(|(row, ..)| *row).collect();
    let mut row = 0;
    rows.retain(|_| {
        row += 1;
        !unsaid.contains(&(row - 1))
    });
    // What each row's subjects are called, read now, inside the pass: by
    // the time the row is spoken the dead have been renamed as remains
    // and a thrown thing has merged into a stack.
    for said in first.iter_mut().chain(rows.iter_mut()) {
        let thing = if matches!(said.phrase(), Some(Phrase::YouUseThing | Phrase::UsesThing)) {
            witness.called_one(said.what)
        } else {
            witness.called_thing(said.what)
        };
        said.called = [witness.called_actor(said.who), witness.called_actor(said.whom)];
        said.called_thing = thing;
    }
    view.rows.extend(first);
    view.rows.extend(rows);
}

/// One template and the tone it is spoken in, or silence.
type Entry = Option<(String, ToneId)>;

/// The words for every phrase.
///
/// Placeholders: `{who}` and `{whom}` are `you` or `the <Name>`, and
/// `{Who}` and `{Whom}` the same capitalised; `{what}` is the thing's
/// [`Name`] said with its count, `a pebble` or `5 pebbles`, and with no
/// `a` for a [`MassNoun`], `chain mail` (see [`atglyph_core::noun`]);
/// `{your what}` is the same thing as the player's own, `your pebble`, for
/// a line about something the player held; `{What}` and `{Your what}` are
/// capitalised; `{n}` the amount;
/// `{named}` the registry's name for the status, kind or ability; and
/// `{detail}` whatever more there is to say. A name other than the player's
/// is coloured as the thing is drawn.
///
/// The default is the English the games spoke before the narrator existed.
/// A game changes a phrase, silences one, or speaks the unseen too.
#[derive(Resource, Debug, Clone)]
pub struct Phrasebook {
    entries: BTreeMap<Phrase, Entry>,
    /// Whether rows the player did not see are spoken. Off by default: a
    /// blow struck out of sight is a blow the player has no way to place.
    pub speak_unseen: bool,
}

impl Default for Phrasebook {
    fn default() -> Self {
        use Phrase::*;
        let table: [(Phrase, &str, ToneId); 79] = [
            (YouHit, "You hit {whom} for {n}.", Tones::HIT),
            (YouHitNothing, "You hit {whom}, to no effect.", Tones::MUTED),
            (HitsYou, "{Who} hits you for {n}.", Tones::BAD),
            (HitsYouForNothing, "{Who} hits you, to no effect.", Tones::MUTED),
            (YouShoot, "You shoot {whom} for {n}.", Tones::HIT),
            (YouShootNothing, "You shoot {whom}, to no effect.", Tones::MUTED),
            (ShootsYou, "{Who} shoots you for {n}.", Tones::BAD),
            (ShootsYouForNothing, "{Who} shoots you, to no effect.", Tones::MUTED),
            (OthersShoot, "{Who} shoots {whom} for {n}.", Tones::TEXT),
            (OthersShootNothing, "{Who} shoots {whom}, to no effect.", Tones::MUTED),
            (YourUseHits, "Your {named} hits {whom} for {n}.", Tones::HIT),
            (YourUseHitsNothing, "Your {named} hits {whom}, to no effect.", Tones::MUTED),
            (UseHitsYou, "{Who}'s {named} hits you for {n}.", Tones::BAD),
            (UseHitsYouForNothing, "{Who}'s {named} hits you, to no effect.", Tones::MUTED),
            (OthersUseHits, "{Who}'s {named} hits {whom} for {n}.", Tones::TEXT),
            (OthersUseHitsNothing, "{Who}'s {named} hits {whom}, to no effect.", Tones::MUTED),
            (YouMiss, "You miss {whom}.", Tones::MUTED),
            (MissesYou, "{Who} misses you.", Tones::MUTED),
            (OthersMiss, "{Who} misses {whom}.", Tones::MUTED),
            (ReflectsOntoYou, "{Who} turns it back on you.", Tones::BAD),
            (YouReflect, "You turn it back on {whom}.", Tones::HIT),
            (OthersReflect, "{Who} turns it back on {whom}.", Tones::TEXT),
            (OthersFight, "{Who} hits {whom} for {n}.", Tones::TEXT),
            (OthersHitNothing, "{Who} hits {whom}, to no effect.", Tones::MUTED),
            (YouHurtYourself, "You hurt yourself for {n}.", Tones::BAD),
            (YouMend, "You mend for {n}.", Tones::GOOD),
            (Mends, "{Who} mends for {n}.", Tones::TEXT),
            (YouTakeFromStatus, "You take {n} from {named}.", Tones::BAD),
            (TakesFromStatus, "{Who} takes {n} from {named}.", Tones::TEXT),
            (YouKill, "You kill {whom}!", Tones::KILL),
            (YouDestroy, "You destroy {whom}.", Tones::KILL),
            (Destroys, "{Who} destroys {whom}.", Tones::TEXT),
            (IsDestroyed, "{Who} is destroyed.", Tones::TEXT),
            (Kills, "{Who} kills {whom}.", Tones::TEXT),
            (Dies, "{Who} dies.", Tones::GOOD),
            // One of the player's side is a loss, whoever dealt it, the
            // player included: no `!` and no kill tone for that.
            (AllyDies, "{Who} dies.", Tones::BAD),
            (KillsAlly, "{Who} kills {whom}.", Tones::BAD),
            (YouKillAlly, "You kill {whom}.", Tones::BAD),
            (YouDie, "You die.", Tones::BAD),
            (YouPickUp, "You pick up {what}.", Tones::TEXT),
            (PicksUp, "{Who} picks up {what}.", Tones::NOTICE),
            // What the player puts on, wields, takes off or drops is
            // theirs, and said so: "You drop your sword.", where "a sword"
            // read as though it were any sword.
            (YouDrop, "You drop {your what}.", Tones::TEXT),
            (Drops, "{Who} drops {what}.", Tones::NOTICE),
            (YouWear, "You put on {your what}.", Tones::TEXT),
            (Wears, "{Who} puts on {what}.", Tones::NOTICE),
            (YouWield, "You wield {your what}.", Tones::TEXT),
            (Wields, "{Who} wields {what}.", Tones::NOTICE),
            (YouTakeOff, "You take off {your what}.", Tones::MUTED),
            (TakesOff, "{Who} takes off {what}.", Tones::MUTED),
            (WillNotComeOff, "{Your what} will not come off.", Tones::BAD),
            (YouThrow, "You throw {what}.", Tones::TEXT),
            (Throws, "{Who} throws {what}.", Tones::NOTICE),
            (YouUseThing, "You use {what}.", Tones::TEXT),
            (UsesThing, "{Who} uses {what}.", Tones::NOTICE),
            (YouOpen, "You open the door.", Tones::MUTED),
            (YouClose, "You close the door.", Tones::MUTED),
            (OpensDoor, "{Who} opens a door.", Tones::NOTICE),
            (ClosesDoor, "{Who} closes a door.", Tones::NOTICE),
            (Leaves, "{Who} leaves.", Tones::NOTICE),
            (FollowsYou, "{Who} follows you.", Tones::NOTICE),
            (YouBumpInto, "{Whom} is in the way.", Tones::MUTED),
            (YouSwapWith, "You change places with {whom}.", Tones::MUTED),
            (NoticesYou, "{Who} notices you.", Tones::NOTICE),
            (YouUse, "You use {named}.", Tones::TEXT),
            (YouUseOn, "You use {named} on {whom}.", Tones::TEXT),
            (YouUseOnMany, "You use {named}, catching {n}.", Tones::TEXT),
            (Uses, "{Who} uses {named}.", Tones::BAD),
            (UsesOn, "{Who} uses {named} on {whom}.", Tones::BAD),
            (UsesOnMany, "{Who} uses {named}, catching {n}.", Tones::BAD),
            (YouCannotUse, "You cannot use {named}: {detail}.", Tones::BAD),
            // A status is named as what its holder is, "warded" or
            // "scorched", so every line about one says "you are".
            (YouAreAfflicted, "You are {named}.", Tones::BAD),
            (YouGainABoon, "You are {named}.", Tones::GOOD),
            (YouAreNoLonger, "You are no longer {named}.", Tones::MUTED),
            (YourAfflictionPasses, "You are no longer {named}.", Tones::GOOD),
            (IsAfflicted, "{Who} is {named}.", Tones::NOTICE),
            (YourLightGoesOut, "Your light gutters and goes out.", Tones::BAD),
            (LightGoesOut, "{What} gutters and goes out.", Tones::MUTED),
            (YouStandInFire, "You are standing in fire.", Tones::BAD),
            (StandsInFire, "{Who} is standing in fire.", Tones::TEXT),
        ];
        let mut entries = BTreeMap::new();
        for (phrase, text, tone) in table {
            entries.insert(phrase, Some((text.to_string(), tone)));
        }
        Self { entries, speak_unseen: false }
    }
}

impl Phrasebook {
    /// Speaks `phrase` as `text`, in `tone`.
    pub fn set(&mut self, phrase: Phrase, text: impl Into<String>, tone: ToneId) -> &mut Self {
        self.entries.insert(phrase, Some((text.into(), tone)));
        self
    }

    /// Never speaks `phrase`.
    pub(crate) fn silence(&mut self, phrase: Phrase) -> &mut Self {
        self.entries.insert(phrase, None);
        self
    }

    /// The words and tone for `phrase`, or `None` when it is silenced.
    pub fn get(&self, phrase: Phrase) -> Option<(&str, ToneId)> {
        self.entries.get(&phrase).and_then(|e| e.as_ref().map(|(t, tone)| (t.as_str(), *tone)))
    }
}

/// How `e` is named in a line, and in what colour.
struct Named {
    text: String,
    color: Option<Color>,
}

impl Named {
    /// The same, but under the name the row was made with, when it was
    /// made with one: what a thing was called then outranks what it is
    /// called now.
    fn or_called(self, called: &Option<String>) -> Self {
        match called {
            Some(text) => Self { text: text.clone(), color: self.color },
            None => self,
        }
    }
}

/// What the presenter reads to put names to entities.
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct Names<'w, 's> {
    player: Query<'w, 's, (), With<Player>>,
    names: Query<'w, 's, (Option<&'static Name>, Option<&'static Glyph>)>,
    stacks: Query<'w, 's, &'static Stack>,
    mass: Query<'w, 's, (), With<MassNoun>>,
}

impl Names<'_, '_> {
    /// `you`, or `the <Name>` in the thing's colour, or `something`.
    /// What anything the player cannot make out is called.
    fn unseen(&self) -> Named {
        Named { text: UNSEEN.into(), color: None }
    }

    fn actor(&self, e: Option<Entity>) -> Named {
        let Some(e) = e else { return self.unseen() };
        if self.player.contains(e) {
            return Named { text: "you".into(), color: None };
        }
        match self.names.get(e) {
            Ok((Some(name), glyph)) => Named { text: atglyph_core::noun::with_the(name.as_str()), color: glyph.map(|g| g.fg) },
            _ => self.unseen(),
        }
    }

    /// The thing as a sentence says it, by `say`: under the name the row
    /// was made with, `called`, or else its [`Name`] now, counted by its
    /// [`Stack`]; in its colour, or `something` when it has no name.
    fn thing(&self, e: Option<Entity>, called: &Option<Called>, say: fn(&Called) -> String) -> Named {
        let glyph = e.and_then(|e| self.names.get(e).ok());
        let color = glyph.and_then(|(_, g)| g).map(|g| g.fg);
        let now = || {
            let e = e?;
            let (name, _) = self.names.get(e).ok()?;
            Some(self::called(name?, self.stacks.get(e).map_or(1, |s| s.count), self.mass.contains(e)))
        };
        match called.clone().or_else(now) {
            Some(called) => Named { text: say(&called), color },
            None => self.unseen(),
        }
    }
}

/// What the narrator calls anything the player cannot make out: a thing
/// with no `Name`, or a doer out of sight.
///
/// One word in one place. A game that wants another rewords the phrases
/// that can name an unseen doer, since a template is the level a game
/// already edits.
pub(crate) const UNSEEN: &str = "something";

/// Fills `template` for `said`: the text, and a span for each name that
/// has a colour.
///
/// `{who}` and `{whom}` the player did not see, by the row's
/// [`who_seen`](Said::who_seen) and [`whom_seen`](Said::whom_seen), are
/// [`UNSEEN`] unless `speak_unseen`. The caller passes it, because whether
/// the unseen are named is [`Phrasebook::speak_unseen`], the presenter's
/// setting and not the row's business.
pub(crate) fn render(template: &str, said: &Said, names: &Names<'_, '_>, speak_unseen: bool) -> (String, Vec<Span>) {
    let mut out = String::new();
    let mut spans = Vec::new();
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let Some(close) = rest[open..].find('}') else {
            out.push_str(&rest[open..]);
            rest = "";
            break;
        };
        let key = &rest[open + 1..open + close];
        rest = &rest[open + close + 1..];
        let capital = key.chars().next().is_some_and(|c| c.is_uppercase());
        let named = match key.to_ascii_lowercase().as_str() {
            // An unseen doer, or an unseen one it was done to, is
            // `something`, whatever it is called: the same word a nameless
            // thing gets, so one template serves both.
            "who" if !(said.who_seen || speak_unseen) => Some(names.unseen()),
            "whom" if !(said.whom_seen || speak_unseen) => Some(names.unseen()),
            "who" => Some(names.actor(said.who).or_called(&said.called[0])),
            "whom" => Some(names.actor(said.whom).or_called(&said.called[1])),
            "what" => Some(names.thing(said.what, &said.called_thing, Called::counted)),
            "your what" => Some(names.thing(said.what, &said.called_thing, Called::yours)),
            "n" => Some(Named { text: said.amount.to_string(), color: None }),
            "named" => Some(Named { text: said.named.clone(), color: None }),
            "detail" => Some(Named { text: said.detail.clone(), color: None }),
            _ => None,
        };
        let Some(Named { mut text, color }) = named else {
            out.push('{');
            out.push_str(key);
            out.push('}');
            continue;
        };
        if capital {
            text = capitalised(&text);
        }
        if let Some(color) = color {
            spans.push(Span { start: out.chars().count(), len: text.chars().count(), color });
        }
        out.push_str(&text);
    }
    out.push_str(rest);
    (out, spans)
}

/// `s` with its first letter raised: `the goblin` reads `The goblin` at the
/// start of a line, and `you` reads `You`.
fn capitalised(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// Speaks every row of the view into the log, and clears the view.
pub(crate) fn speak(mut view: ResMut<NarrationView>, book: Res<Phrasebook>, names: Names, mut log: ResMut<MessageLog>) {
    for said in view.rows.drain(..) {
        if !said.seen && !book.speak_unseen {
            continue;
        }
        let words = match &said.words {
            Words::Phrase(phrase) => book.get(*phrase),
            Words::Own { text, tone } => Some((text.as_str(), *tone)),
        };
        let Some((template, tone)) = words else { continue };
        let (text, spans) = render(template, &said, &names, book.speak_unseen);
        if !text.is_empty() {
            log.push_spans(text, spans, tone, said.turn);
        }
    }
}

/// Speaks what the turns did into the log, in the words of its
/// [`Phrasebook`].
///
/// Adds [`NarrationViewPlugin`] if the game has not. A game that wants some
/// other words changes them here before the plugin is added, or through the
/// [`Phrasebook`] resource afterwards.
#[derive(Default)]
pub struct NarratorPlugin {
    book: Phrasebook,
}

impl NarratorPlugin {
    /// Speaks `phrase` as `text`, in `tone`.
    pub fn phrase(mut self, phrase: Phrase, text: impl Into<String>, tone: ToneId) -> Self {
        self.book.set(phrase, text, tone);
        self
    }

    /// Never speaks `phrase`.
    pub fn silence(mut self, phrase: Phrase) -> Self {
        self.book.silence(phrase);
        self
    }

    /// Speaks what the player did not see as well.
    pub fn speaking_the_unseen(mut self) -> Self {
        self.book.speak_unseen = true;
        self
    }
}

impl Plugin for NarratorPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<NarrationViewPlugin>() {
            app.add_plugins(NarrationViewPlugin);
        }
        // A name is coloured in its glyph's colour, so a remnant that does
        // something in a dead actor's name takes its glyph with the name.
        atglyph_bevy::KeepOnRemnant::keep_on_remnant::<Glyph>(app);
        app.insert_resource(self.book.clone()).add_systems(Update, speak.in_set(crate::ViewSet::Speak));
    }

    fn finish(&self, app: &mut App) {
        atglyph_bevy::depends_on::<crate::UiPlugin>(app, "NarratorPlugin");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::Stage;
    use atglyph_core::DiceRoll;

    fn lines(stage: &Stage) -> Vec<(String, ToneId)> {
        stage.app.world().resource::<MessageLog>().iter().map(|e| (e.text.clone(), e.tone)).collect()
    }

    /// The player strikes a slime and is struck back; each line is in the
    /// right perspective and tone, and the slime's name wears its colour.
    #[test]
    fn a_blow_each_way_is_spoken_from_the_players_side_with_the_foe_named_in_its_colour() {
        let mut stage = Stage::new(NarratorPlugin::default());
        let (player, kind, theirs) = (stage.player, stage.kind, stage.theirs);
        let green = Color::srgb(0.2, 0.9, 0.3);
        let slime = stage
            .app
            .world_mut()
            .spawn((
                Actor,
                Blocks,
                Position(stage.at.offset(1, 0)),
                Health::full(20),
                Faction(theirs),
                MeleeAttack::new(kind, DiceRoll::flat(2)),
                Name::new("slime"),
                atglyph_render::Glyph::new('s', green).on_layer(5),
            ))
            .id();
        stage.tick();
        stage.app.world_mut().write_message(Intent::new(player, Attack(slime)));
        stage.tick();
        // The harness has no minds, so the slime strikes back as a blow
        // written for it: the same event a mind's attack lands as.
        stage.app.world_mut().write_message(DamageEvent::new(player, atglyph_rules::Hit::by(slime, kind, 2)));
        stage.tick();
        let said = lines(&stage);
        let hits: Vec<&(String, ToneId)> = said.iter().filter(|(t, _)| t.starts_with("You hit the slime for ")).collect();
        assert!(!hits.is_empty(), "{said:?}");
        assert!(hits.iter().all(|(_, tone)| *tone == Tones::HIT), "your blows are highlighted: {said:?}");
        let struck = said.iter().find(|(t, _)| t.starts_with("The slime hits you")).expect("the slime struck back: {said:?}");
        assert_eq!(struck.1, Tones::BAD);
        let entry = stage.app.world().resource::<MessageLog>().iter().find(|e| e.text.starts_with("You hit the slime")).unwrap();
        assert_eq!(entry.spans.len(), 1, "one name coloured");
        assert_eq!((entry.spans[0].start, entry.spans[0].len, entry.spans[0].color), (8, 9, green), "'the slime' in its green: {entry:?}");
    }

    /// A foe named with its own article keeps it rather than being given
    /// a second one, one named with `a` likewise, a proper name takes
    /// none, and a plain name still takes `the`: the fault this guards
    /// doubled a name's own article, reading `the the stranger` where
    /// `the stranger` was meant.
    #[test]
    fn a_blow_from_a_foe_is_said_with_its_own_article_never_doubled() {
        let mut stage = Stage::new(NarratorPlugin::default());
        let (player, kind, theirs, at) = (stage.player, stage.kind, stage.theirs, stage.at);
        let mut spawn =
            |name: &'static str, p: Point| stage.app.world_mut().spawn((Actor, Blocks, Position(p), Health::full(20), Faction(theirs), Name::new(name))).id();
        let already_the = spawn("the stranger", at.offset(1, 0));
        let already_a = spawn("a crust of bread", at.offset(-1, 0));
        let grubnak = spawn("Grubnak", at.offset(0, 1));
        let plain = spawn("owl", at.offset(0, -1));
        stage.tick();
        for who in [already_the, already_a, grubnak, plain] {
            stage.app.world_mut().write_message(DamageEvent::new(player, atglyph_rules::Hit::by(who, kind, 3)));
        }
        stage.tick();
        let said: Vec<String> = lines(&stage).into_iter().map(|(text, _)| text).collect();
        assert!(said.contains(&"The stranger hits you for 3.".to_string()), "no doubled article: {said:?}");
        assert!(said.contains(&"A crust of bread hits you for 3.".to_string()), "{said:?}");
        assert!(said.contains(&"Grubnak hits you for 3.".to_string()), "a proper name takes no article: {said:?}");
        assert!(said.contains(&"The owl hits you for 3.".to_string()), "a plain name still takes one: {said:?}");
    }

    /// Someone the player watched leave for another map is said to have
    /// left, one that came through after the player is said to have
    /// followed, and one that left from a cell out of sight is not spoken
    /// of: where it stands now is a cell of another map, so it is the cell
    /// it left that decides.
    #[test]
    fn one_who_leaves_in_sight_is_said_to_leave_and_one_who_follows_the_player_to_follow() {
        let mut stage = Stage::new(NarratorPlugin::default());
        let (player, theirs) = (stage.player, stage.theirs);
        let beside = stage.at.offset(1, 0);
        let rat = stage.app.world_mut().spawn((Actor, Blocks, Position(beside), Health::full(1), Faction(theirs), Name::new("rat"))).id();
        stage.tick();
        let went = |left: Point, after: Option<Entity>| Travelled { actor: rat, from: MapId::ZERO, left, to: MapId(3), after };
        stage.app.world_mut().write_message(went(beside, None));
        stage.tick();
        stage.app.world_mut().write_message(went(stage.at.offset(500, 0), None));
        stage.tick();
        stage.app.world_mut().write_message(went(stage.at.offset(500, 0), Some(player)));
        stage.tick();
        let said: Vec<String> = lines(&stage).into_iter().map(|(text, _)| text).collect();
        assert_eq!(said.iter().filter(|t| *t == "The rat leaves.").count(), 1, "the one seen going, and not the one unseen: {said:?}");
        assert!(said.contains(&"The rat follows you.".to_string()), "{said:?}");
    }

    /// One that came through after the player is told of as following them
    /// wherever the player's sight, not yet cast on the map they arrived
    /// on, says it stands: it came with them, so it is seen and named.
    #[test]
    fn a_follower_that_came_through_with_the_player_is_told_of_as_following_even_out_of_the_sight_left_behind() {
        let mut stage = Stage::new(NarratorPlugin::default());
        let (player, theirs) = (stage.player, stage.theirs);
        let far = stage.at.offset(500, 0);
        let rat = stage.app.world_mut().spawn((Actor, Blocks, Position(far), Health::full(1), Faction(theirs), Name::new("rat"))).id();
        stage.tick();
        stage.app.world_mut().write_message(Travelled { actor: rat, from: MapId::ZERO, left: far, to: MapId(3), after: Some(player) });
        stage.tick();
        let said = lines(&stage);
        assert!(said.contains(&("The rat follows you.".to_string(), Tones::NOTICE)), "told, and named: {said:?}");
    }

    /// A death on the player's own side is a loss, whoever dealt it: its
    /// own phrase in the bad tone, the player's own blow included, where a
    /// foe's death is good news; and a game rewords or silences it as any
    /// phrase.
    #[test]
    fn a_death_on_the_players_side_is_told_as_a_loss_whoever_dealt_it() {
        let mut stage = Stage::new(NarratorPlugin::default().phrase(Phrase::KillsAlly, "{Whom} falls to {who}.", Tones::BAD));
        let (player, theirs) = (stage.player, stage.theirs);
        let ours = stage.app.world().get::<Faction>(player).expect("a side").0;
        let mut body = |name: &str, side| {
            stage.app.world_mut().spawn((Actor, Position(stage.at.offset(1, 0)), Health::full(1), Faction(side), Name::new(name.to_string()))).id()
        };
        let (friend, squire, page, rat) = (body("friend", ours), body("squire", ours), body("page", ours), body("rat", theirs));
        stage.tick();
        let at = stage.at.offset(1, 0);
        for (entity, credit) in [(friend, None), (squire, Some(rat)), (page, Some(player)), (rat, None)] {
            stage.app.world_mut().write_message(DeathEvent { entity, at, credit, was_player: false });
        }
        stage.tick();
        let said = lines(&stage);
        for line in ["The friend dies.", "The squire falls to the rat.", "You kill the page."] {
            assert!(said.contains(&(line.to_string(), Tones::BAD)), "{line:?} as a loss: {said:?}");
        }
        assert!(said.contains(&("The rat dies.".to_string(), Tones::GOOD)), "a foe's death is good news: {said:?}");
    }

    /// A kill by the player is its own phrase in the brightest tone, and a
    /// game's own words for a phrase replace the engine's.
    #[test]
    fn a_kill_by_the_player_is_spoken_as_a_kill_and_a_game_may_reword_it() {
        let mut stage = Stage::new(NarratorPlugin::default().phrase(Phrase::YouKill, "{Whom} is no more.", Tones::GOOD));
        let (player, theirs) = (stage.player, stage.theirs);
        let rat = stage.app.world_mut().spawn((Actor, Blocks, Position(stage.at.offset(1, 0)), Health::full(1), Faction(theirs), Name::new("rat"))).id();
        stage.tick();
        stage.app.world_mut().write_message(Intent::new(player, Attack(rat)));
        stage.tick();
        let said = lines(&stage);
        assert!(said.contains(&("The rat is no more.".to_string(), Tones::GOOD)), "{said:?}");
        assert!(!said.iter().any(|(t, _)| t == "The rat dies."), "not spoken twice");
    }

    /// A blow is spoken of what it landed on, not of what that became.
    ///
    /// A kill and the blow before it used to read "You hit the coolant rat
    /// remains for 2" and "You kill the coolant rat remains!", because a
    /// row is made inside the pass and spoken after it, and by then the
    /// rat had become remains and been renamed.
    #[test]
    fn a_kill_names_what_was_struck_and_not_what_it_became() {
        let mut stage = Stage::new_with(NarratorPlugin::default(), |app| {
            app.add_plugins((atglyph_bevy::props::PropsPlugin, atglyph_bevy::remains::RemainsPlugin));
            app.insert_resource(Seed(atglyph_bevy::testing::TEST_SEED));
        });
        let (player, theirs) = (stage.player, stage.theirs);
        let rat = stage
            .app
            .world_mut()
            .spawn((Actor, Blocks, Position(stage.at.offset(1, 0)), Health::full(1), Faction(theirs), Name::new("coolant rat"), LeavesRemains))
            .id();
        stage.tick();
        stage.app.world_mut().write_message(Intent::new(player, Attack(rat)));
        stage.tick();

        let said = lines(&stage);
        assert!(said.iter().any(|(t, _)| t == "You kill the coolant rat!"), "the rat, not its remains: {said:?}");
        assert!(!said.iter().any(|(t, _)| t.contains("remains")), "and nothing reads as a blow on a corpse: {said:?}");
        // And the engine did rename it, so this is the row keeping its own
        // words rather than the rename not having happened.
        assert_eq!(stage.app.world().get::<Name>(rat).map(|n| n.as_str().to_string()), Some("coolant rat remains".into()));
    }

    /// Nothing dies that was never alive: a crate the player breaks is
    /// destroyed, and the word "kill" never appears.
    #[test]
    fn a_prop_the_player_breaks_is_destroyed_and_never_killed() {
        let mut stage = Stage::new(NarratorPlugin::default());
        let player = stage.player;
        let at = stage.at.offset(1, 0);
        // A crate stands there: a prop with health, which is all it takes to
        // be broken, and no faction, since a crate takes no side.
        let supply = stage.app.world_mut().spawn((atglyph_bevy::Prop, Blocks, Position(at), Health::full(1), Name::new("supply crate"))).id();
        stage.tick();
        stage.app.world_mut().write_message(Intent::new(player, Attack(supply)));
        stage.tick();
        let said = lines(&stage);
        assert!(said.iter().any(|(t, _)| t == "You destroy the supply crate."), "destroyed: {said:?}");
        assert!(!said.iter().any(|(t, _)| t.contains("kill")), "and never killed: {said:?}");
    }

    /// A blow out of the player's sight is a row and not a line, unless the
    /// game asks for the unseen; a silenced phrase is a row and never a line.
    #[test]
    fn the_unseen_and_the_silenced_are_rows_the_log_never_hears() {
        let mut stage = Stage::new(NarratorPlugin::default().silence(Phrase::OthersHitNothing));
        let (kind, theirs) = (stage.kind, stage.theirs);
        // Far out of sight: two monsters, one striking the other for nothing
        // and then for something.
        let far = stage.at.offset(40, 0);
        let a = stage
            .app
            .world_mut()
            .spawn((Actor, Blocks, Position(far), Health::full(20), Faction(theirs), MeleeAttack::new(kind, DiceRoll::flat(3)), Name::new("a")))
            .id();
        let b = stage.app.world_mut().spawn((Actor, Blocks, Position(far.offset(1, 0)), Health::full(20), Armor(9), Faction(theirs), Name::new("b"))).id();
        stage.tick();
        stage.app.world_mut().write_message(DamageEvent::new(b, atglyph_rules::Hit::by(a, kind, 3)));
        stage.tick();
        assert!(lines(&stage).is_empty(), "nothing seen, nothing said: {:?}", lines(&stage));

        stage.app.world_mut().resource_mut::<Phrasebook>().speak_unseen = true;
        stage.app.world_mut().write_message(DamageEvent::new(b, atglyph_rules::Hit::by(a, kind, 3)));
        stage.tick();
        assert!(lines(&stage).is_empty(), "spoken unseen, but that phrase is silenced: {:?}", lines(&stage));
        stage.app.world_mut().entity_mut(b).insert(Armor(0));
        stage.app.world_mut().write_message(DamageEvent::new(b, atglyph_rules::Hit::by(a, kind, 3)));
        stage.tick();
        assert_eq!(lines(&stage), vec![("The a hits the b for 3.".to_string(), Tones::TEXT)]);
    }

    /// A game that asked for the unseen to be narrated asked for it named.
    ///
    /// The two settings meet here: `who_seen` is the fact that the shooter
    /// was out of sight, and `speak_unseen` is the game saying it wants
    /// those lines anyway. Hiding the name as well would leave it with
    /// "Something shoots you" and no way to get the name back.
    #[test]
    fn speak_unseen_names_the_shooter_that_would_otherwise_be_something() {
        let mut stage = Stage::new(NarratorPlugin::default());
        let (player, kind) = (stage.player, stage.kind);
        let hidden = stage.actor("line droid", 'd', 40, 40);
        stage.tick();
        stage.app.world_mut().resource_mut::<Phrasebook>().speak_unseen = true;
        stage.app.world_mut().write_message(DamageEvent::arriving(player, atglyph_rules::Hit::by(hidden, kind, 3), DamageRoute::Shot));
        stage.tick();
        assert_eq!(spoken(&stage, &["The line droid", "Something"]), vec!["The line droid shoots you for 3.".to_string()]);
    }

    /// A template fills every placeholder it knows and leaves alone one it
    /// does not.
    #[test]
    fn a_template_fills_its_placeholders_and_capitalises_on_request() {
        let mut said = Said::new(Phrase::YouHit, 0);
        said.amount = 4;
        said.named = "venom".into();
        let mut app = App::new();
        let mut state: bevy::ecs::system::SystemState<Names> = bevy::ecs::system::SystemState::new(app.world_mut());
        let names = state.get(app.world()).expect("every input is optional");
        let (text, spans) = render("{Who} takes {n} from {named} {odd}.", &said, &names, true);
        assert_eq!(text, "Something takes 4 from venom {odd}.");
        assert!(spans.is_empty());
    }

    /// A thing is named once, as one of it, and said with its count: one
    /// with an article, several counted and in the plural.
    #[test]
    fn a_thing_is_said_as_one_with_an_article_and_as_several_counted() {
        let mut app = App::new();
        let one = app.world_mut().spawn((Name::new("pebble"), Stack { key: 1, count: 1 })).id();
        let five = app.world_mut().spawn((Name::new("pebble"), Stack { key: 1, count: 5 })).id();
        let loose = app.world_mut().spawn(Name::new("ember")).id();
        let mut state: bevy::ecs::system::SystemState<Names> = bevy::ecs::system::SystemState::new(app.world_mut());
        let names = state.get(app.world()).expect("every input is optional");
        let say = |what: Entity| {
            let mut said = Said::new(Phrase::YouThrow, 0);
            said.what = Some(what);
            render("You throw {what}.", &said, &names, true).0
        };
        assert_eq!(say(one), "You throw a pebble.");
        assert_eq!(say(five), "You throw 5 pebbles.");
        assert_eq!(say(loose), "You throw an ember.", "no stack is one");
    }

    /// Several of a thing whose proper name runs through a connective of
    /// its owner's are counted by what is owned: three of the queen's
    /// crowns, not three queens.
    #[test]
    fn several_of_a_proper_name_through_a_connective_count_the_word_that_counts() {
        let mut app = App::new();
        let crowns = app.world_mut().spawn((Name::new("the Queen of Thorns's Crown"), Stack { key: 1, count: 3 })).id();
        let one = app.world_mut().spawn(Name::new("the Queen of Thorns's Crown")).id();
        let mut state: bevy::ecs::system::SystemState<Names> = bevy::ecs::system::SystemState::new(app.world_mut());
        let names = state.get(app.world()).expect("every input is optional");
        let say = |what: Entity, template: &str| {
            let mut said = Said::new(Phrase::YouDrop, 0);
            said.what = Some(what);
            render(template, &said, &names, false).0
        };
        assert_eq!(say(crowns, "You throw {what}."), "You throw 3 Queen of Thorns's Crowns.");
        assert_eq!(say(crowns, "You drop {your what}."), "You drop your 3 Queen of Thorns's Crowns.");
        assert_eq!(say(one, "You drop {your what}."), "You drop the Queen of Thorns's Crown.");
    }

    /// A game's answer to every blow, told in `TurnSet::Listen`: the last
    /// phase a game's reaction can land in before the record is read, so a
    /// collector that ran any earlier in the pass would miss it until the
    /// next pass, whatever order the executor chose. Otherwise the way a
    /// game answers what a pass did.
    fn answer_blows(mut dealt: MessageReader<DamageDealt>, mut tell: MessageWriter<Tell>) {
        for d in dealt.read() {
            tell.write(Tell::new("Struck: {whom}.", Tones::MUTED).to(d.target));
        }
    }

    /// A stage with consumables and throwing, and `triggers` built for a
    /// thing to carry.
    fn with_things(triggers: &str) -> (Stage, atglyph_bevy::Triggers) {
        let stage = Stage::new_with((NarratorPlugin::default(), atglyph_bevy::EffectsPlugin, atglyph_bevy::ThrowingPlugin), |app| {
            app.add_engine_effects();
        });
        let specs: Vec<atglyph_rules::TriggerSpec> = atglyph_rules::Names::new().load_list(triggers).expect("the triggers parse");
        let built = {
            let world = stage.app.world();
            let registries = world.resource::<Registries>().clone();
            atglyph_bevy::Triggers::build(
                &specs,
                &[],
                world.resource::<atglyph_bevy::TriggerKinds>(),
                world.resource::<atglyph_bevy::EffectKinds>(),
                &registries.names(),
            )
            .expect("the triggers build")
        };
        (stage, built)
    }

    /// Using a thing that does something is said, as one of it, whether it
    /// was the last of its stack and is gone by the end of the pass or one
    /// of three and two are left.
    #[test]
    fn using_a_thing_says_so_as_one_of_it_last_of_its_stack_or_not() {
        let (mut stage, triggers) = with_things(r#"[(on: "use", effects: [(kind: "Mend", args: (roll: "1"))])]"#);
        let player = stage.player;
        let last = stage.app.world_mut().spawn((Item, Name::new("stim"), triggers.clone(), Consumable::new(1, WhenEmpty::Destroyed))).id();
        let three =
            stage.app.world_mut().spawn((Item, Name::new("ration"), triggers, Consumable::new(1, WhenEmpty::Destroyed), Stack { key: 2, count: 3 })).id();
        stage.app.world_mut().entity_mut(player).insert(Inventory { items: vec![last, three] });
        stage.tick();
        stage.app.world_mut().write_message(Intent::new(player, UseItem(last)));
        stage.tick();
        stage.app.world_mut().write_message(Intent::new(player, UseItem(three)));
        stage.tick();
        assert_eq!(spoken(&stage, &["You use"]), ["You use a stim.", "You use a ration."]);
        assert!(stage.app.world().get_entity(last).is_err(), "the last stim is gone");
    }

    /// The last of a thing, used up, is named in the log in its own colour,
    /// as it was named when it was picked up, though it is gone from the
    /// bag the moment the pass ends.
    #[test]
    fn the_last_of_a_thing_used_up_is_named_in_its_own_colour() {
        let (mut stage, triggers) = with_things(r#"[(on: "use", effects: [(kind: "Mend", args: (roll: "1"))])]"#);
        let player = stage.player;
        let green = Color::srgb(0.2, 0.9, 0.4);
        let last = stage
            .app
            .world_mut()
            .spawn((Item, Name::new("stim"), atglyph_render::Glyph::new('!', green), triggers, Consumable::new(1, WhenEmpty::Destroyed)))
            .id();
        stage.app.world_mut().entity_mut(player).insert(Inventory { items: vec![last] });
        stage.tick();
        stage.app.world_mut().write_message(Intent::new(player, UseItem(last)));
        stage.tick();
        let entry = stage.app.world().resource::<MessageLog>().iter().find(|e| e.text == "You use a stim.").cloned().expect("the use is said");
        assert_eq!(entry.spans.iter().map(|s| (s.start, s.len, s.color)).collect::<Vec<_>>(), [(8, 6, green)], "'a stim' in its green: {entry:?}");
        assert!(stage.app.world().get::<Inventory>(player).unwrap().items.is_empty(), "and it is out of the bag");
    }

    /// A thing thrown and spent where it lands, a grenade, is named as
    /// what it was, not as something nobody could make out.
    #[test]
    fn a_thing_spent_where_it_lands_is_named_when_it_is_thrown() {
        let (mut stage, triggers) = with_things(r#"[(on: "land", effects: [(kind: "Mend", args: (roll: "1"))])]"#);
        let player = stage.player;
        let grenade =
            stage.app.world_mut().spawn((Item, Name::new("frag grenade"), triggers, Consumable::new(1, WhenEmpty::Destroyed), Throwable::new(6, None))).id();
        stage.app.world_mut().entity_mut(player).insert(Inventory { items: vec![grenade] });
        stage.tick();
        let at = stage.at.offset(3, 0);
        stage.app.world_mut().write_message(Intent::new(player, Throw { item: grenade, at }));
        stage.tick();
        assert_eq!(spoken(&stage, &["You throw"]), ["You throw a frag grenade."]);
    }

    /// The lines a test cares about, oldest first.
    fn spoken(stage: &Stage, starts: &[&str]) -> Vec<String> {
        lines(stage).into_iter().map(|(t, _)| t).filter(|t| starts.iter().any(|s| t.starts_with(s))).collect()
    }

    /// What an actor's `died` trigger lands after its body is gone is said
    /// as that actor's own harm, by its name in its own colour, as a living
    /// one's would be, and not as harm from something unseen.
    #[test]
    fn harm_a_dead_actors_burst_lands_is_said_in_its_name_and_its_colour() {
        let (mut stage, triggers) = with_things(r#"[(on: "died", area: Burst(radius: 1), effects: [(kind: "Harm", args: (kind: "kinetic", roll: "3"))])]"#);
        let player = stage.player;
        let tan = Color::srgb(0.8, 0.6, 0.3);
        let puffball = stage.actor("puffball", 'f', 1, 0);
        stage.app.world_mut().entity_mut(puffball).insert((triggers, Health::full(1), atglyph_render::Glyph::new('f', tan).on_layer(5)));
        stage.tick();
        stage.app.world_mut().write_message(Intent::new(player, Attack(puffball)));
        stage.tick();
        for _ in 0..2 {
            stage.app.world_mut().write_message(Intent::new(player, atglyph_bevy::Wait));
            stage.tick();
        }
        let said = spoken(&stage, &["The puffball", "Something"]);
        assert_eq!(said, vec!["The puffball hits you for 3.".to_string()], "{:?}", lines(&stage));
        let entry = stage.app.world().resource::<MessageLog>().iter().find(|e| e.text.starts_with("The puffball hits you")).expect("the line");
        assert_eq!(entry.spans.len(), 1, "one name coloured: {entry:?}");
        assert_eq!((entry.spans[0].start, entry.spans[0].len, entry.spans[0].color), (0, 12, tan), "'The puffball' in its own colour: {entry:?}");
    }

    /// A worn thing that will not come off says so, in words and as bad
    /// news, when the player's try to take it off came to nothing.
    #[test]
    fn a_thing_that_will_not_come_off_says_so() {
        let mut stage = Stage::new(NarratorPlugin::default());
        let player = stage.player;
        let finger = atglyph_rules::EquipShape::in_slot(atglyph_rules::SlotId::from_raw(0));
        let ring = stage.app.world_mut().spawn((Item, Name::new("ring"), Wearable(finger.clone()), Stuck)).id();
        let mut worn = Equipped(atglyph_rules::Equipment::with_slot_count(1));
        worn.equip(ring, &finger).expect("the slot exists");
        stage.app.world_mut().entity_mut(player).insert((Inventory { items: vec![ring] }, worn));
        stage.tick();
        stage.app.world_mut().write_message(Intent::new(player, Unequip(ring)));
        stage.tick();
        let said = lines(&stage);
        assert!(said.contains(&("Your ring will not come off.".to_string(), Tones::BAD)), "{said:?}");
    }

    /// What the player puts on, takes off and drops is said as theirs, and
    /// what they pick up with the article its name takes: none for a stuff,
    /// so chain mail is never `a chain mail`, and `a` for a thing counted
    /// one at a time.
    #[test]
    fn the_players_own_things_are_said_as_theirs_and_a_stuff_takes_no_article() {
        let mut stage = Stage::new(NarratorPlugin::default());
        let player = stage.player;
        let body = atglyph_rules::EquipShape::in_slot(atglyph_rules::SlotId::from_raw(0));
        let mail = stage.app.world_mut().spawn((Item, Name::new("chain mail"), MassNoun, Wearable(body))).id();
        let here = stage.at;
        stage.app.world_mut().entity_mut(player).insert((Inventory { items: vec![mail] }, Equipped(atglyph_rules::Equipment::with_slot_count(1))));
        stage.tick();
        stage.app.world_mut().write_message(Intent::new(player, Equip(mail)));
        stage.tick();
        stage.app.world_mut().write_message(Intent::new(player, Unequip(mail)));
        stage.tick();
        stage.app.world_mut().write_message(Intent::new(player, DropItem(mail)));
        stage.tick();
        stage.app.world_mut().write_message(Intent::new(player, PickUp));
        stage.tick();
        let theirs = ["You put on your chain mail.", "You take off your chain mail.", "You drop your chain mail.", "You pick up chain mail."];
        assert_eq!(spoken(&stage, &["You put", "You take", "You drop", "You pick"]), theirs);

        let sword = stage.app.world_mut().spawn((Item, Name::new("sword"), Position(here))).id();
        stage.app.world_mut().write_message(Intent::new(player, PickUp));
        stage.tick();
        assert_eq!(spoken(&stage, &["You pick up a"]), ["You pick up a sword."], "a thing counted one at a time takes an article");
        stage.app.world_mut().write_message(Intent::new(player, DropItem(sword)));
        stage.tick();
        assert_eq!(spoken(&stage, &["You drop your s"]), ["You drop your sword."]);
    }

    /// A thing with a proper name is said as written in every line about
    /// the player's own, put on, taken off, dropped and stuck fast, and
    /// when picked up: never `your Crown of Grubnak`, which read as though
    /// the name were a kind and the player held one of them.
    #[test]
    fn a_thing_with_a_proper_name_is_said_as_written_and_never_as_yours() {
        let mut stage = Stage::new(NarratorPlugin::default());
        let player = stage.player;
        let head = atglyph_rules::EquipShape::in_slot(atglyph_rules::SlotId::from_raw(0));
        let crown = stage.app.world_mut().spawn((Item, Name::new("the Crown of Grubnak"), Wearable(head))).id();
        stage.app.world_mut().entity_mut(player).insert((Inventory { items: vec![crown] }, Equipped(atglyph_rules::Equipment::with_slot_count(1))));
        stage.tick();
        stage.app.world_mut().write_message(Intent::new(player, Equip(crown)));
        stage.tick();
        stage.app.world_mut().write_message(Intent::new(player, Unequip(crown)));
        stage.tick();
        stage.app.world_mut().write_message(Intent::new(player, DropItem(crown)));
        stage.tick();
        stage.app.world_mut().write_message(Intent::new(player, PickUp));
        stage.tick();
        stage.app.world_mut().write_message(Intent::new(player, Equip(crown)));
        stage.tick();
        stage.app.world_mut().entity_mut(crown).insert(Stuck);
        stage.app.world_mut().write_message(Intent::new(player, Unequip(crown)));
        stage.tick();
        let said = spoken(&stage, &["You", "The Crown"]);
        assert_eq!(
            said,
            [
                "You put on the Crown of Grubnak.",
                "You take off the Crown of Grubnak.",
                "You drop the Crown of Grubnak.",
                "You pick up the Crown of Grubnak.",
                "You put on the Crown of Grubnak.",
                "The Crown of Grubnak will not come off.",
            ],
            "{said:#?}"
        );
    }

    /// The player strikes a slime and the slime's mind strikes back in the
    /// next pass of the same frame; the game answers each blow with a line
    /// of its own, and every line lands under the blow it answers and over
    /// the blow that came after.
    #[test]
    fn a_games_line_is_spoken_after_the_event_it_answers_in_one_pass_and_before_the_next_pass() {
        let mut stage = Stage::new((NarratorPlugin::default(), MindsPlugin));
        stage.app.add_systems(Turn, answer_blows.in_set(TurnSet::Listen));
        let (player, kind, theirs) = (stage.player, stage.kind, stage.theirs);
        let brain = std::sync::Arc::new(atglyph_rules::Brain::new().then(atglyph_rules::ai::tactics::MeleeAdjacent));
        let slime = stage
            .app
            .world_mut()
            .spawn((
                (Actor, Blocks, Position(stage.at.offset(1, 0)), Health::full(40), Faction(theirs), Perception(6)),
                (MeleeAttack::new(kind, DiceRoll::flat(2)), Mind(brain), Name::new("slime")),
            ))
            .id();
        stage.tick();
        stage.app.world_mut().write_message(Intent::new(player, Attack(slime)));
        stage.tick();
        stage.tick();
        let said = spoken(&stage, &["You hit", "The slime hits", "Struck:"]);
        assert!(said.len() >= 4, "{said:#?}");
        assert!(said[0].starts_with("You hit the slime for "), "{said:#?}");
        assert_eq!(said[1], "Struck: the slime.", "{said:#?}");
        assert_eq!(said[2], "The slime hits you for 2.", "{said:#?}");
        assert_eq!(said[3], "Struck: you.", "{said:#?}");
    }

    /// A mind that notices the player and strikes in one turn is heard to
    /// notice first: it looked round before it acted, whatever order the
    /// narrator reads the kinds of event in.
    #[test]
    fn a_mind_that_notices_the_player_and_strikes_in_one_turn_is_heard_noticing_first() {
        let mut stage = Stage::new((NarratorPlugin::default(), MindsPlugin, StealthPlugin));
        let (player, kind, theirs) = (stage.player, stage.kind, stage.theirs);
        stage.app.world_mut().entity_mut(player).insert(Stealth(atglyph_rules::StealthStats::default()));
        let brain = std::sync::Arc::new(atglyph_rules::Brain::new().then(atglyph_rules::ai::tactics::MeleeAdjacent));
        stage.app.world_mut().spawn((
            (Actor, Blocks, Position(stage.at.offset(1, 0)), Health::full(40), Faction(theirs), Perception(6)),
            (MeleeAttack::new(kind, DiceRoll::flat(2)), Mind(brain), Name::new("slime")),
            (Notice(atglyph_rules::NoticeStats::default()), Aware::default()),
        ));
        stage.tick();
        stage.app.world_mut().write_message(Intent::new(player, Wait));
        stage.tick();
        stage.tick();
        let said = spoken(&stage, &["The slime"]);
        assert_eq!(said.first().map(String::as_str), Some("The slime notices you."), "{said:#?}");
        assert!(said.iter().any(|l| l == "The slime hits you for 2."), "{said:#?}");
    }

    /// A mind seen using what it carries is told of by the line the
    /// player's own use is told by, with the mind named as the one using it
    /// and never as "you".
    #[test]
    fn a_mind_seen_using_what_it_carries_is_named_as_the_one_using_it() {
        let mut stage = Stage::new_with((NarratorPlugin::default(), atglyph_bevy::EffectsPlugin, MindsPlugin), |app| {
            app.add_engine_effects();
        });
        let specs: Vec<atglyph_rules::TriggerSpec> =
            atglyph_rules::Names::new().load_list(r#"[(on: "use", effects: [(kind: "Mend", args: (roll: "1"))])]"#).expect("the triggers parse");
        let triggers = {
            let world = stage.app.world();
            let registries = world.resource::<Registries>().clone();
            atglyph_bevy::Triggers::build(
                &specs,
                &[],
                world.resource::<atglyph_bevy::TriggerKinds>(),
                world.resource::<atglyph_bevy::EffectKinds>(),
                &registries.names(),
            )
            .expect("the triggers build")
        };
        let (player, theirs) = (stage.player, stage.theirs);
        let stim = stage.app.world_mut().spawn((Item, Name::new("stim"), triggers, Consumable::new(1, WhenEmpty::Destroyed))).id();
        let brain = std::sync::Arc::new(atglyph_rules::Brain::new().then(atglyph_rules::ai::tactics::UseCarried::default()));
        stage.app.world_mut().spawn((
            (Actor, Blocks, Position(stage.at.offset(2, 0)), Health { current: 2, max: 10 }, Faction(theirs), Perception(6)),
            (Mind(brain), Name::new("goblin"), Inventory { items: vec![stim] }),
        ));
        stage.tick();
        stage.app.world_mut().write_message(Intent::new(player, Wait));
        stage.tick();
        stage.tick();
        assert_eq!(spoken(&stage, &["The goblin uses", "You use"]), ["The goblin uses a stim."]);
    }

    /// A shot is not a blow. The droid fires from four tiles off, in plain
    /// sight, and the log says so rather than saying it hit you.
    #[test]
    fn a_shot_is_narrated_as_a_shot_and_a_blow_as_a_blow() {
        let mut stage = Stage::new(NarratorPlugin::default());
        let (player, kind) = (stage.player, stage.kind);
        let droid = stage.actor("line droid", 'd', 4, 0);
        stage.tick();
        // The two halves of the same pass, as the resolver writes them.
        let world = stage.app.world_mut();
        world.write_message(DamageEvent::arriving(player, atglyph_rules::Hit::by(droid, kind, 3), DamageRoute::Shot));
        world.write_message(DamageEvent::arriving(player, atglyph_rules::Hit::by(droid, kind, 2), DamageRoute::Melee));
        stage.tick();

        let said = spoken(&stage, &["The line droid"]);
        assert_eq!(said[0], "The line droid shoots you for 3.", "{said:#?}");
        assert_eq!(said[1], "The line droid hits you for 2.", "a blow still reads as one: {said:#?}");
    }

    /// A miss is said from where the player stands, and the droid is named
    /// the way a hit names it.
    #[test]
    fn a_miss_is_said_from_where_the_player_stands() {
        let mut stage = Stage::new(NarratorPlugin::default());
        let player = stage.player;
        let droid = stage.actor("line droid", 'd', 4, 0);
        stage.tick();
        let world = stage.app.world_mut();
        world.write_message(Missed { attacker: player, target: droid, with: None, reach: DamageRoute::Shot });
        world.write_message(Missed { attacker: droid, target: player, with: None, reach: DamageRoute::Shot });
        stage.tick();
        assert_eq!(spoken(&stage, &["You miss"]), vec!["You miss the line droid."]);
        assert_eq!(spoken(&stage, &["The line droid"]), vec!["The line droid misses you."]);
    }

    /// Something that did not travel as a weapon, and names nothing that
    /// landed it, keeps the blow wording: a poison ticking is not a shot,
    /// and whatever inflicted it has already said its own line.
    #[test]
    fn damage_that_was_never_a_weapon_is_still_narrated_as_a_blow() {
        let mut stage = Stage::new(NarratorPlugin::default());
        let (player, kind) = (stage.player, stage.kind);
        let droid = stage.actor("line droid", 'd', 4, 0);
        stage.tick();
        stage.app.world_mut().write_message(DamageEvent::new(player, atglyph_rules::Hit::by(droid, kind, 3)));
        stage.tick();
        assert_eq!(spoken(&stage, &["The line droid"])[0], "The line droid hits you for 3.");
    }

    /// A reflected landing is said before the harm it caused, and from
    /// whoever it turned back onto: onto you, onto someone else by your
    /// own hand, and between two others besides you.
    #[test]
    fn a_reflected_landing_is_said_before_the_harm_it_caused_from_whoever_it_lands_on() {
        let mut stage = Stage::new(NarratorPlugin::default());
        let (player, kind) = (stage.player, stage.kind);
        let droid = stage.actor("line droid", 'd', 4, 0);
        let rat = stage.actor("rat", 'r', -4, 0);
        stage.tick();

        stage.app.world_mut().write_message(Reflected { by: droid, onto: player, source: atglyph_bevy::Source::Offer(droid) });
        stage.app.world_mut().write_message(DamageEvent::new(player, atglyph_rules::Hit::by(droid, kind, 3)));
        stage.tick();
        let said = spoken(&stage, &["The line droid"]);
        assert_eq!(said[0], "The line droid turns it back on you.", "said before the harm it caused: {said:#?}");
        assert_eq!(said[1], "The line droid hits you for 3.", "{said:#?}");
        assert_eq!(lines(&stage).into_iter().find(|(t, _)| t == &said[0]).unwrap().1, Tones::BAD);

        stage.app.world_mut().write_message(Reflected { by: player, onto: rat, source: atglyph_bevy::Source::Offer(player) });
        stage.tick();
        let said: Vec<String> = lines(&stage).into_iter().map(|(t, _)| t).collect();
        assert!(said.contains(&"You turn it back on the rat.".to_string()), "{said:?}");

        stage.app.world_mut().write_message(Reflected { by: droid, onto: rat, source: atglyph_bevy::Source::Offer(droid) });
        stage.tick();
        let said: Vec<String> = lines(&stage).into_iter().map(|(t, _)| t).collect();
        assert!(said.contains(&"The line droid turns it back on the rat.".to_string()), "{said:?}");
    }

    /// A use aimed at the ground that caught one is told without whom,
    /// since the cell is what it was aimed at, one that caught several
    /// still counts them, and one aimed at someone names them: "The rat
    /// uses burst.", not "The rat uses burst on you.", where the burst was
    /// a step through the air that landed beside the player.
    #[test]
    fn a_use_aimed_at_the_ground_names_nobody_and_one_aimed_at_someone_names_them() {
        use crate::view::target::harness::abilities;
        use atglyph_bevy::{AbilitiesPlugin, AbilityEvent, AddEngineEffects};
        let mut stage = Stage::new_with((AbilitiesPlugin, atglyph_bevy::EffectsPlugin, NarratorPlugin::default()), |app| {
            app.add_engine_effects();
            abilities(app);
        });
        let (bolt, burst) = {
            let a = stage.app.world().resource::<Abilities>();
            (a.expect("bolt"), a.expect("burst"))
        };
        let (player, at) = (stage.player, stage.at);
        let rat = stage.actor("rat", 'r', 3, 0);
        stage.tick();
        let other = stage.actor("bat", 'b', 3, 1);
        stage.tick();
        for (user, ability, whom) in [(rat, burst, vec![player]), (rat, bolt, vec![player]), (player, burst, vec![rat]), (player, burst, vec![rat, other])] {
            stage.app.world_mut().write_message(AbilityEvent::Used { user, ability, aim: at, targets: whom });
        }
        stage.tick();
        let said = spoken(&stage, &["The rat uses", "You use"]);
        assert_eq!(said, ["The rat uses burst.", "The rat uses bolt on you.", "You use burst.", "You use burst, catching 2."], "{said:#?}");
    }

    /// A bolt from a thing used, or from an ability, is not a blow: the
    /// line names what landed it, as "You hit the rat" for a sword does
    /// not, so a log of a zapped rod reads as one.
    #[test]
    fn harm_a_use_or_an_ability_landed_names_what_landed_it_rather_than_reading_as_a_blow() {
        use crate::view::target::harness::{abilities, arm};
        use atglyph_bevy::{AbilitiesPlugin, AddEngineEffects, EffectKinds, Use};
        let mut stage = Stage::new_with((AbilitiesPlugin, atglyph_bevy::EffectsPlugin, NarratorPlugin::default()), |app| {
            app.add_engine_effects();
            abilities(app);
        });
        let (bolt, _, _) = arm(&mut stage);
        let specs: Vec<atglyph_rules::TriggerSpec> = atglyph_rules::Names::new()
            .load_list(r#"[(on: "use", targeting: (mode: Bolt(range: 6)), effects: [(kind: "Harm", args: (kind: "kinetic", roll: "4"))])]"#)
            .expect("the trigger parses");
        let triggers = {
            let world = stage.app.world();
            let names = world.resource::<Registries>().names();
            atglyph_bevy::Triggers::build(&specs, &[], world.resource::<atglyph_bevy::TriggerKinds>(), world.resource::<EffectKinds>(), &names)
                .expect("it builds")
        };
        let (player, kind) = (stage.player, stage.kind);
        let rod = stage.app.world_mut().spawn((Item, Name::new("ash rod"), triggers)).id();
        stage.app.world_mut().get_mut::<Inventory>(player).unwrap().items.push(rod);
        let rat = stage.actor("rat", 'r', 3, 0);
        stage.tick();
        let at = stage.at.offset(3, 0);
        stage.app.world_mut().write_message(Intent::new(player, UseItemAt { item: rod, at }));
        stage.tick();
        stage.app.world_mut().write_message(Intent::new(player, Use { ability: bolt, aim: at }));
        stage.tick();
        assert_eq!(stage.app.world().get::<Health>(rat).map(|h| h.current), Some(10 - 4 - 3), "both landed");
        let mine = spoken(&stage, &["You", "Your"]);
        assert!(mine.contains(&"Your ash rod hits the rat for 4.".to_string()), "the rod, named: {mine:#?}");
        assert!(mine.contains(&"Your bolt hits the rat for 3.".to_string()), "the ability, named: {mine:#?}");
        assert!(!mine.iter().any(|l| l.starts_with("You hit")), "neither reads as a blow: {mine:#?}");
        assert!(!mine.iter().any(|l| l.starts_with("You use bolt")), "the bolt is named once, by what it did: {mine:#?}");

        // The other perspectives, as the pipeline reports them.
        let world = stage.app.world_mut();
        world.write_message(DamageEvent::arriving(player, atglyph_rules::Hit::by(rat, kind, 2), DamageRoute::Ability(bolt)));
        world.write_message(DamageEvent::arriving(player, atglyph_rules::Hit::by(rat, kind, 0), DamageRoute::Thing(rod)));
        stage.tick();
        let theirs = spoken(&stage, &["The rat"]);
        assert_eq!(theirs, ["The rat's bolt hits you for 2.", "The rat's ash rod hits you, to no effect."], "{theirs:#?}");
    }

    /// An ability is named once: a bolt that struck is told by the harm it
    /// did, and a burst that came down on nobody, harming nothing, by its
    /// use, since that is the only line it has. The same pair from the
    /// rat's side reads the same way.
    #[test]
    fn a_use_that_harms_is_told_by_its_harm_alone_and_one_that_harms_nothing_by_its_use() {
        use crate::view::target::harness::{abilities, arm};
        use atglyph_bevy::{AbilitiesPlugin, AbilityEvent, AddEngineEffects, Use};
        let mut stage = Stage::new_with((AbilitiesPlugin, atglyph_bevy::EffectsPlugin, NarratorPlugin::default()), |app| {
            app.add_engine_effects();
            abilities(app);
        });
        let (bolt, burst, _) = arm(&mut stage);
        let (player, kind) = (stage.player, stage.kind);
        let rat = stage.actor("rat", 'r', 3, 0);
        stage.tick();
        stage.app.world_mut().write_message(Intent::new(player, Use { ability: bolt, aim: stage.at.offset(3, 0) }));
        stage.tick();
        stage.app.world_mut().write_message(Intent::new(player, Use { ability: burst, aim: stage.at.offset(0, -4) }));
        stage.tick();
        assert_eq!(spoken(&stage, &["You", "Your"]), ["Your bolt hits the rat for 3.", "You use burst."], "{:#?}", lines(&stage));

        // The rat's side, as the pipeline reports a use and the harm it
        // landed in one pass.
        let world = stage.app.world_mut();
        world.write_message(AbilityEvent::Used { user: rat, ability: bolt, aim: stage.at, targets: vec![player] });
        world.write_message(DamageEvent::arriving(player, atglyph_rules::Hit::by(rat, kind, 2), DamageRoute::Ability(bolt)));
        stage.tick();
        assert_eq!(spoken(&stage, &["The rat"]), ["The rat's bolt hits you for 2."], "{:#?}", lines(&stage));
    }

    /// A heal is told as a mend when it restored something and not at all
    /// when it found its target whole, in either direction, and never as a
    /// blow to no effect: it is not a hit of any kind.
    #[test]
    fn a_heal_is_spoken_as_a_mend_when_it_restored_something_and_never_as_a_blow() {
        let mut stage = Stage::new(NarratorPlugin::default());
        let player = stage.player;
        let droid = stage.actor("line droid", 'd', 1, 0);
        stage.tick();
        let world = stage.app.world_mut();
        world.write_message(Heal::new(droid, 3).by(player));
        world.write_message(Heal::new(player, 3).by(droid));
        stage.tick();
        let said = spoken(&stage, &["You", "The line droid"]);
        assert!(said.is_empty(), "a heal that restored nothing is not a line: {said:#?}");

        let world = stage.app.world_mut();
        world.get_mut::<Health>(player).unwrap().current -= 5;
        world.write_message(Heal::new(player, 3).by(droid));
        stage.tick();
        assert_eq!(spoken(&stage, &["You", "The line droid"]), vec!["You mend for 3."]);
    }

    /// A status is told in the words its name is written for, "You are
    /// warded." and "You are no longer warded.", however it ends; and in
    /// the tone of what it is to the player: an affliction landing is bad
    /// news and its cure good, a boon landing is good news and its end, run
    /// out or broken off, only a note.
    #[test]
    fn a_status_on_the_player_is_told_as_good_or_bad_news_by_whether_it_is_a_boon() {
        let mut stage = Stage::new(NarratorPlugin::default());
        let defs = atglyph_rules::Registry::from_defs(vec![atglyph_rules::StatusDef::new("warded").boon(), atglyph_rules::StatusDef::new("scorched")]).unwrap();
        let (warded, scorched) = (defs.expect("warded"), defs.expect("scorched"));
        stage.app.world_mut().resource_mut::<Registries>().statuses = defs;
        let target = stage.player;
        for ev in [
            StatusEvent::Applied { target, status: warded },
            StatusEvent::Cured { target, status: warded },
            StatusEvent::Applied { target, status: scorched },
            StatusEvent::Cured { target, status: scorched },
            StatusEvent::Applied { target, status: warded },
            StatusEvent::Expired { target, status: warded },
        ] {
            stage.app.world_mut().write_message(ev);
            stage.tick();
        }
        let said: Vec<(String, ToneId)> = lines(&stage).into_iter().filter(|(t, _)| t.starts_with("You are")).collect();
        let expected = [
            ("You are warded.", Tones::GOOD),
            ("You are no longer warded.", Tones::MUTED),
            ("You are scorched.", Tones::BAD),
            ("You are no longer scorched.", Tones::GOOD),
            ("You are warded.", Tones::GOOD),
            ("You are no longer warded.", Tones::MUTED),
        ];
        assert_eq!(said, expected.map(|(t, tone)| (t.to_string(), tone)).to_vec());
    }

    /// A cloud keeps its status on whoever breathes it by putting it on
    /// again every turn, and the status is told when it begins, not every
    /// turn it is kept up: three turns in the cloud say it once, where the
    /// log used to read "The rat is paralysed. (x3)".
    #[test]
    fn three_turns_in_a_cloud_say_its_status_once() {
        let mut stage = Stage::new_with((atglyph_bevy::GasPlugin, NarratorPlugin::default()), |app| {
            let statuses = atglyph_rules::Registry::from_defs(vec![atglyph_rules::StatusDef::new("paralysed")]).unwrap();
            let spores = atglyph_rules::gas::GasDef::new("spores").spread(0).fade(0).inflicts(1, statuses.expect("paralysed"), 3);
            let mut registries = app.world_mut().resource_mut::<Registries>();
            registries.statuses = statuses;
            registries.gases = atglyph_rules::Registry::from_defs(vec![spores]).unwrap();
        });
        let rat = stage.actor("rat", 'r', 2, 0);
        let spores = stage.app.world().resource::<Registries>().gases.expect("spores");
        let at = stage.at.offset(2, 0);
        stage.app.world_mut().write_message(atglyph_bevy::Release { gas: spores, at, amount: 255 });
        let player = stage.player;
        for _ in 0..3 {
            stage.app.world_mut().write_message(Intent::new(player, Wait));
            stage.tick();
        }
        assert!(stage.app.world().resource::<atglyph_bevy::Gases>().at(spores, at) > 0, "the cloud stayed");
        let paralysed = stage.app.world().resource::<Registries>().statuses.expect("paralysed");
        assert!(stage.app.world().get::<Afflicted>(rat).is_some_and(|a| a.has(paralysed)), "and kept the rat paralysed");
        let told: Vec<String> = stage.app.world().resource::<MessageLog>().iter().map(|e| e.display()).filter(|t| t.contains("paralysed")).collect();
        assert_eq!(told, ["The rat is paralysed."], "said once, when it began");
    }

    /// A shooter the player cannot see is not named, even though the line
    /// is still spoken: being shot from the dark is worth telling you, and
    /// naming the thing that did it is not something the player could know.
    #[test]
    fn a_shot_from_out_of_sight_is_spoken_without_naming_the_shooter() {
        let mut stage = Stage::new(NarratorPlugin::default());
        let (player, kind) = (stage.player, stage.kind);
        // Far outside the player's ten-tile viewshed, so nothing sees it.
        let hidden = stage.actor("line droid", 'd', 40, 40);
        stage.tick();
        assert!(
            !stage.app.world().get::<Viewshed>(player).expect("a viewshed").can_see(stage.at.offset(40, 40)),
            "the shooter has to be out of sight for this to test anything"
        );
        stage.app.world_mut().write_message(DamageEvent::arriving(player, atglyph_rules::Hit::by(hidden, kind, 3), DamageRoute::Shot));
        stage.tick();

        let said = spoken(&stage, &["Something", "The line droid"]);
        assert_eq!(said, vec!["Something shoots you for 3.".to_string()], "spoken, but not named: {said:#?}");
    }

    /// A goblin [`Concealed`] in the dark stands in a cell the player sees
    /// and is not itself seen, so every line that would name it, as the
    /// one who did it or the one it was done to, calls it `something`, as
    /// the map draws nothing there; beside the player it is seen and
    /// named, and a game that speaks the unseen names it anyway.
    #[test]
    fn a_concealed_actor_in_the_dark_is_never_named_by_any_line_until_it_is_seen() {
        let mut stage = Stage::new_with(NarratorPlugin::default(), |app| {
            app.add_plugins(LightingPlugin);
            let player = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).expect("the player");
            app.world_mut().entity_mut(player).insert(DarkSight(6));
        });
        let (player, kind) = (stage.player, stage.kind);
        let rat = stage.actor("rat", 'r', 3, 0);
        let goblin = stage.actor("goblin", 'g', 4, 0);
        stage.app.world_mut().entity_mut(goblin).insert((Concealed, Health::full(40)));
        stage.tick();
        assert!(stage.app.world().get::<Viewshed>(player).expect("a viewshed").can_see(stage.at.offset(4, 0)), "its cell is in sight");

        let world = stage.app.world_mut();
        world.write_message(DamageEvent::new(player, atglyph_rules::Hit::by(goblin, kind, 3)));
        world.write_message(Missed { attacker: goblin, target: player, with: None, reach: DamageRoute::Melee });
        world.write_message(DamageEvent::arriving(goblin, atglyph_rules::Hit::by(player, kind, 3), DamageRoute::Shot));
        world.write_message(DamageEvent::new(rat, atglyph_rules::Hit::by(goblin, kind, 3)));
        stage.tick();
        let left = stage.at.offset(4, 0);
        stage.app.world_mut().write_message(Travelled { actor: goblin, from: MapId::ZERO, left, to: MapId(3), after: None });
        stage.tick();
        let said = spoken(&stage, &["Something", "You", "The goblin"]);
        assert_eq!(
            said,
            ["Something misses you.", "Something hits you for 3.", "You shoot something for 3.", "Something hits the rat for 3.", "Something leaves."],
            "{:#?}",
            lines(&stage)
        );
        assert!(!lines(&stage).iter().any(|(t, _)| t.contains("goblin")), "never named: {:#?}", lines(&stage));

        stage.app.world_mut().entity_mut(goblin).insert(Position(stage.at.offset(1, 0)));
        stage.tick();
        stage.app.world_mut().write_message(DamageEvent::new(player, atglyph_rules::Hit::by(goblin, kind, 3)));
        stage.tick();
        assert!(spoken(&stage, &["The goblin"]).contains(&"The goblin hits you for 3.".to_string()), "beside the player it is seen");

        stage.app.world_mut().entity_mut(goblin).insert(Position(left));
        stage.app.world_mut().resource_mut::<Phrasebook>().speak_unseen = true;
        stage.tick();
        stage.app.world_mut().write_message(DamageEvent::arriving(goblin, atglyph_rules::Hit::by(player, kind, 3), DamageRoute::Shot));
        stage.tick();
        assert!(spoken(&stage, &["You shoot"]).contains(&"You shoot the goblin for 3.".to_string()), "spoken unseen, named: {:#?}", lines(&stage));
    }

    /// Walking into someone whose one offer has a price brings up the
    /// offers screen, so the log does not also call them in the way; one
    /// who offers nothing still is. The fault this guards logged a
    /// keeper as in the way every time the walk key asked to trade.
    #[test]
    fn a_walk_into_someone_with_a_priced_offer_is_not_said_to_be_in_the_way_and_one_with_none_is() {
        let mut stage = Stage::new_with(NarratorPlugin::default(), |app| {
            app.add_plugins(PropsPlugin);
            app.insert_resource(Seed(atglyph_bevy::testing::TEST_SEED));
            app.add_verb("strip");
        });
        let tags = atglyph_rules::Registry::from_defs(vec![atglyph_rules::TagDef::new("scrip")]).expect("one tag");
        stage.app.insert_resource(Currency(tags.expect("scrip")));
        let (player, at) = (stage.player, stage.at);
        let strip = stage.app.world().resource::<Verbs>().get("strip").expect("declared");
        let mut spawn = |name: &'static str, p: Point, offers: Vec<Offering>| {
            stage.app.world_mut().spawn((Actor, Blocks, Position(p), OnMap(MapId::ZERO), Health::full(5), Name::new(name), Offers(offers))).id()
        };
        spawn("clerk", at.offset(1, 0), vec![Offering::new(strip, 100).priced(20)]);
        spawn("loiterer", at.offset(-1, 0), Vec::new());
        stage.tick();
        stage.app.world_mut().write_message(Intent::new(player, Bump(atglyph_core::Direction::East)));
        stage.tick();
        let said: Vec<String> = lines(&stage).into_iter().map(|(text, _)| text).collect();
        assert!(!said.iter().any(|t| t.contains("in the way")), "the clerk is asked about, not in the way: {said:?}");
        stage.app.world_mut().write_message(Intent::new(player, Bump(atglyph_core::Direction::West)));
        stage.tick();
        let said: Vec<String> = lines(&stage).into_iter().map(|(text, _)| text).collect();
        assert!(said.contains(&"The loiterer is in the way.".to_string()), "{said:?}");
    }
}
