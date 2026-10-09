//! What a thing, and a fight, look like to the adventurer, who does not
//! yet know the level of everything they find or wear.
//!
//! The engine's panels count a worn thing's true numbers, since those are
//! what a blow meets, and mark a figure the adventurer cannot settle: left
//! alone, the vitals would read `armor 3?` for leather found at `+2`,
//! leather being armor 1, which tells the `+2` on the first turn and makes
//! the [`FAMILIAR_AFTER`](crate::lore::FAMILIAR_AFTER) turns of wear that
//! would teach it mean nothing. What a thing not yet known seems to be is
//! Undermere's to say, so in `ViewSet::Annotate` it counts every worn or
//! wielded thing not yet known at its kind's plain numbers, the level of
//! nought any thing of its kind is: the vitals' armor, which keeps its
//! mark, the look cursor's forecast of a fight and its chance to hit, and
//! the targeting box's chance to hit, each worked out again through the
//! calls the engine's collector makes and said to count unknown gear
//! plain.
//!
//! Only what a level changes is taken back: the armor a level adds to a
//! piece of armor, and what a short wearer pays for it, the damage a level
//! adds to a weapon's blow or shot, and the accuracy it lends its wielder,
//! each by the rule the armory applied it by.
//!
//! The thing itself reads the same way. The engine's pack and look cursor
//! print a thing not yet known as `unknown` and nothing it is worth, which
//! is right for a potion whose kind is a secret and wrong for chain mail,
//! whose kind is plain to see and only its level is not. So for a thing
//! whose kind is known and whose level is not, [`seem_pack`] and
//! [`seem_looked_at`] say what any thing of its kind is at nought, the
//! armor, the blow or the shot the pack prints for a known one and, for a
//! ring, what each level of it gives, which is all a ring at nought does;
//! and in place of the bare `unknown`, what is known of its level and how
//! long it must still be worn to tell it. Nothing of the true level is
//! read, so nothing of it shows.

use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_bevy::{Attempt, band_at};
use rl_engine::rl_core::{DiceRoll, Point, geometry};
use rl_engine::rl_rules::accuracy::{Delivery, Odds, Shot};
use rl_engine::rl_rules::damage::DamageKindId;
use rl_engine::rl_rules::forecast::{Combatant, Duel, duel};
use rl_engine::rl_rules::{DamageStage, StatId};

use rl_engine::rl_rules::stats::Op;
use rl_engine::rl_ui::panel::sheet::plain_op;
use rl_engine::rl_ui::view::sheet::{ResistLine, Strike};

use crate::armory::{ACCURACY_PER_LEVEL, Armory, DAMAGE_PER_LEVEL, ItemKind};
use crate::content::{ACCURACY, STRENGTH};
use crate::lore::{CurseShown, FAMILIAR_AFTER, Lore, Unfamiliar};
use crate::run::Adventurer;
use crate::strength::{armor_cost, short_of};

/// What the things someone wears and the adventurer does not know yet add
/// past their kind's plain numbers, which is what a figure counting them
/// as known takes back off.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Unseen {
    /// Armor, less what more a wearer short of strength pays for it.
    pub armor: i32,
    /// The bonus a level puts on the blow struck with an unknown thing.
    pub blow: i32,
    /// The bonus a level puts on the shot fired with one.
    pub shot: i32,
    /// The accuracy every unknown weapon worn lends at its level.
    pub accuracy: i32,
}

/// The rolls one blow or shot lands, with `less` off the first, the blow
/// or the shot itself, and any extra strikes as they are.
fn less_off(mut rolls: Vec<(DamageKindId, DiceRoll)>, less: i32) -> Vec<(DamageKindId, DiceRoll)> {
    if let Some((_, dice)) = rolls.first_mut() {
        dice.bonus -= less;
    }
    rolls
}

/// A thing not yet known as its plain numbers are worked out from: its
/// kind, its level and the armor it truly gives.
type Unknowable = (&'static ItemKind, Option<&'static Enchant>, Option<&'static Armor>);

/// Everything a fight is reckoned from, and what is not known of it.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Seeming<'w, 's> {
    armory: Res<'w, Armory>,
    registries: Res<'w, Registries>,
    loadout: Loadout<'w, 's>,
    worn: Query<'w, 's, &'static Equipped>,
    /// Each thing not yet known, its kind, its level and the armor it gives.
    unknown: Query<'w, 's, Unknowable, With<Unknown>>,
    stats: Query<'w, 's, &'static StatBlock>,
    /// Who pays for wearing too heavy a thing: only the adventurer.
    adventurers: Query<'w, 's, (), With<Adventurer>>,
    bodies: Query<'w, 's, (&'static Position, Option<&'static Health>)>,
    speeds: Speeds<'w, 's>,
    marks: Marksmanship<'w, 's>,
    hits: Res<'w, HitRules>,
    lighting: Option<Res<'w, Lighting>>,
    rules: Res<'w, CombatRules>,
    stages: Res<'w, DamageStages>,
}

