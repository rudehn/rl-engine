//! The narrator: what the turns did, in words, in the log.
//!
//! Every game with a log writes the same system: read [`DamageDealt`] and
//! [`DeathEvent`], branch on whether the player did it or had it done to
//! it, pick a tone, push a line. Ten copies of that, each getting the order
//! wrong when two monsters act in one frame and each narrating blows nobody
//! saw. So it is a view, a collector and a presenter, the split every panel
//! has, with one twist in where the collector runs.
//!
//! The view, [`NarrationView`], is rows of [`Said`]: which [`Phrase`], who
//! did it, to whom, with what, how much, where, and whether the player saw
//! it. No string the game did not supply. The collector runs in
//! [`TurnSet::React`], once per pass rather than
//! once per frame, because one pass is one actor's action and reading that
//! pass's events in a fixed order gives the true order across a frame of
//! many turns: the cast, then the blow it landed, then the next actor's
//! step. A collector in the drawing phase sees a whole frame's buffers at
//! once and cannot know which blow followed which cast.
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
//! What using an item means, what a quest said, and how a game's own events
//! read stay the game's: it pushes those lines to the [`MessageLog`] as it
//! always did.

use std::collections::BTreeMap;

use bevy::prelude::*;
use rl_bevy::prelude::*;
use rl_core::Point;
use rl_render::Glyph;

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
    /// Someone killed someone else.
    Kills,
    /// Someone died of no one in particular.
    Dies,
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
    /// You threw something.
    YouThrow,
    /// Someone else did.
    Throws,
    /// You opened a door.
    YouOpen,
    /// You closed one.
    YouClose,
    /// Someone else opened one.
    OpensDoor,
    /// Someone else closed one.
    ClosesDoor,
    /// You walked into someone you would not strike.
    YouBumpInto,
    /// You changed places with someone.
    YouSwapWith,
    /// Someone noticed you.
    NoticesYou,
    /// You used an ability on nobody in particular.
    YouUse,
    /// You used one on someone.
    YouUseOn,
    /// You used one on several.
    YouUseOnMany,
    /// Someone else used one on nobody in particular.
    Uses,
    /// Someone else used one on someone.
    UsesOn,
    /// Someone else used one on several.
    UsesOnMany,
    /// You tried an ability and were refused.
    YouCannotUse,
    /// A status was put on you.
    YouAreAfflicted,
    /// A status on you ran out.
    YouAreNoLonger,
    /// A status on you was cured.
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

/// One thing that happened, as the narrator reads it.
#[derive(Debug, Clone, PartialEq)]
pub struct Said {
    /// What kind of thing.
    pub phrase: Phrase,
    /// Who did it, if anyone: `{who}` in a template.
    pub who: Option<Entity>,
    /// To whom, if anyone: `{whom}`.
    pub whom: Option<Entity>,
    /// With what, if anything: `{what}`, named as the thing is named.
    pub what: Option<Entity>,
    /// A registry's name for the status, kind or ability involved:
    /// `{named}`.
    pub named: String,
    /// More words, where there are any: the reasons a use was refused.
    pub detail: String,
    /// How much: `{n}`.
    pub amount: i32,
    /// Where, when it happened somewhere.
    pub at: Option<Point>,
    /// Whether the player saw it: it happened to the player, or in the
    /// player's sight.
    pub seen: bool,
    /// The whole turn it happened on.
    pub turn: u32,
}

impl Said {
    /// A row with nothing but its phrase and turn.
    pub fn new(phrase: Phrase, turn: u32) -> Self {
        Self { phrase, who: None, whom: None, what: None, named: String::new(), detail: String::new(), amount: 0, at: None, seen: true, turn }
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
            .add_systems(Turn, collect_narration.in_set(TurnSet::React));
    }

    fn finish(&self, app: &mut App) {
        rl_bevy::depends_on::<crate::UiPlugin>(app, "NarrationViewPlugin");
    }
}

/// Every event the narrator reads, in the order it reads them within a
/// pass: a use before the blows it landed, blows before the deaths they
/// caused.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Heard<'w, 's> {
    abilities: MessageReader<'w, 's, AbilityEvent>,
    bumps: MessageReader<'w, 's, Bumped>,
    swaps: MessageReader<'w, 's, Swapped>,
    doors: MessageReader<'w, 's, DoorEvent>,
    items: MessageReader<'w, 's, ItemEvent>,
    dealt: MessageReader<'w, 's, DamageDealt>,
    statuses: MessageReader<'w, 's, StatusEvent>,
    deaths: MessageReader<'w, 's, DeathEvent>,
    noticed: MessageReader<'w, 's, Noticed>,
    lights: MessageReader<'w, 's, LightEvent>,
    fires: MessageReader<'w, 's, FireEvent>,
}

