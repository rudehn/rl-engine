//! The pick each charge offers, once its console is set.
//!
//! Three permanent upgrades, kept for the run: [`Upgrade::Stims`] grants
//! an ability, [`Upgrade::Servos`] raises [`Speed`], and
//! [`Upgrade::Uplink`] raises the range of every ranged weapon worn, one
//! tile, for good. [`apply`] does the one picked; [`Choosing`] is `Some`
//! while a pick is on screen, and [`choice_keys`] is the whole of reading
//! it, since nothing about a pick is a turn: no action claims it, and the
//! pick closes without ending the run. [`Taken`] is what keeps four picks
//! against a pool of three from offering, or applying, the same upgrade
//! twice.
//!
//! Uplink's bonus is applied and removed, not derived fresh every pass the
//! way `ammo::sync_ammo` and `droids::sensors::sync_dark_sight` compute
//! theirs. Both of those exist because two different sources could
//! legitimately disagree about one field on the same pass (a bag two
//! weapons draw from, a jam and a helmet both claiming sight), so a system
//! that only ever adds what it once subtracted was the bug, not the fix.
//! Uplink has no second source to race: [`react_uplink`] and [`apply`]'s
//! own `raise` are the only code that ever touches a weapon's range for
//! this reason, both guarded by [`Reached`] against ever adding it twice,
//! and the run has no way to un-pick an upgrade once made. A derived
//! design would also need a stored "range before any bonus" nowhere in
//! the file currently keeps (`RangedAttack::range` is both the label and
//! the value in one field), and the only place to add it is
//! `gear::spawn_item`, which this pick does not touch. `heat::vent_heat`
//! and `ammo::sync_ammo` only ever relocate a weapon's whole
//! `RangedAttack` between it and
//! [`Stowed::Ranged`] wholesale; neither
//! reads or rewrites the number inside, so the bonus rides along through a
//! lock or a dry spell for free.

use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_core::Rect;

use crate::heat::Stowed;

/// What finishing the first objective may give, one of three, kept for the
/// run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Upgrade {
    /// An ability that patches you up.
    Stims,
    /// One more tile of reach on every ranged weapon worn.
    Uplink,
    /// A tenth faster at everything.
    Servos,
}

/// The pool every charge's pick draws from, from spec section 11.
pub const OFFERED: [Upgrade; 3] = [Upgrade::Stims, Upgrade::Uplink, Upgrade::Servos];

/// What this run has already fitted, in the order it was picked.
///
/// A resource rather than components read off the player, because what an
/// upgrade did to the player is not always something to read back: servos
/// raised a number that armor could raise too, and stims pushed an id
/// onto a list that an implant will push onto later. The pick needs to
/// know what it offered before, which is its own question.
#[derive(Resource, Debug, Clone, Default)]
pub struct Taken(pub Vec<Upgrade>);

/// The upgrades a pick offers given what the run has: the pool in
/// [`OFFERED`] order, minus what is taken, at most three rows because
/// three is what the screen and the design both say a pick is.
///
/// Empty is a real answer, not a bug: four charges against a pool of
/// three means the fourth pick has nothing to give, and a pick with no
/// rows closes itself rather than offering a repeat.
pub fn on_offer(taken: &Taken) -> Vec<Upgrade> {
    OFFERED.iter().copied().filter(|u| !taken.0.contains(u)).take(3).collect()
}

/// The name and the one-line pitch a menu row shows for `upgrade`.
fn blurb(upgrade: Upgrade) -> (&'static str, &'static str) {
    match upgrade {
        Upgrade::Stims => ("Stims", "An ability that closes a wound, once every twenty turns."),
        Upgrade::Uplink => ("Uplink", "One more tile of reach on every ranged weapon you wear."),
        Upgrade::Servos => ("Servos", "A tenth faster at everything, for good."),
    }
}

/// Marks the player once [`Upgrade::Uplink`] is picked. Never removed:
/// the run has no way to take an upgrade back.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Uplinked;

/// A ranged weapon [`react_uplink`] or [`apply`] has already raised by the
/// uplink's one tile: the guard against raising the same weapon twice, and
/// what [`react_uplink`] reads on an `ItemEvent::Unequipped` to know
/// whether the tile it is taking off was ever this system's to give.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Reached;