impl Seeming<'_, '_> {
    /// What the things `who` wears and the adventurer does not know yet
    /// add past their kind's plain numbers. Each thing once, however many
    /// slots it fills; nothing for someone who wears nothing unknown.
    pub fn unseen(&self, who: Entity) -> Unseen {
        let mut unseen = Unseen::default();
        let Ok(worn) = self.worn.get(who) else { return unseen };
        // Only the adventurer pays for a thing too heavy for them, and pays
        // by the armor the thing gives, so what they seem to pay is what
        // the thing seems to give.
        let strength = self.registries.stats.id(STRENGTH).filter(|_| self.adventurers.contains(who));
        let strength = strength.and_then(|stat| self.stats.get(who).ok().map(|s| s.0.value(stat, &self.registries.stats)));
        let mut counted: Vec<Entity> = Vec::new();
        for (_, item) in worn.0.worn() {
            if counted.contains(&item) {
                continue;
            }
            counted.push(item);
            let Ok((kind, enchant, armor)) = self.unknown.get(item) else { continue };
            let d = self.armory.defs.get(kind.0);
            let (gives, plain) = (armor.map_or(0, |a| a.0), self.armory.armor_at(kind.0, 0));
            unseen.armor += gives - plain;
            if let Some(strength) = strength {
                let short = short_of(d, strength);
                unseen.armor -= armor_cost(d, short, gives) - armor_cost(d, short, plain);
            }
            if d.melee.is_some() || d.ranged.is_some() {
                unseen.accuracy += ACCURACY_PER_LEVEL * enchant.map_or(0, |e| e.level);
            }
        }
        // The blow and the shot from the worn thing the engine strikes and
        // fires with, when that is one not known yet.
        let level = |from: Option<Entity>| from.and_then(|item| self.unknown.get(item).ok()).map_or(0, |(_, enchant, _)| enchant.map_or(0, |e| e.level));
        unseen.blow = DAMAGE_PER_LEVEL * level(self.loadout.melee_with(who).and_then(|(from, _)| from));
        unseen.shot = DAMAGE_PER_LEVEL * level(self.loadout.ranged_with(who).and_then(|(from, _)| from));
        unseen
    }

    /// The odds `attacker` lands what it would attack `target` with from
    /// where both stand, its accuracy `less` lower, by the rule the engine's
    /// `Marksmanship::at_distance` picks the attack by: a blow beside, a
    /// shot while it reaches, and nothing further.
    fn odds(&self, attacker: Entity, target: Entity, less: i32) -> Option<Odds> {
        if less == 0 {
            return self.marks.at_distance(&self.loadout, attacker, target);
        }
        let distance = geometry::chebyshev(self.bodies.get(attacker).ok()?.0.0, self.bodies.get(target).ok()?.0.0);
        if distance <= 1 {
            self.loadout.melee(attacker)?;
            return self.odds_of(attacker, target, Attempt::Blow, less);
        }
        let gun = self.loadout.ranged(attacker).filter(|g| distance <= g.range)?;
        self.odds_of(attacker, target, Attempt::Shot(&gun), less)
    }

    /// The odds of what `user` fires at `target` with the cursor on
    /// `cursor`, its accuracy `less` lower, as the targeting cursor previews
    /// a shot: struck as a blow point blank, and as the shot in hand
    /// further off.
    pub fn fire_odds(&self, user: Entity, target: Entity, cursor: Point, less: i32) -> Option<Odds> {
        if geometry::is_adjacent(self.bodies.get(user).ok()?.0.0, cursor) {
            return self.odds_of(user, target, Attempt::Blow, less);
        }
        let gun = self.loadout.ranged(user)?;
        self.odds_of(user, target, Attempt::Shot(&gun), less)
    }

