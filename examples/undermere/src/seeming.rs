//! What a thing, and a fight, look like to the adventurer, who does not
//! yet know the level of everything they find or wear.
//!
//! The engine's panels count a worn thing's true numbers, since those are
//! what a blow meets, and mark a figure the adventurer cannot settle: left
//! alone, the vitals would read `armor 3?` for leather found at `+2`,
//! leather being armor 1, which tells the `+2` on the first turn and makes
//! the [`FAMILIAR_AFTER`] turns of wear that
//! would teach it mean nothing. What a thing not yet known seems to be is
//! Undermere's to say, so [`keep_seems`] keeps on every wearer the engine's
//! [`Seems`]: what each worn or wielded thing not yet known adds past its
//! kind's plain numbers, the level of nought any thing of its kind is. The
//! engine's collectors take that off the vitals' armor, which keeps its
//! mark, the look cursor's forecast of a fight and its chance to hit, and
//! the targeting box's chance to hit, and say they did; Undermere says, in
//! its own words, that unknown gear is counted plain.
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
use rl_engine::rl_core::DiceRoll;

use rl_engine::rl_rules::stats::Op;
use rl_engine::rl_ui::panel::sheet::plain_op;
use rl_engine::rl_ui::view::sheet::{ResistLine, Strike};

use crate::armory::{Armory, ItemKind};
use crate::content::{ACCURACY, ARMOR, STRENGTH};
use crate::lore::{CurseShown, FAMILIAR_AFTER, Lore, Unfamiliar};
use crate::run::Adventurer;
use crate::strength::{armor_cost, short_of};

/// A thing not yet known as its plain numbers are worked out from: its
/// kind, its level and the armor it truly gives.
type Unknowable = (&'static ItemKind, Option<&'static ItemLevel>, Option<&'static Armor>);

/// Everything what someone's worn things seem to be is reckoned from.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Seeming<'w, 's> {
    armory: Res<'w, Armory>,
    lore: Res<'w, Lore>,
    registries: Res<'w, Registries>,
    loadout: Loadout<'w, 's>,
    worn: Query<'w, 's, &'static Equipped>,
    /// Each thing not yet known, its kind, its level and the armor it gives.
    unknown: Query<'w, 's, Unknowable, With<Unknown>>,
    stats: Query<'w, 's, &'static StatBlock>,
    /// Who pays for wearing too heavy a thing: only the adventurer.
    adventurers: Query<'w, 's, (), With<Adventurer>>,
}

impl Seeming<'_, '_> {
    /// What the things `who` wears and the adventurer does not know yet
    /// add past their kind's plain numbers, as the engine's [`Seems`] takes
    /// them off. Each thing once, however many slots it fills; nothing for
    /// someone who wears nothing unknown.
    pub fn unseen(&self, who: Entity) -> Seems {
        let mut unseen = Seems::default();
        let Ok(worn) = self.worn.get(who) else { return unseen };
        // Only the adventurer pays for a thing too heavy for them, and pays
        // by the armor the thing gives, so what they seem to pay is what
        // the thing seems to give.
        let strength = self.registries.stats.id(STRENGTH).filter(|_| self.adventurers.contains(who));
        let strength = strength.and_then(|stat| self.stats.get(who).ok().map(|s| s.0.value(stat, &self.registries.stats)));
        let accuracy = self.registries.stats.id(ACCURACY);
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
            unseen.accuracy += self.armory.per_level(kind.0, accuracy) * enchant.map_or(0, |l| l.0);
        }
        // The blow and the shot from the worn thing the engine strikes and
        // fires with, when that is one not known yet: what its level adds to
        // a damage roll, that many times.
        let damage = |from: Option<Entity>| {
            from.and_then(|item| self.unknown.get(item).ok())
                .map_or(0, |(kind, level, _)| self.armory.level_of(kind.0).and_then(|l| l.damage).unwrap_or(0) * level.map_or(0, |l| l.0))
        };
        unseen.blow = damage(self.loadout.melee_with(who).and_then(|(from, _)| from));
        unseen.shot = damage(self.loadout.ranged_with(who).and_then(|(from, _)| from));
        unseen
    }

    /// Whether anything `who` wears and the adventurer does not know yet
    /// could move their armor: a thing of a kind not yet known, which could
    /// be any kind of its family, or one of a known kind whose level moves
    /// armor, as [`Armory::level_moves_armor`] says. Read off the kind and
    /// never off the thing, so the answer is one the adventurer could give.
    pub fn armor_unsettled(&self, who: Entity) -> bool {
        let Ok(worn) = self.worn.get(who) else { return false };
        let armor = self.registries.stats.id(ARMOR);
        worn.0.worn().any(|(_, item)| {
            self.unknown.get(item).is_ok_and(|(kind, ..)| !self.lore.knows(&self.armory.defs.get(kind.0).name) || self.armory.level_moves_armor(kind.0, armor))
        })
    }
}