/// Applies `upgrade` to `player`, once, for the run. Called from the pick
/// screen when a choice is confirmed, and directly by a test through
/// [`crate::testing::pick`].
pub fn apply(upgrade: Upgrade, player: Entity, world: &mut World) {
    match upgrade {
        Upgrade::Stims => {
            let stims = world.resource::<Abilities>().expect("stims");
            match world.get_mut::<Grants>(player) {
                Some(mut grants) => grants.0.push(stims),
                None => {
                    world.entity_mut(player).insert(Grants(vec![stims]));
                }
            }
        }
        Upgrade::Servos => match world.get_mut::<Speed>(player) {
            Some(mut speed) => speed.0 += 10,
            None => {
                world.entity_mut(player).insert(Speed(110));
            }
        },
        Upgrade::Uplink => {
            world.entity_mut(player).insert(Uplinked);
            let worn: Vec<Entity> = world.get::<Equipped>(player).map(|e| e.0.worn().map(|(_, item)| item).collect()).unwrap_or_default();
            for item in worn {
                raise(world, item);
            }
        }
    }
}

/// Raises `item`'s ranged reach by one tile, wherever its attack currently
/// lives (worn, or [`Stowed::Ranged`] while locked or dry), and tags it
/// [`Reached`]. Does nothing to an item already tagged, or one with no
/// ranged attack to raise at all.
fn raise(world: &mut World, item: Entity) {
    if world.get::<Reached>(item).is_some() {
        return;
    }
    let raised = if let Some(mut attack) = world.get_mut::<RangedAttack>(item) {
        attack.range += 1;
        true
    } else if let Some(mut stowed) = world.get_mut::<Stowed>(item) {
        match &mut *stowed {
            Stowed::Ranged(attack) => {
                attack.range += 1;
                true
            }
            Stowed::Melee(_) => false,
        }
    } else {
        false
    };
    if raised {
        world.entity_mut(item).insert(Reached);
    }
}

/// Reacts to every [`ItemEvent`]: an actor already [`Uplinked`] that
/// equips a ranged weapon raises it on the spot, and unequipping one
/// raised gives its tile back. A typed reader rather than exclusive, the
/// way every other reactive system in this game is (`heat::heat_on_struck`,
/// `ammo::spend_ammo`): reading `ItemEvent` through `Messages::drain`
/// instead would starve any other system that ever reads the same
/// message, so this repeats `raise`'s few lines against queries rather
/// than share it with [`apply`]'s `World`-based version.
///
/// Ordered after `heat`'s and `ammo`'s own `TurnSet::React` systems, which
/// move a weapon's `RangedAttack` in and out of [`Stowed`] by command, a
/// copy of the attack taken as the command is written. Run before those
/// commands apply, a raise would land on the copy being replaced and be
/// lost with it, while [`Reached`] still said it was given: equipping an
/// empty slug pistol from the ground is an equip and a stowing in one
/// pass. After them, the attack is wherever this pass left it.
pub fn react_uplink(
    mut commands: Commands,
    mut events: MessageReader<ItemEvent>,
    uplinked: Query<(), With<Uplinked>>,
    reached: Query<(), With<Reached>>,
    mut rangeds: Query<&mut RangedAttack>,
    mut stoweds: Query<&mut Stowed>,
) {
    for ev in events.read() {
        match *ev {
            ItemEvent::Equipped { actor, item } if uplinked.contains(actor) && !reached.contains(item) => {
                let raised = match rangeds.get_mut(item) {
                    Ok(mut attack) => {
                        attack.range += 1;
                        true
                    }
                    Err(_) => match stoweds.get_mut(item) {
                        Ok(mut stowed) => match &mut *stowed {
                            Stowed::Ranged(attack) => {
                                attack.range += 1;
                                true
                            }
                            Stowed::Melee(_) => false,
                        },
                        Err(_) => false,
                    },
                };
                if raised {
                    commands.entity(item).insert(Reached);
                }
            }
            ItemEvent::Unequipped { item, .. } if reached.contains(item) => {
                match rangeds.get_mut(item) {
                    Ok(mut attack) => attack.range -= 1,
                    Err(_) => {
                        if let Ok(mut stowed) = stoweds.get_mut(item)
                            && let Stowed::Ranged(attack) = &mut *stowed
                        {
                            attack.range -= 1;
                        }
                    }
                }
                commands.entity(item).remove::<Reached>();
            }
            _ => {}
        }
    }
}

/// The upgrades on screen while a pick is open, or `None` while none is.
///
/// A `Vec` rather than `[Upgrade; 3]`: a pick late in a run offers fewer
/// than three, since it never offers what the run already has.
#[derive(Resource, Default)]
pub struct Choosing(pub Option<Vec<Upgrade>>);