    /// The odds `attacker` lands `attempt` on `target` where both stand, its
    /// accuracy `less` lower: the engine's own `Marksmanship::odds` when
    /// nothing is to be taken off, and otherwise the same shot put to the
    /// run's hit model as that call puts it, with that accuracy.
    pub fn odds_of(&self, attacker: Entity, target: Entity, attempt: Attempt, less: i32) -> Option<Odds> {
        if less == 0 {
            return self.marks.odds(attacker, target, attempt);
        }
        let (from, to) = (self.bodies.get(attacker).ok()?.0.0, self.bodies.get(target).ok()?.0.0);
        let (delivery, effective, range) = match attempt {
            Attempt::Blow => (Delivery::Melee, 0, 1),
            Attempt::Shot(gun) => (Delivery::Shot, gun.effective_range(), gun.range),
            Attempt::Throw(thrown) => {
                thrown.strike?;
                (Delivery::Thrown, thrown.effective_range(), thrown.range)
            }
        };
        let stat = |who: Entity, pick: Option<StatId>| pick.map(|stat| self.stats.get(who).map_or(0, |s| s.0.value(stat, &self.registries.stats)));
        let accuracy = stat(attacker, self.rules.accuracy).map(|a| a - less);
        let light = band_at(self.lighting.as_deref(), to);
        let shot = Shot { delivery, distance: geometry::chebyshev(from, to), effective, range, light, accuracy, evasion: stat(target, self.rules.evasion) };
        self.hits.0.odds(&shot)
    }

    /// The fight between `me` and `them` as the look cursor forecasts it,
    /// with every worn or wielded thing not yet known on either side at
    /// its kind's plain numbers, and `me`'s chance to land what it would
    /// strike with from here; `None` when nothing either side wears is
    /// unknown, or either is not a body with health.
    ///
    /// Each side as `rl_rules::forecast` reads it, built as the engine's
    /// collector builds it, so a fight with nothing unknown in it reckons
    /// exactly what the collector did.
    pub fn forecast(&self, me: Entity, them: Entity) -> Option<(Duel, Option<Odds>)> {
        let (mine, theirs) = (self.unseen(me), self.unseen(them));
        if mine == Unseen::default() && theirs == Unseen::default() {
            return None;
        }
        let ((my_at, Some(my_health)), (their_at, Some(their_health))) = (self.bodies.get(me).ok()?, self.bodies.get(them).ok()?) else { return None };
        let apart = geometry::chebyshev(my_at.0, their_at.0);
        let (my_blows, my_shots) = (less_off(self.loadout.blows(me), mine.blow), less_off(self.loadout.shots(me), mine.shot));
        let (their_blows, their_shots) = (less_off(self.loadout.blows(them), theirs.blow), less_off(self.loadout.shots(them), theirs.shot));
        let (my_arms, their_arms) = (self.loadout.arms(me, &my_blows, &my_shots), self.loadout.arms(them, &their_blows, &their_shots));
        let (my_resists, their_resists) = (self.loadout.resistances(me), self.loadout.resistances(them));
        let (my_odds, their_odds) = (self.odds(me, them, mine.accuracy), self.odds(them, me, theirs.accuracy));
        let asker = Combatant::closing(my_health.current, self.loadout.armor(me) - mine.armor, self.speeds.of(me), &my_resists, &my_arms, apart)
            .hitting(my_odds.as_ref().map_or(100, Odds::percent));
        let other = Combatant::closing(their_health.current, self.loadout.armor(them) - theirs.armor, self.speeds.of(them), &their_resists, &their_arms, apart)
            .hitting(their_odds.as_ref().map_or(100, Odds::percent));
        let stages: Vec<&dyn DamageStage<Entity>> = self.stages.0.iter().map(|s| s.as_ref() as &dyn DamageStage<Entity>).collect();
        Some((duel(&asker, &other, &self.registries.damage_kinds, &stages), my_odds))
    }
}

/// Counts each thing the adventurer wears and does not know yet at its
/// kind's plain armor in the vitals' figure, which keeps the mark the
/// engine put on it: `+2 leather armor` not yet known reads `armor 1?`,
/// what any leather gives, and `armor 3` once it is known.
///
/// In `ViewSet::Annotate`, over the engine's true total. Absent vitals, as
/// in a test that draws none, is nothing to correct.
pub fn seem_armor(vitals: Option<ResMut<VitalsView>>, seeming: Seeming) {
    let Some(mut vitals) = vitals else { return };
    let Some(me) = vitals.entity.filter(|_| vitals.armor_uncertain) else { return };
    let unseen = seeming.unseen(me).armor;
    if unseen != 0 {
        vitals.armor = Some(vitals.armor.unwrap_or(0) - unseen);
    }
}

