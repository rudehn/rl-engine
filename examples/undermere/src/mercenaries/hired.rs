//! A candidate taken up: on the adventurer's side, following them, and
//! offering what a hired hand offers.

use bevy::prelude::*;
use rl_engine::prelude::*;
use serde::{Deserialize, Serialize};

use super::*;
use crate::allies::befriend;

/// The verb a hired mercenary offers to be handed something from the pack.
pub const GIVE: &str = "give";

/// What handing a mercenary something takes, in hundredths of a step: one
/// step.
pub const GIVE_TIME: u32 = 100;

/// The verb a hired mercenary offers to be let go.
pub const DISMISS: &str = "dismiss";

/// What letting a mercenary go takes, in hundredths of a step: one step,
/// as hiring it did. The offer asks first, so only a yes spends it.
pub const DISMISS_TIME: u32 = 100;

/// A sword the adventurer hired: who it is, what it cost, and what it has
/// been told to do.
///
/// Its name is kept here as well as on its `Name`, since the tally names
/// the fallen after the body is gone, and the fee is what it was hired
/// for, which a dismissal reads. The order is the game's to keep: the
/// brain for it is put back from it, never saved.
#[derive(Component, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mercenary {
    /// What it is called, a name the run gave no one else.
    pub name: String,
    /// What it was hired for, in gold coins.
    pub fee: u32,
    /// What it has been told to do.
    pub order: Order,
}

/// What a hired mercenary called `name` offers the adventurer: [`GIVE`],
/// answered by [`answer_give`], then [`DISMISS`], answered by
/// [`answer_dismiss`]. Giving chooses the thing first, in its name, among
/// what [`give_choices`] names, so backing out of the pack costs nothing;
/// dismissing asks first, in its name, since it cannot be taken back.
pub fn hired_offers(world: &World, name: &str) -> Offers {
    let verbs = world.resource::<Verbs>();
    let among = world.resource::<GiveAmong>().0;
    let question = format!("Dismiss {name}? The fee is not given back, and {name} sets down everything you gave.");
    Offers(vec![
        Offering::new(verbs.get(GIVE).expect("UndermerePlugin declares give"), GIVE_TIME).choosing(format!("Give {name} which?"), among),
        Offering::new(verbs.get(DISMISS).expect("UndermerePlugin declares dismiss"), DISMISS_TIME).asking(question),
    ])
}

/// Hires `candidate` for `adventurer`: off the tavern's list, on the
/// adventurer's side by [`befriend`], following them by `Leader`, a
/// [`Mercenary`] on [`Order::Follow`] with the follow brain, and offering
/// [`GIVE`] and [`DISMISS`] in place of `hire` and talk.
///
/// The fee the engine paid into its pack is spent there and then, gone
/// from play: what it carries is what it was hired with, so neither a
/// death nor a dismissal hands the adventurer back their own coin.
/// Nothing for an entity that is no candidate.
pub fn hire(world: &mut World, candidate: Entity, adventurer: Entity) {
    let Some(Candidate { fee }) = world.get::<Candidate>(candidate).copied() else { return };
    let name = world.get::<Name>(candidate).map_or_else(String::new, |n| n.as_str().to_string());
    spend_fee(world, candidate);
    befriend(world, candidate);
    let offers = hired_offers(world, &name);
    let hired = Mercenary { name, fee, order: Order::Follow };
    world.entity_mut(candidate).remove::<Candidate>().insert((Leader(adventurer), hired, offers));
    obey(world, candidate, Order::Follow);
}

/// Makes `mercenary`, restored by a save as its kind on the adventurer's
/// side, the hired hand `hired` says it was: its name, the
/// [`Mercenary`] itself, its [`GIVE`] and [`DISMISS`] offers, and the
/// brain for its order by [`obey`], which keeps the engine's wit to
/// travel off it as hiring does.
///
/// None of that is saved but the [`Mercenary`]: a brain is content made
/// again, and the engine keeps nobody's offers. Its `Leader`, its place
/// in the party and the post it holds are the engine's, put back after
/// this, so the post [`obey`] gives one on hold here, where the restore
/// first stands it, is replaced by the one it held.
pub fn serve_again(world: &mut World, mercenary: Entity, hired: Mercenary) {
    let offers = hired_offers(world, &hired.name);
    let order = hired.order;
    world.entity_mut(mercenary).insert((Name::new(hired.name.clone()), hired, offers));
    obey(world, mercenary, order);
}

/// Takes the currency out of `who`'s pack and out of play.
fn spend_fee(world: &mut World, who: Entity) {
    let Some(currency) = world.get_resource::<Currency>().copied() else { return };
    let bag = world.get::<Inventory>(who).map(|b| b.items.clone()).unwrap_or_default();
    let coins: Vec<Entity> = bag.into_iter().filter(|i| currency.is(world.get::<Tagged>(*i))).collect();
    if let Some(mut bag) = world.get_mut::<Inventory>(who) {
        bag.items.retain(|i| !coins.contains(i));
    }
    for coin in coins {
        world.despawn(coin);
    }
}