/// The pick screen's own list: three rows, one line each.
#[derive(Resource, Default)]
pub struct ChoiceScreen {
    menu: ListMenu,
}

/// The name the pick's modal is declared under, and the id
/// [`FoundryPlugin`](crate::plugin::FoundryPlugin) declares it with.
pub const MODAL: &str = "choice";

/// The id of the pick's modal.
fn modal(modals: &Modals) -> ModalId {
    modals.get(MODAL).expect("FoundryPlugin declares the choice modal")
}

/// Opens the pick on whatever is left to offer. A pick with nothing left
/// opens nothing, which is what the fourth charge does against a pool of
/// three: the charge still counts, and the screen does not appear to
/// offer a choice that is not there.
pub fn offer(modals: &mut Modals, screen: &mut ChoiceScreen, choosing: &mut Choosing, taken: &Taken) {
    let rows = on_offer(taken);
    if rows.is_empty() {
        return;
    }
    screen.menu.title = "The charge is set. Choose one upgrade".to_string();
    screen.menu.hints = "\u{2191}\u{2193} pick \u{2022} enter choose".to_string();
    screen.menu.set_rows(
        rows.iter()
            .map(|u| {
                let (name, text) = blurb(*u);
                MenuRow::new(name).detail(text)
            })
            .collect(),
    );
    choosing.0 = Some(rows);
    let id = modal(modals);
    modals.open(id);
}

/// Reads the pick screen's keys, exclusively: moving the cursor is a
/// resource write same as any menu's, but confirming calls [`apply`],
/// which needs the whole [`World`] the way [`crate::testing::pick`] does.
/// No action claims a turn for this: the pick costs no turn, and the run
/// carries on once it closes.
pub fn choice_keys(world: &mut World) {
    let id = modal(world.resource::<Modals>());
    if !world.resource::<Modals>().is_top(id) {
        return;
    }
    let (up, down, confirm) = {
        let keys = world.resource::<ButtonInput<KeyCode>>();
        let directions = world.resource::<DirectionKeys>();
        let cursor = world.resource::<CursorKeys>();
        let held = |want: Direction| directions.0.iter().any(|(k, d)| *d == want && keys.just_pressed(*k));
        (held(Direction::North), held(Direction::South), keys.just_pressed(cursor.confirm) || keys.just_pressed(cursor.also_confirm))
    };
    let mut screen = world.resource_mut::<ChoiceScreen>();
    if up {
        screen.menu.move_by(-1);
    }
    if down {
        screen.menu.move_by(1);
    }
    if !confirm {
        return;
    }
    let selected = screen.menu.selected;
    let Some(upgrade) = world.resource::<Choosing>().0.as_ref().and_then(|rows| rows.get(selected).copied()) else { return };
    let Some(player) = world.query_filtered::<Entity, With<Player>>().iter(world).next() else { return };
    apply(upgrade, player, world);
    world.resource_mut::<Taken>().0.push(upgrade);
    world.resource_mut::<Modals>().close_one(id);
    world.resource_mut::<Choosing>().0 = None;
}

/// The pick's presenter: draws [`ChoiceScreen`] in the rectangle it is
/// built with, the way the engine's own panels take theirs, so `main.rs`
/// cuts it from the screen beside every other panel rather than the
/// screen working out where to sit on its own. A headless test adds none,
/// and the pick still opens, reads its keys and applies the upgrade
/// without it.
pub struct ChoicePanel(pub Rect);

/// Where [`ChoicePanel`] draws.
#[derive(Resource, Debug, Clone, Copy)]
struct ChoiceRect(Rect);

impl Plugin for ChoicePanel {
    fn build(&self, app: &mut App) {
        app.insert_resource(ChoiceRect(self.0)).add_systems(Update, draw_choice.in_set(PresentSet::Overlay));
    }
}

/// Draws the pick screen while it is open.
fn draw_choice(screen: Res<ChoiceScreen>, modals: Res<Modals>, palette: Res<Palette>, rect: Res<ChoiceRect>, mut terminal: ResMut<Terminal>) {
    if !modals.is_open(modal(&modals)) {
        return;
    }
    draw_menu(&mut terminal, rect.0, &screen.menu, &palette);
}