/// Forecasts the fight the look cursor is over with every worn or wielded
/// thing not yet known on either side at its kind's plain numbers, as
/// [`Seeming::forecast`] reckons it, its chance to hit with it, and says
/// under it that unknown gear is counted plain, the forecast's own `?`.
///
/// In `ViewSet::Annotate`, over the engine's forecast, which is left as it
/// is when nothing in the fight is unknown.
pub fn seem_forecast(view: Option<ResMut<InspectView>>, seeming: Seeming, players: Query<Entity, With<Player>>, mut facets: ResMut<Facets>) {
    let Some(mut view) = view else { return };
    let (Some(_), Some(them), Ok(me)) = (view.duel, view.subject.as_ref().map(|s| s.entity), players.single()) else { return };
    let Some((forecast, odds)) = seeming.forecast(me, them) else { return };
    view.duel = Some(forecast);
    view.odds = odds;
    view.facets.push(facets.facet("forecast", "unknown gear counted plain").toned(Tones::MUTED));
}

/// Writes the targeting box's chance to hit again with every worn thing
/// not yet known lending only its kind's plain accuracy, as
/// [`Seeming::odds_of`] reckons it, and says under it that unknown gear is
/// counted plain, the chance's own `?`: a `+2` bow not yet known no longer
/// reads `+10 accuracy`, the level its wearer has not learned.
///
/// A shot or a blow fired, and a throw, whose chance reads the thrower's
/// accuracy as well. In `ViewSet::Annotate`, over the engine's chance, which
/// is left as it is when nothing the aimer wears is unknown.
pub fn seem_aim(view: Option<ResMut<TargetView>>, seeming: Seeming, missiles: Query<&Throwable>, mut facets: ResMut<Facets>) {
    let Some(mut view) = view else { return };
    let (Some(user), Some(target), true) = (view.user, view.targets.first().map(|r| r.entity), view.odds.is_some()) else { return };
    let less = seeming.unseen(user).accuracy;
    if less == 0 {
        return;
    }
    view.odds = match (view.throwing, view.firing) {
        (Some(item), _) => missiles.get(item).ok().and_then(|thrown| seeming.odds_of(user, target, Attempt::Throw(thrown), less)),
        (None, true) => seeming.fire_odds(user, target, view.cursor, less),
        (None, false) => return,
    };
    view.facets.push(facets.facet("odds", "unknown gear counted plain").toned(Tones::MUTED));
}

/// What any thing of a kind is at nought, in the terms the pack prints a
/// known one in: what [`Unsettled::plain`] reads off the kind's line, and
/// never off the thing, whose numbers carry its level.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Plain {
    /// The blow it is swung with.
    pub blow: Option<Strike>,
    /// The shot it fires.
    pub shot: Option<Strike>,
    /// The armor it gives.
    pub armor: i32,
    /// What it resists, which no level touches.
    pub resists: Vec<ResistLine>,
    /// What it gives its wearer's stats, by the stat's name.
    pub gives: Vec<(String, Op)>,
    /// What it strikes for thrown.
    pub thrown: Option<Strike>,
    /// What each level of it gives, by the stat's name: all a ring does,
    /// since at nought it gives nothing.
    pub per_level: Vec<(String, i32)>,
}

impl Plain {
    /// Its numbers as lines, in the pack's words and the pack's order, for
    /// a panel that prints no numbers of its own: the look cursor's.
    pub fn lines(&self) -> Vec<String> {
        let strike = |s: &Strike| match s.range {
            Some(range) => format!("{} {} to {range}", s.dice, s.kind),
            None => format!("{} {}", s.dice, s.kind),
        };
        let mut lines: Vec<String> = Vec::new();
        lines.extend(self.blow.iter().map(strike));
        lines.extend(self.shot.iter().map(|s| format!("shoots {}", strike(s))));
        if self.armor != 0 {
            lines.push(format!("armor {:+}", self.armor));
        }
        lines.extend(self.resists.iter().map(|r| if r.pct < 0 { format!("weak to {} {}%", r.name, -r.pct) } else { format!("resists {} {}%", r.name, r.pct) }));
        lines.extend(self.gives.iter().map(|(stat, op)| format!("{stat} {}", plain_op(*op))));
        lines.extend(self.thrown.iter().map(|s| format!("thrown {}", strike(s))));
        lines
    }