/// Keeps on every wearer the [`Seems`] the engine's panels take off its
/// true numbers: what each worn or wielded thing not yet known adds past
/// its kind's plain numbers, as [`Seeming::unseen`] reckons it.
///
/// Every wearer carries one from the moment it can wear anything, a
/// component [`Equipped`] requires in this game, so keeping it changes a
/// value and never what an entity is made of. Written only when it moved,
/// so nothing reads a change that did not happen.
///
/// Before `ViewSet::Collect`, where the collectors read it.
pub fn keep_seems(mut wearers: Query<(Entity, &mut Seems)>, seeming: Seeming) {
    for (who, mut seems) in &mut wearers {
        seems.set_if_neq(seeming.unseen(who));
    }
}

/// Takes the mark off the vitals' armor, which the engine puts on it
/// whenever anything worn is not yet known, when nothing worn could move
/// it, as [`Seeming::armor_unsettled`] says: an unidentified ring of light
/// leaves `armor 1` plain. The figure itself the engine counts as it
/// seems, from [`keep_seems`]: `+2 leather armor` not yet known reads
/// `armor 1?`, what any leather gives, and `armor 3` once it is known.
///
/// A rule of the kind, which the adventurer knows, and never of the thing:
/// marking by whether the thing gives armor would tell a cursed leather at
/// nought armor from a plain one, or a ring of a kind that gives armor from
/// one that gives light, the turn it went on.
///
/// In `ViewSet::Annotate`. Absent vitals, as in a test that draws none, is
/// nothing to correct.
pub fn seem_armor(vitals: Option<ResMut<VitalsView>>, seeming: Seeming) {
    let Some(mut vitals) = vitals else { return };
    let Some(me) = vitals.entity.filter(|_| vitals.armor_uncertain) else { return };
    vitals.armor_uncertain = seeming.armor_unsettled(me);
}

/// Says under the look cursor's forecast that unknown gear is counted
/// plain, the forecast's own `?`, when the engine counted either side's
/// worn things not yet known as they seem.
///
/// In `ViewSet::Annotate`. The engine forecasts no fight with an ally or in
/// Mereholt's peace, so the words never qualify a forecast that is not
/// there.
pub fn seem_forecast(view: Option<ResMut<InspectView>>, mut facets: ResMut<Facets>) {
    let Some(mut view) = view else { return };
    if view.seeming {
        view.facets.push(facets.facet("forecast", "unknown gear counted plain").toned(Tones::MUTED));
    }
}

