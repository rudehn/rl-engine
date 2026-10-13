//! Where someone stands: a staff of blinking's step of several cells, a
//! scroll of teleportation's, which sends its reader far off, a wand of
//! teleport other's, which sends what it strikes far off, a wand of
//! beckoning's, which pulls it close, a potion of descent's, which
//! sinks its drinker to the floor below, and a floor that gives way,
//! which drops whoever stands on it there too.
//!
//! Each on the same floor moves through [`EffectWorld::place`] or
//! [`EffectWorld::slide`], so nobody lands in a wall or on someone else,
//! and the index of who stands where and the mover's sight are kept
//! right; a descent goes by the engine's warp, which builds the floor
//! below if nobody has been there.

use bevy::prelude::*;
use rand::Rng;
use rl_engine::prelude::*;
use rl_engine::rl_rules::ability::{RawValue, read_args};

use super::FAR;
use crate::vaults::open_to_the_way_in;

/// A free cell of the place being read, drawn from `rng`, at least
/// [`FAR`] steps from `from`; with none that far, one of the furthest
/// there are. Never a transition, so a far step never lands exactly on a
/// floor's own stairs, and never a cell shut away from the way in, so
/// nobody is sent behind a vault door they cannot open from within, as
/// [`open_to_the_way_in`] says. `None` on a place with nowhere free at
/// all.
pub fn far_cell(world: &mut EffectWorld<'_, '_>, from: Point) -> Option<Point> {
    let choices = far_choices(world.map(), from, |p| world.is_free(p) && !world.is_transition(p));
    if choices.is_empty() {
        return None;
    }
    let pick = world.rng.random_range(0..choices.len());
    Some(choices[pick])
}

/// What [`far_cell`] draws among on the map being read: each cell `free`
/// admits, other than `from`, and open to the way in when the map is a
/// place, at least [`FAR`] steps from `from` or, with none that far, as
/// far as any is. Empty with nowhere free at all.
pub fn far_choices(map: &WorldMap, from: Point, free: impl Fn(Point) -> bool) -> Vec<Point> {
    let reached = map.place(map.current()).map(|place| open_to_the_way_in(place, map.tables()));
    let open: Vec<Point> = map.window_tiles().cells().filter(|p| *p != from && free(*p) && reached.as_ref().is_none_or(|r| r.contains(p))).collect();
    let Some(furthest) = open.iter().map(|p| geometry::chebyshev(*p, from)).max() else { return open };
    let far = FAR.min(furthest);
    open.into_iter().filter(|p| geometry::chebyshev(*p, from) >= far).collect()
}

/// Moves the user along the line they aimed it down, as far as its reach
/// and no further, onto the last cell of the line they can stand on: a
/// staff of blinking's. Written `(kind: "Blink", args: (reach: 2,
/// per_level: 2))`: the reach in cells, and what each of the thing's
/// [`EffectBonus`] amount adds to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Blink {
    /// The furthest it goes with nothing added.
    pub reach: i32,
    /// Cells added for each of its carrier's bonus amount.
    pub per_level: i32,
}

impl Blink {
    /// The furthest it goes with `bonus` added.
    pub fn reach_with(&self, bonus: EffectBonus) -> i32 {
        self.reach + self.per_level * bonus.amount
    }
}

impl Effect for Blink {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let reach = self.reach_with(landing.bonus).max(0) as usize;
        let line = landing.path.iter().copied().filter(|p| *p != landing.origin).take(reach);
        let mut to = None;
        for p in line {
            // Never onto a way: a monster that blinked onto the stairs
            // would stand where nothing else is ever put, off every
            // floor's own exclusion of them, and could take the player's
            // own place on a cell `GoThrough` reads literally.
            if !world.is_free(p) || world.is_transition(p) {
                break;
            }
            to = Some(p);
        }
        match to {
            Some(to) if world.place(landing.user, to) => {}
            _ => super::shows_nothing(world, landing),
        }
    }

    fn describe(&self, _: &Registries, bonus: EffectBonus) -> String {
        format!("blinks you up to {} cells", self.reach_with(bonus))
    }
}

impl FromArgs for Blink {
    const KIND: &'static str = "Blink";

    fn from_args(args: &RawValue, _: &Names<'_>) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Args {
            reach: i32,
            per_level: i32,
        }
        let a: Args = read_args(args)?;
        Ok(Self { reach: a.reach, per_level: a.per_level })
    }
}

/// Sends everyone it struck to a free cell of the floor far from whoever
/// sent them: a wand of teleport other's. Takes no arguments.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TeleportOther;

impl Effect for TeleportOther {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let mut sent = 0;
        for target in landing.targets.iter().copied().filter(|t| *t != landing.user) {
            if let Some(to) = far_cell(world, landing.origin)
                && world.place(target, to)
            {
                sent += 1;
            }
        }
        if sent == 0 {
            super::shows_nothing(world, landing);
        }
    }

    fn describe(&self, _: &Registries, _: EffectBonus) -> String {
        "sends it far away".to_string()
    }
}