    /// What each level gives, a line apiece: `light +1 a level`.
    pub fn per_level_lines(&self) -> Vec<String> {
        self.per_level.iter().map(|(stat, n)| format!("{stat} {n:+} a level")).collect()
    }
}

/// A thing whose level is not known yet, as [`Unsettled`] reads it: its
/// kind, its level, how long it has been worn, whether its curse has shown,
/// and what it resists.
type Unlearned = (&'static ItemKind, Option<&'static Enchant>, &'static Unfamiliar, Has<CurseShown>, Option<&'static Resists>);

/// What the pack and the look cursor read of a thing whose kind the
/// adventurer knows and whose level they do not: its kind, what they have
/// learned of its level, and the facet keys the lines go under.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Unsettled<'w, 's> {
    armory: Res<'w, Armory>,
    registries: Res<'w, Registries>,
    lore: Res<'w, Lore>,
    /// Each thing not yet known, its kind, its level, how long it has been
    /// worn, whether its curse has shown, and what it resists.
    things: Query<'w, 's, Unlearned, With<Unknown>>,
    facets: ResMut<'w, Facets>,
}

impl Unsettled<'_, '_> {
    /// What any thing of `thing`'s kind is at nought, when `thing` is one
    /// whose kind is known and whose level is not; `None` for anything
    /// else, a potion of a kind not yet known among them, which stays
    /// `unknown`.
    pub fn plain(&self, thing: Entity) -> Option<Plain> {
        let (kind, _, _, _, resists) = self.things.get(thing).ok()?;
        let d = self.armory.defs.get(kind.0);
        if !self.lore.knows(&d.name) || !self.armory.learned_by_wear(kind.0) {
            return None;
        }
        let r = &self.registries;
        let strike = |kind: Id<rl_engine::rl_rules::damage::DamageKind>, dice: DiceRoll, range: Option<i32>| Strike {
            kind: r.damage_kinds.name(kind).to_string(),
            dice,
            range,
        };
        Some(Plain {
            // A level of nought adds nothing to a roll, so the kind's own.
            blow: d.melee.as_ref().map(|m| strike(m.kind.id(), m.roll, None)),
            shot: d.ranged.as_ref().map(|g| strike(g.kind.id(), g.roll, Some(g.range))),
            armor: self.armory.armor_at(kind.0, 0),
            resists: resists
                .map(|resists| {
                    r.damage_kinds
                        .iter()
                        .filter(|(k, _)| resists.0.get(*k) != 0)
                        .map(|(k, def)| ResistLine { kind: k, name: def.name.clone(), pct: resists.0.get(k) })
                        .collect()
                })
                .unwrap_or_default(),
            gives: self.armory.gives_at(kind.0, 0, r.stats.id(ACCURACY)).into_iter().map(|(stat, op)| (r.stats.name(stat).to_string(), op)).collect(),
            thrown: d.throw.and_then(|t| t.strike.map(|(dice, k)| strike(k.id(), dice, Some(t.range)))),
            per_level: d.per_level.iter().map(|(stat, n)| (r.stats.name(stat.id()).to_string(), *n)).collect(),
        })
    }

    /// What is known of `thing`'s level, and how long it must still be
    /// worn to tell it, the lines a panel prints in place of `unknown`:
    /// "level not yet known, shown as +0" over a kind's numbers at nought,
    /// the bare "level not yet known" for a ring, whose lines are what a
    /// level gives, and "level below nought, how far not yet known" once a
    /// curse has shown; then "known in 140 more turns worn" while it is
    /// `worn`, and "wearing it 200 turns will tell its level" while it is
    /// not.
    ///
    /// Below nought only while it still is: a curse shown and then
    /// enchanted up to nought came unstuck saying so, and is not known to
    /// be below anything.
    pub fn level_lines(&mut self, thing: Entity, plain: &Plain, worn: bool) -> Vec<Facet> {
        let Ok((_, level, unfamiliar, shown, _)) = self.things.get(thing) else { return Vec::new() };
        let below = shown && level.is_some_and(|l| l.level < 0);
        let numbers = !plain.lines().is_empty();
        let (known, tone) = match (below, numbers) {
            (true, _) => ("level below nought, how far not yet known", Tones::BAD),
            (false, true) => ("level not yet known, shown as +0", Tones::MUTED),
            (false, false) => ("level not yet known", Tones::MUTED),
        };
        let left = unfamiliar.left();
        let turns = if left == 1 { "1 more turn".to_string() } else { format!("{left} more turns") };
        let wear = match (worn, unfamiliar.worn) {
            (true, _) => format!("known in {turns} worn"),
            (false, 0) => format!("wearing it {FAMILIAR_AFTER} turns will tell its level"),
            (false, _) => format!("wearing it {turns} will tell its level"),
        };
        vec![self.facets.facet("level", known).toned(tone), self.facets.facet("level", wear).toned(Tones::MUTED)]
    }

