//! What a body does when a blow lands on it, or it lands one, or it
//! steps: a jelly splitting, a bat or a wight drinking what it wounds, a
//! salamander's trail of fire, and a wraith's withering touch.

use super::*;

/// Splits into two when a blow leaves it alive, each with half its
/// health: a pink jelly's `splitting`. Marked at spawn off [`Tricks::splits`],
/// and judged by [`judge_splits`] against `DamageDealt` rather than the
/// trait model's own `struck`, which fires before a blow's damage is
/// known.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Splits;

/// The least health a body must have left to split at all: two halves of
/// the trait's own "none below 2". Below it a blow splits nothing, since
/// halves floored at two each came to more health than the one they split
/// from, and every split of a small jelly grew the floor's.
const SPLIT_MIN_HEALTH: i32 = 4;

/// Splits whoever a blow of this pass left alive and marked [`Splits`]:
/// half its current health each, rounded down, the second made on a free
/// cell beside the first. Nothing for one the blow killed, read fresh off
/// its `Health` rather than off the event, nothing for one left with less
/// than `SPLIT_MIN_HEALTH`, and nothing for one with no free cell to
/// stand a second half on.
///
/// A blow, not any harm: a status's own tick, a burn from the fire it
/// stands in among them, carries the status it is the tick of and is
/// passed over. A jelly in a fire otherwise split on every turn it burned.
///
/// In `TurnSet::React`, after `apply_damage` has written `DamageDealt`.
pub fn judge_splits(mut dealt: MessageReader<DamageDealt>, splitting: Query<(), With<Splits>>, mut commands: Commands) {
    let mut struck: Vec<Entity> = Vec::new();
    for hit in dealt.read().filter(|d| d.dealt > 0 && d.hit.status.is_none() && splitting.contains(d.target)) {
        if !struck.contains(&hit.target) {
            struck.push(hit.target);
        }
    }
    for target in struck {
        commands.queue(move |world: &mut World| {
            try_split(world, target);
        });
    }
}

/// `target`, as [`judge_splits`] says, and whether it did.
fn try_split(world: &mut World, target: Entity) -> bool {
    if world.get::<Splits>(target).is_none() {
        return false;
    }
    let Some(health) = world.get::<Health>(target).copied() else { return false };
    if health.current < SPLIT_MIN_HEALTH {
        return false;
    }
    let Some(kind) = world.get::<Kind>(target).map(|k| k.0) else { return false };
    let Some(at) = world.get::<Position>(target).map(|p| p.0) else { return false };
    let map = world.get::<OnMap>(target).map_or(MapId::SURFACE, |m| m.0);
    let beside = free_cells_near(world.resource::<WorldMap>(), world.resource::<Occupancy>(), at, 1, 1);
    let Some(second_at) = beside.first().copied() else { return false };
    let half = health.current / 2;
    world.entity_mut(target).insert(Health { current: half, max: health.max });
    let made = spawn_monster_now(world, kind, second_at, map);
    world.entity_mut(made).insert(Health { current: half, max: health.max });
    // Told only where it is seen, as `withering`'s own touch is by the
    // engine's own narrator: a split in a room nobody is watching is not
    // something the adventurer learns of from the log either. Learned here,
    // where the split is, rather than off the blow: a blow too small to
    // split anything, or with no room beside it, shows nothing.
    if seen_by_player(world, at) {
        let name = world.get::<Name>(target).map_or_else(String::new, |n| n.as_str().to_string());
        world.write_message(Tell::new(format!("{} splits into two!", crate::effects::the(&name)), Tones::NOTICE));
        let kind = world.resource::<Bestiary>().defs.get(kind).name.clone();
        world.resource_mut::<MonsterLore>().show(&kind, "splitting");
    }
    true
}

/// Marks a kind that mends itself what a landed blow of its own dealt: a
/// vampire bat's and a wight's `draining`. Judged by [`judge_draining`]
/// against `DamageDealt`, since nothing yet knows what a bite "dealt" at
/// the moment `hit` itself fires, which is before a blow's damage is
/// resolved.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Drains;

/// Mends every `Drains` attacker the health its own landed blow took this
/// pass, by the engine's `Heal`, scaled by its own mending as any heal is:
/// a bite drains no more life than its victim had left.
///
/// `hit.attacker` rather than `hit.credit`: the one who triggers an
/// on-hit rider, which is what draining is, and never damage over time or
/// splash crediting a kill to somebody who threw nothing this pass.
///
/// In `TurnSet::React`, after `apply_damage` has written `DamageDealt`.
pub fn judge_draining(mut dealt: MessageReader<DamageDealt>, draining: Query<(), With<Drains>>, mut heals: MessageWriter<Heal>) {
    for hit in dealt.read().filter(|d| d.lost > 0) {
        if let Some(attacker) = hit.hit.attacker
            && draining.contains(attacker)
        {
            heals.write(Heal::new(attacker, hit.lost).by(attacker));
        }
    }
}