/// Says under the targeting box's chance to hit that unknown gear is
/// counted plain, the chance's own `?`, when the engine reckoned it at the
/// accuracy the aimer seems to have: a `+2` bow not yet known no longer
/// reads `+10 accuracy`, the level its wearer has not learned.
///
/// In `ViewSet::Annotate`.
pub fn seem_aim(view: Option<ResMut<TargetView>>, mut facets: ResMut<Facets>) {
    let Some(mut view) = view else { return };
    if view.seeming {
        view.facets.push(facets.facet("odds", "unknown gear counted plain").toned(Tones::MUTED));
    }
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
    /// What each level of it gives, by the stat's name, as its own line
    /// writes it: all a ring does, since at nought it gives nothing. Not
    /// what its tag's level adds, a weapon's accuracy, which every thing
    /// of its kind has and the pack says nowhere else either.
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
type Unlearned = (&'static ItemKind, Option<&'static ItemLevel>, &'static Unfamiliar, Has<CurseShown>, Option<&'static Resists>);

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
            gives: self.armory.gives_at(kind.0, 0).into_iter().map(|(stat, op)| (r.stats.name(stat).to_string(), op)).collect(),
            thrown: d.throw.and_then(|t| t.strike.map(|(dice, k)| strike(k.id(), dice, Some(t.range)))),
            per_level: d.level.iter().flat_map(|l| &l.stats).map(|(stat, n)| (r.stats.name(stat.id()).to_string(), *n)).collect(),
        })
    }

    /// What is known of `thing`'s level, and how long it must still be
    /// worn to tell it, the lines a panel prints in place of `unknown`:
    /// "level below nought, how far not yet known" once a curse has shown;
    /// "numbers shown at +0" over a kind's numbers at nought, which a ring,
    /// whose lines are what a level gives, has none of; then "level known in
    /// 140 more turns worn" while it is `worn`, and "wearing it 200 turns
    /// will tell its level" while it is not.
    ///
    /// No line says the level is not yet known, since the name says
    /// `unidentified` already. Below nought only while it still is: a curse
    /// shown and then enchanted up to nought came unstuck saying so, and is
    /// not known to be below anything.
    pub fn level_lines(&mut self, thing: Entity, plain: &Plain, worn: bool) -> Vec<Facet> {
        let Ok((_, level, unfamiliar, shown, _)) = self.things.get(thing) else { return Vec::new() };
        let mut lines = Vec::new();
        if shown && level.is_some_and(|l| l.0 < 0) {
            lines.push(self.facets.facet("level", "level below nought, how far not yet known").toned(Tones::BAD));
        }
        if !plain.lines().is_empty() {
            lines.push(self.facets.facet("level", "numbers shown at +0").toned(Tones::MUTED));
        }
        let left = unfamiliar.left();
        let turns = if left == 1 { "1 more turn".to_string() } else { format!("{left} more turns") };
        let wear = match (worn, unfamiliar.worn) {
            (true, _) => format!("level known in {turns} worn"),
            (false, 0) => format!("wearing it {FAMILIAR_AFTER} turns will tell its level"),
            (false, _) => format!("wearing it {turns} will tell its level"),
        };
        lines.push(self.facets.facet("level", wear).toned(Tones::MUTED));
        lines
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
    use rl_engine::rl_rules::accuracy::Odds;
    use rl_engine::rl_rules::forecast::Duel;

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

    /// A ring of light not yet known moves no armor at any level, so the
    /// vitals read the armor plain while it is worn; chain mail not yet
    /// known beside it keeps the mark.
    #[test]
    fn an_unknown_ring_that_moves_no_armor_leaves_the_armor_unmarked() {
        let (mut app, me, _) = arena_with(21, VitalsViewPlugin);
        let ring = thing(&mut app, "ring of light", 2, false);
        equip(&mut app, me, ring);
        settle(&mut app);
        let plain = vitals(&app);
        assert!(!plain.1, "an unknown ring of light, and nothing marked: {plain:?}");
        let mail = thing(&mut app, "chain mail", 2, false);
        equip(&mut app, me, mail);
        settle(&mut app);
        assert!(vitals(&app).1, "unknown chain mail beside it is marked");
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
    /// plain one's, and only the unknown one is marked. One wearer puts on
    /// each pair in turn and takes it off again, the one that clings last.
    #[test]
    fn an_unknown_worn_thing_reads_what_a_plain_one_of_its_kind_reads_whatever_its_level() {
        let (mut app, me, _) = arena_with(21, VitalsViewPlugin);
        for (name, level) in [("leather armor", 2), ("plate armor", 2), ("shield", 1), ("chain mail", 3), ("scale mail", -2)] {
            let plain_thing = thing(&mut app, name, 0, true);
            equip(&mut app, me, plain_thing);
            settle(&mut app);
            let plain = vitals(&app);
            crate::testing::unequip(&mut app, me, plain_thing);
            let unknown_thing = thing(&mut app, name, level, false);
            equip(&mut app, me, unknown_thing);
            settle(&mut app);
            let unknown = vitals(&app);
            assert_eq!(unknown, (plain.0, true), "{name} at {level:+}, not yet known, reads as a plain one, marked");
            assert!(!plain.1, "{name}, known, is not marked");
            if level >= 0 {
                crate::testing::unequip(&mut app, me, unknown_thing);
            }
        }
    }

    /// A pack row as the test below compares it: what it says in place of
    /// `unknown`, its armor, its blow, its shot, what it gives, and what the
    /// game added.
    type Said = (Vec<String>, i32, Option<String>, Option<String>, Vec<(String, Op)>, Vec<String>);

    /// What the pack's rows say of each of `things`, found and carried, its
    /// kind known: what the row for each says it is, in the order given.
    fn pack_rows(app: &mut App, me: Entity, things: &[(&str, i32)]) -> Vec<Said> {
        let mut found = Vec::new();
        for (name, level) in things {
            app.world_mut().resource_mut::<crate::lore::Lore>().learn(name);
            let thing = find(app, name, *level);
            crate::testing::give(app, me, thing);
            found.push(thing);
        }
        settle(app);
        let view = app.world().resource::<InventoryView>();
        let strike = |s: &Option<Strike>| s.as_ref().map(|s| format!("{} {}", s.dice, s.kind));
        let texts = |facets: &[Facet]| facets.iter().map(|f| f.text.clone()).collect::<Vec<_>>();
        found
            .iter()
            .map(|thing| {
                let row = view.rows.iter().find(|r| r.entity == *thing).expect("in the pack");
                (texts(&row.seems), row.armor, strike(&row.blow), strike(&row.shot), row.bestows.clone(), texts(&row.facets))
            })
            .collect()
    }

    /// Over every kind whose level is learned by wearing it, what the pack
    /// says of one found is the same at every level it could be, cursed or
    /// better, so nothing in it tells the level; and it is what a +0 one of
    /// the kind gives, which is what a known +0 one reads. One pack holds
    /// every thing at once and the rows are read together; the kinds that
    /// are learned by wearing come from the app's own armory.
    #[test]
    fn what_the_pack_says_of_a_thing_whose_level_is_not_known_is_the_same_at_every_level() {
        const KINDS: [&str; 5] = ["chain mail", "sword", "bow", "shield", "ring of regeneration"];
        const LEVELS: [i32; 5] = [0, -3, -1, 2, 3];
        let things: Vec<(&str, i32)> = KINDS.iter().flat_map(|name| LEVELS.iter().map(move |level| (*name, *level))).collect();
        let (mut app, me, _) = arena_with(25, InventoryViewPlugin);
        let rows = pack_rows(&mut app, me, &things);
        let armory = app.world().resource::<Armory>();
        let kinds: Vec<&str> = armory.defs.iter().filter(|(id, _)| armory.learned_by_wear(*id)).map(|(_, d)| d.name.as_str()).collect();
        for (n, name) in KINDS.iter().enumerate() {
            assert!(kinds.contains(name), "{name} is learned by wearing it");
            let own_rows = &rows[n * LEVELS.len()..(n + 1) * LEVELS.len()];
            let plain = &own_rows[0];
            assert!(plain.0.last().is_some_and(|l| l.starts_with("wearing it")), "{name}: {:?}", plain.0);
            for (level, row) in LEVELS.iter().zip(own_rows).skip(1) {
                assert_eq!(row, plain, "{name} at {level:+} reads as it does at nought");
            }
        }
        let chain_at_two = things.iter().position(|t| *t == ("chain mail", 2)).expect("chain mail at two is in the pack");
        assert_eq!(rows[chain_at_two].1, 3, "chain mail at two reads the plain three");
        let ring_at_two = things.iter().position(|t| *t == ("ring of regeneration", 2)).expect("the ring at two is in the pack");
        assert_eq!(rows[ring_at_two].5, ["regeneration +10 a level", "mending +25 a level"], "what each level of a ring gives");
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