    /// What each level of it gives, as facets under its numbers.
    fn per_level(&mut self, plain: &Plain) -> Vec<Facet> {
        plain.per_level_lines().into_iter().map(|line| self.facets.facet("per level", line)).collect()
    }
}

/// Writes into each row of the pack whose kind is known and whose level is
/// not what any thing of its kind is at nought, which the pack prints as it
/// prints a known thing's numbers, and in place of its bare `unknown` what
/// is known of its level and how long it must still be worn to tell it, as
/// [`Unsettled`] reads them; a ring, which gives nothing at nought, says
/// what each level gives under them.
///
/// In `ViewSet::Annotate`, over the engine's row, which keeps nothing of a
/// thing not yet known. Absent a pack, as in a test that draws none,
/// nothing is written.
pub fn seem_pack(view: Option<ResMut<InventoryView>>, mut unsettled: Unsettled) {
    let Some(mut view) = view else { return };
    for row in view.rows.iter_mut().filter(|r| r.unknown) {
        let Some(plain) = unsettled.plain(row.entity) else { continue };
        row.seems = unsettled.level_lines(row.entity, &plain, row.worn());
        let per_level = unsettled.per_level(&plain);
        row.facets.extend(per_level);
        let Plain { blow, shot, armor, resists, gives, thrown, .. } = plain;
        (row.blow, row.shot, row.armor, row.resists, row.bestows, row.thrown) = (blow, shot, armor, resists, gives, thrown);
    }
}

