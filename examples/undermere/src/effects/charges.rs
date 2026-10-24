//! What its user's things hold: a scroll of recharging's every staff made
//! full and every wand given a charge more.
//!
//! A charge is the engine's [`Consumable`], counted on the thing; which
//! things are staffs and which wands is their tags, so the scroll finds
//! them in its reader's pack as the bag shows them.

use atglyph::prelude::all::*;
use atglyph::rules::ability::RawValue;
use bevy::prelude::*;

use super::moving::no_args;

/// Every staff its user carries made full, and every wand given one charge
/// more, past what it held when it was found if need be: a scroll of
/// recharging's. With neither in the pack it shows nothing. Takes no
/// arguments.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Recharge;

impl Effect for Recharge {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let landing = landing.clone();
        world.commands.queue(move |world: &mut World| {
            if !recharge(world, landing.user) {
                super::showed_nothing(world, &landing);
            }
        });
    }

    fn describe(&self, _: &Registries, _: EffectBonus) -> String {
        "fills your staffs and puts a charge in each wand".to_string()
    }
}

impl FromArgs for Recharge {
    const KIND: &'static str = "Recharge";

    fn from_args(args: &RawValue, _: &Names<'_>) -> Result<Self, String> {
        no_args(args, Self::KIND).map(|_| Self)
    }
}

/// Fills every staff `reader` carries and puts a charge in every wand, as
/// [`Recharge`] says, and says so. False when they carry neither.
pub fn recharge(world: &mut World, reader: Entity) -> bool {
    let registries = world.resource::<Registries>();
    let (Some(staff), Some(wand)) = (registries.tags.id("staff"), registries.tags.id("wand")) else { return false };
    let carried: Vec<Entity> = world.get::<Inventory>(reader).map(|bag| bag.items.clone()).unwrap_or_default();
    let mut filled = 0;
    for thing in carried {
        let Some(tags) = world.get::<Tagged>(thing).cloned() else { continue };
        let Some(mut held) = world.get_mut::<Consumable>(thing) else { continue };
        if tags.contains(&staff) {
            held.left = held.max;
            if let Some(pace) = held.recharge.as_mut() {
                pace.progress = 0;
            }
            filled += 1;
        } else if tags.contains(&wand) {
            held.left += 1;
            held.max = held.max.max(held.left);
            filled += 1;
        }
    }
    if filled > 0 && world.get::<Player>(reader).is_some() {
        world.write_message(Tell::new("Your staffs and wands hum with power.", Tones::GOOD));
    }
    filled > 0
}