impl FromArgs for TeleportOther {
    const KIND: &'static str = "TeleportOther";

    fn from_args(args: &RawValue, _: &Names<'_>) -> Result<Self, String> {
        no_args(args, Self::KIND).map(|_| Self)
    }
}

/// Sends the user to a free cell of the floor far from where they stand: a
/// scroll of teleportation's. Undermere's own, in place of the engine's
/// `Teleport`, which moves its user to where a use landed and so, for a
/// scroll read where its reader stands, nowhere. With nowhere free on the
/// floor it shows nothing. Takes no arguments.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Teleport;

impl Effect for Teleport {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        match far_cell(world, landing.origin) {
            Some(to) => {
                world.place(landing.user, to);
            }
            None => super::shows_nothing(world, landing),
        }
    }

    fn describe(&self, _: &Registries, _: EffectBonus) -> String {
        "sends you far away".to_string()
    }
}

impl FromArgs for Teleport {
    const KIND: &'static str = "Teleport";

    fn from_args(args: &RawValue, _: &Names<'_>) -> Result<Self, String> {
        no_args(args, Self::KIND).map(|_| Self)
    }
}

/// Pulls everyone it struck along the line to whoever sent it, to the
/// nearest cell beside them it can stand on: a wand of beckoning's. Takes
/// no arguments.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Beckon;

impl Effect for Beckon {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let mut pulled = 0;
        for target in landing.targets.iter().copied().filter(|t| *t != landing.user) {
            let Some(at) = world.position(target) else { continue };
            let steps = geometry::chebyshev(at, landing.origin) - 1;
            if steps > 0 && world.slide(target, at, landing.origin, steps) != at {
                pulled += 1;
            }
        }
        // A foe already beside its user, or with nowhere to come, is not
        // seen to be pulled at all.
        if pulled == 0 {
            super::shows_nothing(world, landing);
        }
    }

    fn describe(&self, _: &Registries, _: EffectBonus) -> String {
        "pulls it to your side".to_string()
    }
}

impl FromArgs for Beckon {
    const KIND: &'static str = "Beckon";

    fn from_args(args: &RawValue, _: &Names<'_>) -> Result<Self, String> {
        no_args(args, Self::KIND).map(|_| Self)
    }
}

/// Sinks the user through the floor to a random cell of the floor below:
/// a potion of descent's. From Mereholt that is the first floor; from the
/// floor above the throne's, and on the throne's, nothing is below it to
/// sink to, as [`floor_below`] says, and it shows nothing, so the log says
/// nothing seemed to happen and the potion is not learned by it. Takes no
/// arguments.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Descend;

impl Effect for Descend {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let roll = world.rng.random::<u64>();
        let landing = landing.clone();
        world.commands.queue(move |world: &mut World| descend(world, &landing, roll));
    }

    fn describe(&self, _: &Registries, _: EffectBonus) -> String {
        "sinks you through the floor to the one below".to_string()
    }
}

impl FromArgs for Descend {
    const KIND: &'static str = "Descend";

    fn from_args(args: &RawValue, _: &Names<'_>) -> Result<Self, String> {
        no_args(args, Self::KIND).map(|_| Self)
    }
}

/// Someone sinking to the place `to`, who lands on the cell `roll` picks
/// among its free ones once they are there.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sinking {
    /// The place they sink to.
    pub to: MapId,
    /// What picks the cell they land on, drawn from the effect stream as
    /// the potion was drunk.
    pub roll: u64,
}

/// Sends `landing`'s user to the floor below the one they stand on, as
/// [`Descend`] says, or marks the use as having shown nothing.
pub fn descend(world: &mut World, landing: &Landing, roll: u64) {
    let user = landing.user;
    let here = world.get::<OnMap>(user).map_or(MapId::SURFACE, |m| m.0);
    let Some(below) = floor_below(here) else {
        super::showed_nothing(world, landing);
        return;
    };
    world.entity_mut(user).insert(Sinking { to: below, roll });
    world.write_message(WarpRequest::into_place(user, below));
    if world.get::<Player>(user).is_some() {
        world.write_message(Tell::new("You sink through the floor!", Tones::NOTICE));
    }
}

/// The floor someone sinking or falling through the floor of `here` lands
/// on: the first from Mereholt, the next one down from a floor. Never the
/// throne's, which is entered by its stairs alone, as its trapdoors are
/// never laid on the floor above it, so a rotten floor given way or a
/// potion drunk there drops nobody into its hall; and nothing from the
/// throne's floor or anywhere else.
pub fn floor_below(here: MapId) -> Option<MapId> {
    use crate::dungeon::{FLOORS, TOWN, floor_of, map_of};
    match floor_of(here) {
        Some(floor) if floor + 1 < FLOORS => Some(map_of(floor + 1)),
        None if here == TOWN => Some(map_of(1)),
        _ => None,
    }
}