/// Items that strike or shoot when wielded, which are wielded rather than
/// worn.
type Weapons<'w, 's> = Query<'w, 's, (), (With<Item>, Or<(With<MeleeAttack>, With<RangedAttack>)>)>;

/// What the collector reads about the world to fill a row.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Witness<'w, 's> {
    turns: Res<'w, Turns>,
    registries: Option<Res<'w, Registries>>,
    abilities: Option<Res<'w, Abilities>>,
    player: Query<'w, 's, (Entity, &'static Position, &'static Viewshed), With<Player>>,
    positions: Query<'w, 's, &'static Position>,
    weapons: Weapons<'w, 's>,
}

impl Witness<'_, '_> {
    fn is_you(&self, e: Entity) -> bool {
        self.player.single().is_ok_and(|(me, _, _)| me == e)
    }

    fn at(&self, e: Entity) -> Option<Point> {
        self.positions.get(e).ok().map(|p| p.0)
    }

    /// Whether the player saw `at`, or was one of `these`.
    fn seen(&self, at: Option<Point>, these: &[Option<Entity>]) -> bool {
        let Ok((me, my_pos, sight)) = self.player.single() else { return false };
        if these.iter().flatten().any(|e| *e == me) {
            return true;
        }
        at.is_some_and(|p| p == my_pos.0 || sight.can_see(p))
    }

    fn status_name(&self, id: rl_rules::StatusId) -> String {
        self.registries.as_deref().map(|r| r.statuses.name(id).to_string()).unwrap_or_default()
    }

    fn ability_name(&self, id: rl_rules::AbilityId) -> String {
        self.abilities.as_deref().map(|a| a.get(id).name.clone()).unwrap_or_default()
    }
}