/// Says under the look cursor, of a thing lying there whose kind is known
/// and whose level is not, what is known of its level and how long wearing
/// it will take to tell it, in place of the bare `unknown`, and what any
/// thing of its kind is at nought under that, as [`seem_pack`] says it in
/// the pack.
///
/// In `ViewSet::Annotate`, over the engine's view. Absent the look cursor,
/// nothing is said.
pub fn seem_looked_at(view: Option<ResMut<InspectView>>, mut unsettled: Unsettled) {
    let Some(mut view) = view else { return };
    let Some(thing) = view.subject.as_ref().map(|s| s.entity).filter(|_| view.unknown) else { return };
    let Some(plain) = unsettled.plain(thing) else { return };
    view.seems = unsettled.level_lines(thing, &plain, false);
    for line in plain.lines() {
        let facet = unsettled.facets.facet("plain", line);
        view.facets.push(facet);
    }
    let per_level = unsettled.per_level(&plain);
    view.facets.extend(per_level);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lore::{FAMILIAR_AFTER, identify};
    use crate::testing::{arena_with, equip, find, pass_turns, settle, still};

    /// The vitals' armor, and whether it is marked unsettled.
    fn vitals(app: &App) -> (Option<i32>, bool) {
        let view = app.world().resource::<VitalsView>();
        (view.armor, view.armor_uncertain)
    }

    /// `name` at `level`, found, and known outright when `known`, as a
    /// scroll of identify would make it.
    fn thing(app: &mut App, name: &str, level: i32, known: bool) -> Entity {
        let thing = find(app, name, level);
        if known {
            identify(app.world_mut(), thing);
        }
        thing
    }

    /// The vitals with `name` at `level` put on, known or not.
    fn vitals_wearing(name: &str, level: i32, known: bool) -> (Option<i32>, bool) {
        let (mut app, me, _) = arena_with(21, VitalsViewPlugin);
        let worn = thing(&mut app, name, level, known);
        equip(&mut app, me, worn);
        settle(&mut app);
        vitals(&app)
    }

    /// Leather is armor 1, so `+2 leather armor` not yet known reads what
    /// any leather gives, marked, and not the `armor 3?` that told its level
    /// on the first turn; worn two hundred turns, it is known, and reads its
    /// own three unmarked.
    #[test]
    fn a_worn_thing_not_yet_known_reads_its_kinds_plain_armor_marked_until_it_is_familiar() {
        let (mut app, me, _) = arena_with(21, VitalsViewPlugin);
        let leather = thing(&mut app, "leather armor", 2, false);
        equip(&mut app, me, leather);
        settle(&mut app);
        assert_eq!(vitals(&app), (Some(1), true), "what any leather gives, and marked");
        pass_turns(&mut app, FAMILIAR_AFTER);
        settle(&mut app);
        assert_eq!(vitals(&app), (Some(3), false), "familiar, its own, unmarked");
    }

    /// A thing made known by being told is read as it is at once, as one
    /// worn long enough is.
    #[test]
    fn a_worn_thing_identified_reads_its_own_armor_unmarked() {
        let (mut app, me, _) = arena_with(21, VitalsViewPlugin);
        let leather = thing(&mut app, "leather armor", 2, false);
        equip(&mut app, me, leather);
        settle(&mut app);
        identify(app.world_mut(), leather);
        settle(&mut app);
        assert_eq!(vitals(&app), (Some(3), false));
    }

    /// Only what is not known is counted plain: a known `+1 helm` beside an
    /// unknown `+2 leather armor` gives its own two, and the leather what any
    /// leather gives.
    #[test]
    fn a_known_worn_thing_beside_an_unknown_one_counts_at_its_own_numbers() {
        let (mut app, me, _) = arena_with(22, VitalsViewPlugin);
        let helm = thing(&mut app, "helm", 1, true);
        equip(&mut app, me, helm);
        let leather = thing(&mut app, "leather armor", 2, false);
        equip(&mut app, me, leather);
        settle(&mut app);
        assert_eq!(vitals(&app), (Some(2 + 1), true));
    }

    /// Over armor of every weight and level, cursed and too heavy to wear
    /// well included, a worn thing not yet known reads exactly what a plain
    /// one of its kind reads, what a short wearer pays for it counted as the
    /// plain one's, and only the unknown one is marked.
    #[test]
    fn an_unknown_worn_thing_reads_what_a_plain_one_of_its_kind_reads_whatever_its_level() {
        for (name, level) in [("leather armor", 2), ("plate armor", 2), ("scale mail", -2), ("shield", 1), ("chain mail", 3)] {
            let (unknown, plain) = (vitals_wearing(name, level, false), vitals_wearing(name, 0, true));
            assert_eq!(unknown, (plain.0, true), "{name} at {level:+}, not yet known, reads as a plain one, marked");
            assert!(!plain.1, "{name}, known, is not marked");
        }
    }

    /// A pack row as the test below compares it: what it says in place of
    /// `unknown`, its armor, its blow, its shot, what it gives, and what the
    /// game added.
    type Said = (Vec<String>, i32, Option<String>, Option<String>, Vec<(String, Op)>, Vec<String>);

    /// What the pack's row for `name` at `level`, found and carried, its
    /// kind known, says it is.
    fn pack_row(name: &str, level: i32) -> Said {
        let (mut app, me, _) = arena_with(25, InventoryViewPlugin);
        app.world_mut().resource_mut::<crate::lore::Lore>().learn(name);
        let found = find(&mut app, name, level);
        crate::testing::give(&mut app, me, found);
        settle(&mut app);
        let view = app.world().resource::<InventoryView>();
        let row = view.rows.iter().find(|r| r.entity == found).expect("in the pack");
        let strike = |s: &Option<Strike>| s.as_ref().map(|s| format!("{} {}", s.dice, s.kind));
        let texts = |facets: &[Facet]| facets.iter().map(|f| f.text.clone()).collect::<Vec<_>>();
        (texts(&row.seems), row.armor, strike(&row.blow), strike(&row.shot), row.bestows.clone(), texts(&row.facets))
    }

    /// Over every kind whose level is learned by wearing it, what the pack
    /// says of one found is the same at every level it could be, cursed or
    /// better, so nothing in it tells the level; and it is what a +0 one of
    /// the kind gives, which is what a known +0 one reads.
    #[test]
    fn what_the_pack_says_of_a_thing_whose_level_is_not_known_is_the_same_at_every_level() {
        let library = crate::content::Library::load(&crate::content::effect_kinds(), &Moments::default());
        let armory = &library.armory;
        let kinds: Vec<&str> = armory.defs.iter().filter(|(id, _)| armory.learned_by_wear(*id)).map(|(_, d)| d.name.as_str()).collect();
        for name in ["chain mail", "sword", "bow", "shield", "ring of regeneration"] {
            assert!(kinds.contains(&name), "{name} is learned by wearing it");
            let plain = pack_row(name, 0);
            assert!(plain.0.first().is_some_and(|l| l.starts_with("level not yet known")), "{name}: {:?}", plain.0);
            for level in [-3, -1, 2, 3] {
                assert_eq!(pack_row(name, level), plain, "{name} at {level:+} reads as it does at nought");
            }
        }
        let (_, armor, ..) = pack_row("chain mail", 2);
        assert_eq!(armor, 3, "chain mail at nought");
        let (.., per_level) = pack_row("ring of regeneration", 2);
        assert_eq!(per_level, ["regeneration +10 a level", "mending +25 a level"], "what each level of a ring gives");
    }

    /// The forecast of a fight with a goblin, the chance to hit it, and what
    /// the game said under it, with `name` at `level` wielded, known or not.
    fn forecast_wielding(name: &str, level: i32, known: bool) -> (Option<Duel>, Option<Odds>, Vec<String>) {
        let (mut app, me, here) = arena_with(23, InspectViewPlugin);
        still(&mut app, "goblin", here.offset(1, 0));
        let weapon = thing(&mut app, name, level, known);
        equip(&mut app, me, weapon);
        let modal = rl_engine::rl_ui::view::inspect::inspect_modal(app.world().resource::<Modals>());
        app.world_mut().resource_mut::<Modals>().open(modal);
        app.world_mut().resource_mut::<InspectView>().cursor = here.offset(1, 0);
        app.update();
        let view = app.world().resource::<InspectView>();
        (view.duel, view.odds.clone(), view.facets.iter().map(|f| f.text.clone()).collect())
    }

    /// The look cursor forecasts a fight with a sword not yet known as one
    /// with a plain sword, its blow and the accuracy it lends both, and says
    /// so; known, its own level counts, as the engine forecast it.
    #[test]
    fn the_forecast_counts_an_unknown_weapon_as_a_plain_one_of_its_kind_and_says_so() {
        let unknown = forecast_wielding("sword", 3, false);
        let plain = forecast_wielding("sword", 0, true);
        let known = forecast_wielding("sword", 3, true);
        assert!(plain.0.is_some(), "a goblin beside the adventurer is forecast");
        assert_eq!((unknown.0, &unknown.1), (plain.0, &plain.1), "a +3 sword not yet known is forecast as a plain one");
        assert_eq!(unknown.2, ["unknown gear counted plain"], "and the forecast says it counted it so");
        assert!(plain.2.is_empty() && known.2.is_empty(), "nothing is said of a fight with nothing unknown in it");
        assert_ne!((known.0, &known.1), (plain.0, &plain.1), "known, its level counts");
    }

    /// The targeting box's chance to hit, and what the game said under it,
    /// for a shot at a goblin four cells off with a bow at `level`, known
    /// or not, and arrows in the pack.
    fn aiming_a_bow(level: i32, known: bool) -> (Option<Odds>, Vec<String>) {
        let (mut app, me, here) = arena_with(24, TargetViewPlugin);
        still(&mut app, "goblin", here.offset(4, 0));
        let arrows = find(&mut app, "arrow", 0);
        crate::testing::give(&mut app, me, arrows);
        let bow = thing(&mut app, "bow", level, known);
        equip(&mut app, me, bow);
        app.world_mut().write_message(AimFire { user: me });
        app.update();
        app.world_mut().resource_mut::<TargetView>().cursor = here.offset(4, 0);
        app.update();
        let view = app.world().resource::<TargetView>();
        (view.odds.clone(), view.facets.iter().map(|f| f.text.clone()).collect())
    }

    /// The targeting box gives a shot with a bow not yet known the chance a
    /// plain bow gives, not the `+10 accuracy` a `+2` one lends, and says it
    /// counted it so; known, its level counts, and nothing is said.
    #[test]
    fn the_targeting_box_counts_an_unknown_bows_accuracy_as_a_plain_ones_and_says_so() {
        let (unknown, plain, known) = (aiming_a_bow(2, false), aiming_a_bow(0, true), aiming_a_bow(2, true));
        assert!(plain.0.is_some(), "a goblin in reach has a chance to be hit");
        assert_eq!(unknown.0, plain.0, "a +2 bow not yet known is aimed as a plain one");
        assert_eq!(unknown.1, ["unknown gear counted plain"], "and the box says it counted it so");
        assert!(plain.1.is_empty() && known.1.is_empty(), "nothing is said of an aim with nothing unknown in it");
        assert_ne!(known.0, plain.0, "known, its level counts");
    }
}