/// Answers a candidate's [`HIRE`], already paid for: hires it by [`hire`]
/// and says it in the mercenaries' tone, `Brenna joins you.`
///
/// In `TurnSet::React`, beside the candidates' own talk. A full party is
/// never asked: [`keep_hire_offered`] has taken the offer off.
pub fn answer_hire(
    mut commands: Commands,
    mut done: MessageReader<Interacted>,
    verbs: Res<Verbs>,
    candidates: Query<&Name, With<Candidate>>,
    tones: Res<Tones>,
    mut tell: MessageWriter<Tell>,
) {
    let Some(hire_verb) = verbs.get(HIRE) else { return };
    for ev in done.read().filter(|ev| ev.verb == hire_verb) {
        let Ok(name) = candidates.get(ev.target) else { continue };
        tell.write(Tell::new(format!("{} joins you.", name.as_str()), tones.get(TONE).unwrap_or(Tones::TEXT)));
        let (candidate, adventurer) = (ev.target, ev.actor);
        commands.queue(move |world: &mut World| hire(world, candidate, adventurer));
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::allies::Ally;
    use crate::mercenaries::fixtures::{in_mereholt, offered, take, waiting};
    use crate::testing::{at, gold, monster, pack, pass_turns, said, set_gold};

    /// Whether `who` thinks with the brain for `order`.
    fn thinks(app: &App, who: Entity, order: Order) -> bool {
        let mind = app.world().get::<Mind>(who).expect("a mind");
        Arc::ptr_eq(&mind.0, &app.world().resource::<MercenaryBrains>().mind(order).0)
    }

    /// Taking up `hire` makes a candidate a mercenary over a span of
    /// seeds: no candidate any more, on the adventurer's side and an
    /// [`Ally`] the save keeps, following them by `Leader` under the follow
    /// order and its brain, named and priced as it was, offering `give`
    /// and `dismiss` in place of `hire` and talk, and said to have joined.
    /// The fee is spent, not carried: none of the gold is in its pack.
    #[test]
    fn hiring_makes_a_candidate_a_follower_with_leader_mercenary_give_and_dismiss_and_the_follow_order_over_a_span_of_seeds() {
        for seed in 0..4 {
            let (mut app, me) = in_mereholt(seed);
            let candidate = waiting(&mut app)[0];
            let fee = app.world().get::<Candidate>(candidate).expect("a candidate").fee;
            let name = app.world().get::<Name>(candidate).expect("a name").as_str().to_string();
            set_gold(&mut app, me, fee + 3);
            take(&mut app, me, candidate, HIRE);
            let world = app.world();
            assert!(world.get::<Candidate>(candidate).is_none(), "seed {seed}: no longer waiting");
            assert_eq!(world.get::<Leader>(candidate), Some(&Leader(me)), "seed {seed}");
            assert_eq!(world.get::<Mercenary>(candidate), Some(&Mercenary { name: name.clone(), fee, order: Order::Follow }), "seed {seed}");
            assert!(world.get::<Ally>(candidate).is_some(), "seed {seed}: an ally");
            assert_eq!(world.get::<Faction>(candidate).map(|f| f.0), world.get::<Faction>(me).map(|f| f.0), "seed {seed}: the adventurer's side");
            assert!(thinks(&app, candidate, Order::Follow), "seed {seed}: the follow brain");
            assert_eq!(offered(&app, candidate), [(GIVE.to_string(), None), (DISMISS.to_string(), None)], "seed {seed}");
            assert_eq!(gold(&app, me), 3, "seed {seed}: the fee paid");
            assert!(pack(&app, candidate).iter().all(|(thing, _)| !thing.contains("gold")), "seed {seed}: and spent: {:?}", pack(&app, candidate));
            assert!(said(&app).contains(&format!("{name} joins you.")), "seed {seed}: {:?}", said(&app));
        }
    }

    /// The party is counted in mercenaries: a freed captive follows the
    /// adventurer by the same `Leader`, and with one mercenary and a
    /// captive behind them the other candidates still offer `hire`; a
    /// second mercenary hired fills the party.
    #[test]
    fn the_party_counts_hired_mercenaries_and_never_a_freed_captive() {
        let (mut app, me) = in_mereholt(2);
        set_gold(&mut app, me, 10_000);
        let candidates = waiting(&mut app);
        take(&mut app, me, candidates[0], HIRE);
        let near = at(&app, me).offset(0, 1);
        let captive = monster(&mut app, "goblin", near);
        crate::captives::free(app.world_mut(), captive, me);
        pass_turns(&mut app, 1);
        assert_eq!(app.world().get::<Leader>(captive), Some(&Leader(me)), "the captive follows");
        let hires = |app: &App, who: Entity| offered(app, who).iter().any(|(v, _)| v == HIRE);
        assert!(hires(&app, candidates[1]) && hires(&app, candidates[2]), "one mercenary and a captive leave room");
        take(&mut app, me, candidates[1], HIRE);
        pass_turns(&mut app, 1);
        assert!(!hires(&app, candidates[2]), "two mercenaries fill it");
    }
}