/// Fills [`NarrationView`] with what this pass did.
pub fn collect_narration(mut view: ResMut<NarrationView>, mut heard: Heard, witness: Witness) {
    let turn = witness.turns.turn_number();
    let say = |phrase: Phrase, who: Option<Entity>, whom: Option<Entity>| {
        let at = who.and_then(|e| witness.at(e)).or_else(|| whom.and_then(|e| witness.at(e)));
        let mut said = Said::new(phrase, turn);
        said.who = who;
        said.whom = whom;
        said.at = at;
        said.seen = witness.seen(at, &[who, whom]);
        said
    };

    let mut rows: Vec<Said> = Vec::new();
    for ev in heard.abilities.read() {
        match ev {
            AbilityEvent::Used { user, ability, targets, .. } => {
                let you = witness.is_you(*user);
                let others: Vec<Entity> = targets.iter().copied().filter(|t| t != user).collect();
                let (phrase, whom) = match others.as_slice() {
                    [] => (if you { Phrase::YouUse } else { Phrase::Uses }, None),
                    [one] => (if you { Phrase::YouUseOn } else { Phrase::UsesOn }, Some(*one)),
                    _ => (if you { Phrase::YouUseOnMany } else { Phrase::UsesOnMany }, None),
                };
                let mut said = say(phrase, Some(*user), whom);
                said.named = witness.ability_name(*ability);
                said.amount = others.len() as i32;
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
        if witness.is_you(b.actor) {
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
            ItemEvent::Thrown { actor, item, .. } => (actor, item, if witness.is_you(actor) { Phrase::YouThrow } else { Phrase::Throws }),
            // What using an item means is the game's, and so are the words.
            ItemEvent::Used { .. } => continue,
        };
        let mut said = say(phrase, Some(actor), None);
        said.what = Some(item);
        rows.push(said);
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
        let phrase = match (you_attacker, you_target, landed) {
            (true, _, true) => Phrase::YouHit,
            (true, _, false) => Phrase::YouHitNothing,
            (false, true, true) => Phrase::HitsYou,
            (false, true, false) => Phrase::HitsYouForNothing,
            (false, false, true) => Phrase::OthersFight,
            (false, false, false) => Phrase::OthersHitNothing,
        };
        let mut said = say(phrase, d.hit.attacker, Some(target));
        said.amount = d.dealt;
        rows.push(said);
    }
    for ev in heard.statuses.read() {
        let (target, status, phrase) = match *ev {
            StatusEvent::Applied { target, status } => (target, status, if witness.is_you(target) { Phrase::YouAreAfflicted } else { Phrase::IsAfflicted }),
            StatusEvent::Expired { target, status } if witness.is_you(target) => (target, status, Phrase::YouAreNoLonger),
            StatusEvent::Cured { target, status } if witness.is_you(target) => (target, status, Phrase::YourAfflictionPasses),
            _ => continue,
        };
        let mut said = say(phrase, Some(target), None);
        said.named = witness.status_name(status);
        rows.push(said);
    }
    for d in heard.deaths.read() {
        let mut said = match (d.was_player, d.credit) {
            (true, _) => say(Phrase::YouDie, Some(d.entity), None),
            (false, Some(by)) if witness.is_you(by) => say(Phrase::YouKill, Some(by), Some(d.entity)),
            (false, Some(by)) => say(Phrase::Kills, Some(by), Some(d.entity)),
            (false, None) => say(Phrase::Dies, Some(d.entity), None),
        };
        // The dead have left their cell already; the event says where.
        said.at = Some(d.at);
        said.seen = d.was_player || witness.seen(Some(d.at), &[d.credit]);
        rows.push(said);
    }
    for n in heard.noticed.read() {
        if witness.is_you(n.subject) {
            rows.push(say(Phrase::NoticesYou, Some(n.observer), Some(n.subject)));
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
    view.rows.extend(rows);
}

/// One template and the tone it is spoken in, or silence.
type Entry = Option<(String, ToneId)>;

/// The words for every phrase.
///
/// Placeholders: `{who}` and `{whom}` are `you` or `the <Name>`, and
/// `{Who}` and `{Whom}` the same capitalised; `{what}` is the thing's
/// [`Name`] as written, and `{What}` capitalised; `{n}` the amount;
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
        let table: [(Phrase, &str, ToneId); 52] = [
            (YouHit, "You hit {whom} for {n}.", Tones::HIT),
            (YouHitNothing, "You hit {whom}, to no effect.", Tones::MUTED),
            (HitsYou, "{Who} hits you for {n}.", Tones::BAD),
            (HitsYouForNothing, "{Who} hits you, to no effect.", Tones::MUTED),
            (OthersFight, "{Who} hits {whom} for {n}.", Tones::TEXT),
            (OthersHitNothing, "{Who} hits {whom}, to no effect.", Tones::MUTED),
            (YouHurtYourself, "You hurt yourself for {n}.", Tones::BAD),
            (YouMend, "You mend for {n}.", Tones::GOOD),
            (Mends, "{Who} mends for {n}.", Tones::TEXT),
            (YouTakeFromStatus, "You take {n} from {named}.", Tones::BAD),
            (TakesFromStatus, "{Who} takes {n} from {named}.", Tones::TEXT),
            (YouKill, "You kill {whom}!", Tones::KILL),
            (Kills, "{Who} kills {whom}.", Tones::TEXT),
            (Dies, "{Who} dies.", Tones::GOOD),
            (YouDie, "You die.", Tones::BAD),
            (YouPickUp, "You pick up {what}.", Tones::TEXT),
            (PicksUp, "{Who} picks up {what}.", Tones::NOTICE),
            (YouDrop, "You drop {what}.", Tones::TEXT),
            (Drops, "{Who} drops {what}.", Tones::NOTICE),
            (YouWear, "You put on {what}.", Tones::TEXT),
            (Wears, "{Who} puts on {what}.", Tones::NOTICE),
            (YouWield, "You wield {what}.", Tones::TEXT),
            (Wields, "{Who} wields {what}.", Tones::NOTICE),
            (YouTakeOff, "You take off {what}.", Tones::MUTED),
            (TakesOff, "{Who} takes off {what}.", Tones::MUTED),
            (YouThrow, "You throw {what}.", Tones::TEXT),
            (Throws, "{Who} throws {what}.", Tones::NOTICE),
            (YouOpen, "You open the door.", Tones::MUTED),
            (YouClose, "You close the door.", Tones::MUTED),
            (OpensDoor, "{Who} opens a door.", Tones::NOTICE),
            (ClosesDoor, "{Who} closes a door.", Tones::NOTICE),
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
            (YouAreAfflicted, "You are {named}.", Tones::BAD),
            (YouAreNoLonger, "You are no longer {named}.", Tones::MUTED),
            (YourAfflictionPasses, "The {named} passes.", Tones::GOOD),
            (IsAfflicted, "{Who} is {named}.", Tones::NOTICE),
            (YourLightGoesOut, "Your light gutters and goes out.", Tones::BAD),
            (LightGoesOut, "{What} gutters and goes out.", Tones::MUTED),
            (YouStandInFire, "You are standing in fire.", Tones::BAD),
            (StandsInFire, "{Who} is standing in fire.", Tones::TEXT),
            (YouHit, "You hit {whom} for {n}.", Tones::HIT),
            (YouKill, "You kill {whom}!", Tones::KILL),
            (YouDie, "You die.", Tones::BAD),
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
    pub fn silence(&mut self, phrase: Phrase) -> &mut Self {
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

/// What the presenter reads to put names to entities.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Names<'w, 's> {
    player: Query<'w, 's, (), With<Player>>,
    names: Query<'w, 's, (Option<&'static Name>, Option<&'static Glyph>)>,
}

impl Names<'_, '_> {
    /// `you`, or `the <Name>` in the thing's colour, or `something`.
    fn actor(&self, e: Option<Entity>) -> Named {
        let Some(e) = e else { return Named { text: "something".into(), color: None } };
        if self.player.contains(e) {
            return Named { text: "you".into(), color: None };
        }
        match self.names.get(e) {
            Ok((Some(name), glyph)) => Named { text: format!("the {}", name.as_str()), color: glyph.map(|g| g.fg) },
            _ => Named { text: "something".into(), color: None },
        }
    }

    /// The thing's [`Name`] as written, in its colour, or `something`.
    fn thing(&self, e: Option<Entity>) -> Named {
        match e.and_then(|e| self.names.get(e).ok()) {
            Some((Some(name), glyph)) => Named { text: name.as_str().to_string(), color: glyph.map(|g| g.fg) },
            _ => Named { text: "something".into(), color: None },
        }
    }
}

/// Fills `template` for `said`: the text, and a span for each name that
/// has a colour.
pub fn render(template: &str, said: &Said, names: &Names<'_, '_>) -> (String, Vec<Span>) {
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
            "who" => Some(names.actor(said.who)),
            "whom" => Some(names.actor(said.whom)),
            "what" => Some(names.thing(said.what)),
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
pub fn speak(mut view: ResMut<NarrationView>, book: Res<Phrasebook>, names: Names, mut log: ResMut<MessageLog>) {
    for said in view.rows.drain(..) {
        if !said.seen && !book.speak_unseen {
            continue;
        }
        let Some((template, tone)) = book.get(said.phrase) else { continue };
        let (text, spans) = render(template, &said, &names);
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
        app.insert_resource(self.book.clone()).add_systems(Update, speak.in_set(crate::ViewSet::Speak));
    }

    fn finish(&self, app: &mut App) {
        rl_bevy::depends_on::<crate::UiPlugin>(app, "NarratorPlugin");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::Stage;
    use rl_core::DiceRoll;

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
                MeleeAttack { kind, dice: DiceRoll::flat(2), cost: None },
                Name::new("slime"),
                rl_render::Glyph::new('s', green).on_layer(5),
            ))
            .id();
        stage.tick();
        stage.app.world_mut().write_message(Intent::new(player, Attack(slime)));
        stage.tick();
        // The harness has no minds, so the slime strikes back as a blow
        // written for it: the same event a mind's attack lands as.
        stage.app.world_mut().write_message(DamageEvent { target: player, hit: rl_rules::Hit::by(slime, kind, 2) });
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
            .spawn((Actor, Blocks, Position(far), Health::full(20), Faction(theirs), MeleeAttack { kind, dice: DiceRoll::flat(3), cost: None }, Name::new("a")))
            .id();
        let b = stage.app.world_mut().spawn((Actor, Blocks, Position(far.offset(1, 0)), Health::full(20), Armor(9), Faction(theirs), Name::new("b"))).id();
        stage.tick();
        stage.app.world_mut().write_message(DamageEvent { target: b, hit: rl_rules::Hit::by(a, kind, 3) });
        stage.tick();
        assert!(lines(&stage).is_empty(), "nothing seen, nothing said: {:?}", lines(&stage));

        stage.app.world_mut().resource_mut::<Phrasebook>().speak_unseen = true;
        stage.app.world_mut().write_message(DamageEvent { target: b, hit: rl_rules::Hit::by(a, kind, 3) });
        stage.tick();
        assert!(lines(&stage).is_empty(), "spoken unseen, but that phrase is silenced: {:?}", lines(&stage));
        stage.app.world_mut().entity_mut(b).insert(Armor(0));
        stage.app.world_mut().write_message(DamageEvent { target: b, hit: rl_rules::Hit::by(a, kind, 3) });
        stage.tick();
        assert_eq!(lines(&stage), vec![("The a hits the b for 3.".to_string(), Tones::TEXT)]);
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
        let (text, spans) = render("{Who} takes {n} from {named} {odd}.", &said, &names);
        assert_eq!(text, "Something takes 4 from venom {odd}.");
        assert!(spans.is_empty());
    }
}