/// Loads Foundry's one ability, `stims`, granted by the first upgrade.
/// Seed-independent, so it is loaded once, outside `NewRun`, the way
/// `main.rs` and `testing::headless` load `Registries`.
pub fn load_abilities(kinds: &EffectKinds, registries: &Registries) -> Abilities {
    const ABILITIES_RON: &str = include_str!("../assets/abilities.ron");
    Abilities::load(ABILITIES_RON, kinds, &registries.names()).unwrap_or_else(|e| panic!("assets/abilities.ron: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rl_engine::rl_core::RunSeed;

    /// A pick never offers what the run already has, so four charges
    /// across a run give four different upgrades rather than the same
    /// three over and over, and the pick after the pool runs dry offers
    /// nothing rather than a repeat.
    #[test]
    fn a_pick_offers_only_what_the_run_has_not_taken() {
        let all = Taken(Vec::new());
        assert_eq!(on_offer(&all).len(), 3, "nothing taken: the whole pool, capped at three");
        let two_left = Taken(vec![Upgrade::Stims]);
        let offer = on_offer(&two_left);
        assert!(!offer.contains(&Upgrade::Stims), "what is taken is not offered again");
        assert_eq!(offer.len(), 2);
        let empty = Taken(vec![Upgrade::Stims, Upgrade::Uplink, Upgrade::Servos]);
        assert!(on_offer(&empty).is_empty(), "a dry pool offers nothing");
    }

    #[test]
    fn each_upgrade_does_what_it_says() {
        // Stims: an ability to use. Uplink: a shot reaches one further, on
        // the weapon worn now and on one put on later. Servos: ten percent
        // faster.
        let mut app = crate::testing::headless(RunSeed(2));
        let (player, blaster) = crate::testing::player_with_hand_blaster(&mut app);
        for u in OFFERED {
            crate::testing::pick(&mut app, player, u);
        }
        assert!(crate::testing::knows(&app, player, "stims"));
        assert_eq!(app.world().get::<RangedAttack>(blaster).unwrap().range, 6, "five, and one more");
        assert_eq!(app.world().get::<Speed>(player).map(|s| s.0), Some(110));
        let carbine = crate::testing::equip_new(&mut app, player, "blaster carbine");
        assert_eq!(app.world().get::<RangedAttack>(carbine).unwrap().range, 10, "nine, and one more, put on after the pick");
    }

    #[test]
    fn taking_off_an_uplinked_weapon_takes_the_extra_reach_with_it() {
        let mut app = crate::testing::headless(RunSeed(2));
        let (player, blaster) = crate::testing::player_with_hand_blaster(&mut app);
        crate::testing::pick(&mut app, player, Upgrade::Uplink);
        crate::testing::unequip(&mut app, player, blaster);
        assert_eq!(app.world().get::<RangedAttack>(blaster).unwrap().range, 5, "back to what the file says");
    }

    #[test]
    fn a_weapon_locked_by_heat_when_uplink_is_picked_comes_back_raised() {
        let mut app = crate::testing::headless(RunSeed(1));
        let (player, first, _second) = crate::testing::dual_blasters(&mut app);
        crate::testing::fire_at_a_target(&mut app, player, 7);
        assert!(app.world().get::<RangedAttack>(first).is_none(), "locked: stowed");
        crate::testing::pick(&mut app, player, Upgrade::Uplink);
        let Stowed::Ranged(attack) = app.world().get::<Stowed>(first).unwrap() else { panic!("still stowed as ranged") };
        assert_eq!(attack.range, 6, "raised while stowed");
        crate::testing::pass_turns(&mut app, 6);
        assert_eq!(app.world().get::<RangedAttack>(first).unwrap().range, 6, "cooled off with its extra reach");
    }

    #[test]
    fn a_weapon_gone_dry_when_uplink_is_picked_comes_back_raised() {
        let mut app = crate::testing::headless(RunSeed(1));
        let (player, pistol) = crate::testing::slug_pistol_with(&mut app, 0);
        assert!(app.world().get::<RangedAttack>(pistol).is_none(), "dry from the start");
        crate::testing::pick(&mut app, player, Upgrade::Uplink);
        let Stowed::Ranged(attack) = app.world().get::<Stowed>(pistol).unwrap() else { panic!("still stowed as ranged") };
        assert_eq!(attack.range, 7, "raised while dry");
        crate::testing::give_slugs(&mut app, player, 3);
        app.update();
        assert_eq!(app.world().get::<RangedAttack>(pistol).unwrap().range, 7, "loaded again with its extra reach");
    }
}