/// Marks a kind whose steps leave fire behind them: a salamander's
/// `fire-trail`.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct FireTrail;

/// How many turns a fire-trail's own step burns: short, so it marks the
/// kind's own path rather than setting the whole floor alight behind it.
const FIRE_TRAIL_TURNS: u8 = 3;

/// Ignites the cell a fire-trailing kind just stepped off, `Stepped`'s
/// `from` rather than `to`: it stands on `to` now, and need not catch
/// fire under its own feet.
///
/// In `TurnSet::React`, after the engine has reported every step of the
/// pass.
pub fn trail_fire(mut stepped: MessageReader<Stepped>, trailing: Query<(), With<FireTrail>>, mut kindle: MessageWriter<Kindle>) {
    for step in stepped.read().filter(|s| trailing.contains(s.actor)) {
        kindle.write(Kindle { at: step.from, turns: FIRE_TRAIL_TURNS });
    }
}

/// Caps `weakened`'s own stacking at [`WEAKENED_MAX_STACKS`]: a wraith's
/// `withering`. `Inflict` has nowhere to say "no more than this many
/// instances of me at once", since the engine's `Stacking::Stack` has no
/// maximum of its own; this is that maximum, for one status. Written
/// `(kind: "Wither", args: (turns: 20))`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wither {
    /// How long each stack lasts.
    pub turns: u32,
}

impl Effect for Wither {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let (by, turns, targets) = (Some(landing.user), self.turns, landing.targets.clone());
        world.commands.queue(move |world: &mut World| {
            for target in targets {
                wither(world, target, by, turns);
            }
        });
    }

    fn describe(&self, _: &Registries, _: EffectBonus) -> String {
        format!("saps strength for {} turns, up to three stacks", self.turns)
    }
}

impl FromArgs for Wither {
    const KIND: &'static str = "Wither";

    fn from_args(args: &RawValue, _: &Names<'_>) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Args {
            turns: u32,
        }
        let a: Args = read_args(args)?;
        Ok(Self { turns: a.turns })
    }
}

/// How many stacks of `weakened` [`Wither`] lets pile onto one target: the
/// content sheet's own cap.
pub const WEAKENED_MAX_STACKS: usize = 3;

/// Inflicts `weakened` on `target` for `turns`, credited to `by`, unless
/// it already holds [`WEAKENED_MAX_STACKS`] instances: a fourth touch
/// before the first's clock runs out is refused rather than let the stat
/// climb without bound.
pub fn wither(world: &mut World, target: Entity, by: Option<Entity>, turns: u32) {
    let Some(weakened) = world.resource::<Registries>().statuses.id(crate::content::WEAKENED) else { return };
    let stacks = world.get::<Afflicted>(target).map_or(0, |a| a.0.iter().filter(|s| s.id == weakened).count());
    if stacks >= WEAKENED_MAX_STACKS {
        return;
    }
    world.write_message(Afflict { target, status: weakened, turns, by, held_by: None });
}

#[cfg(test)]
mod tests {
    use rl_engine::rl_bevy::status::resolve_afflictions;

    use super::*;

    /// `Wither` refuses a fourth and a fifth stack of `weakened` rather
    /// than let it climb without bound.
    #[test]
    fn wither_caps_weakened_at_three_stacks() {
        let mut app = App::new();
        app.add_message::<Afflict>().add_message::<Cure>().add_message::<rl_engine::rl_bevy::status::StatusEvent>();
        app.insert_resource(crate::content::registries());
        app.add_systems(Update, resolve_afflictions);
        let e = app.world_mut().spawn((Afflicted(Statuses::new()), StatBlock(Stats::new()))).id();
        for _ in 0..5 {
            wither(app.world_mut(), e, None, 20);
            app.update();
        }
        let weakened = app.world().resource::<Registries>().statuses.expect(crate::content::WEAKENED);
        let stacks = app.world().get::<Afflicted>(e).unwrap().0.iter().filter(|s| s.id == weakened).count();
        assert_eq!(stacks, WEAKENED_MAX_STACKS, "a fourth and a fifth touch are both refused");
    }
}