/// Whoever stands where it lands falls through the floor to the one
/// below, as a rotten floor drops its taker when it gives way and a
/// trapdoor whoever steps on it: [`fall_through`] for each. Nothing on the
/// throne's floor or the one above it, as [`floor_below`] says, or in
/// Mereholt, whose ground is no floor to fall through. Takes no
/// arguments.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Fall;

impl Effect for Fall {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        for who in landing.targets.clone() {
            let roll = world.rng.random::<u64>();
            world.commands.queue(move |world: &mut World| {
                fall_through(world, who, roll);
            });
        }
    }

    fn describe(&self, _: &Registries, _: EffectBonus) -> String {
        "drops whoever stands there through the floor to the one below".to_string()
    }
}

impl FromArgs for Fall {
    const KIND: &'static str = "Fall";

    fn from_args(args: &RawValue, _: &Names<'_>) -> Result<Self, String> {
        no_args(args, Self::KIND).map(|_| Self)
    }
}

/// Drops `who` through the floor of the place they stand on to the floor
/// below, landing where `roll` picks among its free cells off the stairs,
/// as [`land_sinking`] lands a potion of descent's drinker; false, and
/// nothing done, where [`floor_below`] has nothing below or anywhere that
/// is no floor. The one fall, which [`Fall`] lands and a rotten floor's
/// own collapse calls.
pub fn fall_through(world: &mut World, who: Entity, roll: u64) -> bool {
    let here = world.get::<OnMap>(who).map_or(MapId::SURFACE, |m| m.0);
    let Some(below) = floor_below(here).filter(|_| here != crate::dungeon::TOWN) else { return false };
    world.entity_mut(who).insert(Sinking { to: below, roll });
    world.write_message(WarpRequest::into_place(who, below));
    if world.get::<Player>(who).is_some() {
        world.write_message(Tell::new("The floor gives way beneath you!", Tones::BAD));
    }
    true
}

/// Puts whoever a potion of descent sank, or a floor dropped, on a free
/// cell of the floor they went to, picked by the roll they went with, once
/// they have arrived: a floor first entered is built, and its people
/// placed, by the arrival, so the cell is picked among what is free once
/// it stands, among [`landing_cells`]: never on the stairs, up or down,
/// nor anywhere shut away from the way in.
///
/// In `TurnSet::React`, after the arrival's own reactions, by the engine's
/// warp, so who stands where and what can be seen are kept right as any
/// warp keeps them; the warp comes in the next pass.
pub fn land_sinking(
    mut commands: Commands,
    sinking: Query<(Entity, &Sinking, &OnMap)>,
    map: Res<WorldMap>,
    occupancy: Res<Occupancy>,
    stairs: Query<(&Position, &OnMap), crate::stairs::WayThrough>,
    mut warps: MessageWriter<WarpRequest>,
) {
    for (who, sink, on) in &sinking {
        if on.0 != sink.to {
            continue;
        }
        commands.entity(who).remove::<Sinking>();
        let on_stairs = |p: Point| stairs.iter().any(|(at, m)| m.0 == sink.to && at.0 == p);
        let open = landing_cells(&map, sink.to, |p| !occupancy.is_occupied(p) && !on_stairs(p));
        if open.is_empty() {
            continue;
        }
        let at = open[(sink.roll % open.len() as u64) as usize];
        warps.write(WarpRequest { actor: who, to: Destination::Place { map: sink.to, arrive: Arrive::At(at) } });
    }
}

/// Every cell of the place `to` a fall or a potion of descent may set
/// someone down on, in the same order every run: ground to stand on that `free` admits,
/// off the place's way in and way down, which nobody comes through a
/// ceiling to land on and which a fall onto the way down would take
/// straight on to the next floor's, and open to the way in, as
/// [`open_to_the_way_in`] says, so nobody lands behind a vault door
/// whose key lies outside it. Empty for a map that is no place.
pub fn landing_cells(map: &WorldMap, to: MapId, free: impl Fn(Point) -> bool) -> Vec<Point> {
    let Some(place) = map.place(to) else { return Vec::new() };
    let reached = open_to_the_way_in(place, map.tables());
    reached.into_iter().filter(|p| *p != place.entry && Some(*p) != place.exit && map.is_walkable_on(to, *p) && free(*p)).collect::<Vec<Point>>()
}

/// Refuses any argument at all, for an effect that takes none: a field
/// written for one is a typo for another effect, or a number that would
/// quietly do nothing.
pub fn no_args(args: &RawValue, kind: &str) -> Result<(), String> {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Nothing {}
    let text = args.get_ron().trim();
    if text.is_empty() || text == "()" {
        return Ok(());
    }
    read_args::<Nothing>(args).map(|_| ()).map_err(|e| format!("{kind} takes no arguments: {e}"))
}
