//! A probe's radar reports back to the rest of the deck. Spec section 8.5:
//! noticing an enemy is not a private fact for a droid carrying [`Alarm`],
//! it sounds a klaxon every droid on the deck can hear.
//!
//! A probe is an alarm and nothing else: it carries no weapon, and its
//! brain keeps the commando in sight at a distance rather than closing to
//! fight. The alarm is not instant. A probe that knows where the commando
//! is first signals, the engine's work, for as many of its own turns as
//! its definition says, standing where it is with its row reading
//! `signalling`: that is the commando's window, since a hit breaks the
//! signalling and it starts over, and a kill ends it. Only when it
//! finishes is the alarm [`Raised`]. From then, for every turn it takes
//! knowing where the commando is, it shouts again, so the deck keeps
//! homing in for as long as the probe hangs there, and a shout the player
//! can see is a pulse on the probe. The log says when it starts
//! signalling and, once, when the alarm sounds, since a line a turn would
//! bury everything else.
//!
//! The alarm is a noise of the game's own, carried by the engine's
//! hearing: loud enough to reach every open corner of a deck, stopped by
//! walls and muffled by a shut door the way any sound is. A droid that
//! hears it comes to where the probe sounded it, not to where the enemy
//! stands; what it finds there is the ordinary sight and notice roll, so
//! an alarm answered is not yet an enemy caught.
//!
//! Nothing about the alarm is a `DarkSight` matter: a line droid with no
//! radar at all still hears a probe's klaxon, since hearing is not seeing.

use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_bevy::{Thinking, WorkBegan, WorkDone, WorkKinds};
use rl_engine::rl_rules::ability::Look;
use rl_engine::rl_rules::ai::{Decision, Tactic, TacticCtx};
use rl_engine::rl_rules::work::{Work, WorkKindId};

/// The kind of work a probe does before its alarm sounds, by the word its
/// row shows.
pub const SIGNALLING: &str = "signalling";

/// The name the alarm's sound is declared under, with
/// [`AddSound::add_sound`](rl_engine::rl_bevy::AddSound) in
/// [`FoundryPlugin`](crate::plugin::FoundryPlugin).
pub const ALARM_SOUND: &str = "alarm";

/// How far the klaxon carries, in steps of open deck: past the far corner
/// of a 70 by 40 deck by any way round it, so spec section 8.5's "wakes the
/// deck" still holds wherever the probe stands.
pub const ALARM_LOUDNESS: i32 = 80;

/// What a shout looks like: a burst of red on the probe's own cell. A
/// burst is drawn in its look's colour alone, so the glyph is never shown
/// and says only what the pulse is.
pub const PULSE: Look = Look { glyph: '!', color: rl_engine::rl_grid::Rgb::new(255, 51, 38) };

// ANCHOR: noise_rules
/// How loud the engine's own actions are on a deck. A blow or a shot
/// carries ten steps, so a firefight draws the droids in earshot; steps,
/// doors and a thrown thing landing make no sound worth hearing over the
/// machinery, and a shut bulkhead takes three steps off anything that
/// passes it.
pub const NOISE: NoiseRules = NoiseRules { step: 0, strike: 10, door: 0, landing: 0, door_muffle: 3 };
// ANCHOR_END: noise_rules

/// Marks a monster that sounds the deck's alarm: once it knows where the
/// commando is it signals for `signal` of its own turns, and then shouts
/// for as long as it still knows, through [`shout_alarm`].
///
/// Its own component rather than a field on [`Notice`](rl_engine::rl_bevy::prelude::Notice),
/// since most of a deck's droids never carry it: only the probe's radar
/// reports back.
#[derive(Component, Debug, Clone, Copy)]
pub struct Alarm {
    /// The turns of signalling before the alarm sounds.
    pub signal: u16,
}

/// The alarm this probe signalled for is up: it shouts on every action
/// until it no longer knows where the commando is, and must signal again
/// the next time it finds out.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Raised;

/// What a probe that has not yet raised its alarm knows it should do
/// about an enemy it knows of: the kind of work, and how long it takes.
#[derive(Debug, Clone, Copy)]
pub struct Signal {
    /// The kind of work, so the tactic needs no resource.
    pub kind: WorkKindId,
    /// The turns it takes.
    pub turns: u16,
}

/// Tells a probe holding the turn that it has an alarm to raise: it knows
/// where the commando is and has not raised one yet.
pub fn sense_alarm(
    mut thinking: ResMut<Thinking>,
    kinds: Res<WorkKinds>,
    quiet: Query<(&Alarm, &Aware), Without<Raised>>,
    players: Query<Entity, With<Player>>,
) {
    let Some(actor) = thinking.actor() else { return };
    let (Ok((alarm, aware)), Some(kind)) = (quiet.get(actor), kinds.get(SIGNALLING)) else { return };
    if !players.iter().any(|p| aware.knows(p)) {
        return;
    }
    if let Some(snapshot) = thinking.snapshot_mut() {
        snapshot.add_sense(Signal { kind, turns: alarm.signal });
    }
}

// ANCHOR: signal
/// Raise the alarm: begin signalling, which the engine carries on turn
/// after turn until it is done or a hit breaks it.
pub struct SignalAlarm;

impl Tactic<Entity> for SignalAlarm {
    fn name(&self) -> &'static str {
        "signal_alarm"
    }

    fn evaluate(&self, ctx: &mut TacticCtx<'_, Entity>) -> Option<Decision<Entity>> {
        let signal = *ctx.snapshot.sense::<Signal>()?;
        Some(Decision::Work(Work::new(signal.kind, signal.turns)))
    }
}
// ANCHOR_END: signal

// ANCHOR: tell
/// Answers a probe's signalling: says in the log that it has started,
/// which is the commando's warning, and when it finishes raises the alarm
/// and says so, once, and not on every shout after.
pub fn sound_alarm(
    mut commands: Commands,
    mut began: MessageReader<WorkBegan>,
    mut done: MessageReader<WorkDone>,
    kinds: Res<WorkKinds>,
    alarmed: Query<(), With<Alarm>>,
    mut tell: MessageWriter<Tell>,
) {
    let Some(signalling) = kinds.get(SIGNALLING) else { return };
    for ev in began.read().filter(|ev| ev.kind == signalling && alarmed.contains(ev.actor)) {
        tell.write(Tell::new("{Who} starts signalling.", Tones::NOTICE).by(ev.actor));
    }
    for ev in done.read().filter(|ev| ev.kind == signalling && alarmed.contains(ev.actor)) {
        commands.entity(ev.actor).insert(Raised);
        tell.write(Tell::new("{Who} sounds an alarm.", Tones::BAD).by(ev.actor));
    }
}
// ANCHOR_END: tell

/// A probe whose alarm is up, where it stands and what it knows.
type Sounding<'w, 's> = Query<'w, 's, (&'static Position, &'static Aware), (With<Alarm>, With<Raised>)>;

// ANCHOR: shout
/// Lowers the alarm of a probe that no longer knows where the commando is,
/// so the next time it finds out it has to signal again.
pub fn lower_alarm(mut commands: Commands, raised: Query<(Entity, &Aware), With<Raised>>, players: Query<Entity, With<Player>>) {
    for (probe, aware) in &raised {
        if !players.iter().any(|p| aware.knows(p)) {
            commands.entity(probe).remove::<Raised>();
        }
    }
}

/// Shouts the alarm for every action a probe whose alarm is [`Raised`]
/// finishes while it knows where the player is: a [`MakeNoise`] of [`ALARM_SOUND`] where it
/// stands, as loud as [`ALARM_LOUDNESS`], and a [`PULSE`] on it when the
/// player can see it there. Whoever hears it comes to look; the engine's
/// hearing decides who that is.
///
/// On the probe's own actions rather than on the clock, so a probe frozen
/// on a deck the commando left says nothing, and one that notices and acts
/// in the same pass shouts in that pass. The pulse is a cue like any
/// other, so it holds the turns while it plays and a key skips it; one out
/// of sight would give the probe away and hold the turns for nothing to
/// see, so the noise goes out and the pulse does not.
pub fn shout_alarm(
    mut done: MessageReader<ActionDone>,
    alarmed: Sounding,
    players: Query<(Entity, &Viewshed), With<Player>>,
    sounds: Res<Sounds>,
    mut noise: MessageWriter<MakeNoise>,
    mut cues: MessageWriter<Cued>,
) {
    let alarm = sounds.get(ALARM_SOUND).expect("FoundryPlugin declares the alarm's sound");
    for ev in done.read() {
        let Ok((at, aware)) = alarmed.get(ev.actor) else { continue };
        if !players.iter().any(|(p, _)| aware.knows(p)) {
            continue;
        }
        noise.write(MakeNoise { at: at.0, loudness: ALARM_LOUDNESS, sound: alarm, maker: Some(ev.actor) });
        if !players.iter().any(|(_, sight)| sight.can_see(at.0)) {
            continue;
        }
        cues.write(Cued { actor: ev.actor, cue: Cue::Burst { on: vec![Anchor::on(ev.actor, at.0)], look: LookOf::Given(PULSE), from: None } });
    }
}
// ANCHOR_END: shout

#[cfg(test)]
mod tests {
    use rl_engine::rl_bevy::testing::KeyScriptPlugin;
    use rl_engine::rl_core::{Point, Rect, RunSeed};
    use rl_engine::rl_render::{MapViewPlugin, Particles, ParticlesPlugin, Terminal};

    use super::*;

    fn at(app: &App, e: Entity) -> Point {
        app.world().get::<Position>(e).expect("it stands somewhere").0
    }

    fn player(app: &mut App) -> Entity {
        app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap()
    }

    fn lines(app: &App) -> Vec<String> {
        app.world().resource::<MessageLog>().iter().map(|e| e.text.clone()).collect()
    }

    /// Where `listener` last heard something, if it still remembers.
    fn heard(app: &App, listener: Entity) -> Option<Point> {
        app.world().get::<Heard>(listener).and_then(|h| h.last_known())
    }

    /// Real damage, through combat, so a hit is a hit and a death a death.
    fn strike(app: &mut App, target: Entity, amount: i32) {
        let kinetic = app.world().resource::<Registries>().damage_kinds.expect("kinetic");
        app.world_mut().write_message(DamageEvent::new(target, rl_engine::rl_rules::Hit::from_source(None, kinetic, amount)));
    }

    fn shouts(app: &App, probe: Entity) -> usize {
        app.world().resource::<crate::testing::Alarms>().shouts.iter().filter(|(who, ..)| *who == probe).count()
    }

    /// The alarm is not instant: a probe that knows where the commando is
    /// signals first, for four of its own turns, which at its speed is a
    /// little over three of the commando's, standing where it is and saying
    /// so, and only then does the alarm sound.
    #[test]
    fn a_probe_signals_where_it_stands_before_the_alarm_sounds() {
        let mut app = crate::testing::headless(RunSeed(1));
        let (probe, me) = crate::testing::droid_facing_player(&mut app, "probe droid", 4);
        crate::testing::clear_droids(&mut app, &[probe]);
        crate::testing::record_alarms_from_now(&mut app);
        crate::testing::alert(&mut app, probe, me);
        let stood = at(&app, probe);
        crate::testing::pass_turns(&mut app, 2);
        assert!(app.world().get::<Working>(probe).is_some(), "two turns in, it is still signalling");
        assert_eq!(shouts(&app, probe), 0, "and the alarm has not sounded");
        assert_eq!(at(&app, probe), stood, "nor has it moved from where it began");
        assert!(lines(&app).iter().any(|l| l == "The probe droid starts signalling."), "{:#?}", lines(&app));
        assert!(!lines(&app).iter().any(|l| l == "The probe droid sounds an alarm."), "{:#?}", lines(&app));

        crate::testing::pass_turns(&mut app, 4);
        assert!(app.world().get::<Raised>(probe).is_some(), "its signalling done, the alarm is raised");
        assert!(shouts(&app, probe) > 0, "and it shouts");
        assert!(lines(&app).iter().any(|l| l == "The probe droid sounds an alarm."), "{:#?}", lines(&app));
    }

    /// The window the signalling opens: a probe hit before it finishes
    /// starts over, so the alarm that was a turn away is three away again,
    /// and one killed never sounds it.
    #[test]
    fn a_probe_hit_while_signalling_starts_over_and_one_killed_never_sounds_the_alarm() {
        let mut app = crate::testing::headless(RunSeed(1));
        let (probe, me) = crate::testing::droid_facing_player(&mut app, "probe droid", 4);
        crate::testing::clear_droids(&mut app, &[probe]);
        crate::testing::record_alarms_from_now(&mut app);
        crate::testing::alert(&mut app, probe, me);
        crate::testing::pass_turns(&mut app, 2);
        strike(&mut app, probe, 2);
        crate::testing::pass_turns(&mut app, 2);
        assert_eq!(shouts(&app, probe), 0, "four turns on, past where it would have sounded, and it has not");
        assert!(app.world().get::<Raised>(probe).is_none());

        strike(&mut app, probe, 1_000);
        crate::testing::pass_turns(&mut app, 6);
        assert_eq!(shouts(&app, probe), 0, "and killed, it never does");
        assert!(!lines(&app).iter().any(|l| l == "The probe droid sounds an alarm."), "{:#?}", lines(&app));
    }

    /// A probe that has not noticed anyone says nothing; one that knows
    /// where the commando is shouts the alarm on every turn it takes. The
    /// pulse is for the eye: a probe the player cannot see shouts without
    /// one, so an unseen probe neither gives itself away nor holds the
    /// turns.
    #[test]
    fn a_probe_out_of_sight_shouts_on_every_turn_it_knows_where_the_commando_is_without_a_pulse() {
        let mut app = crate::testing::headless(RunSeed(1));
        let (probe, _) = crate::testing::out_of_sight(&mut app, "probe droid", 3000, 6000);
        crate::testing::record_alarms_from_now(&mut app);
        crate::testing::pass_turns(&mut app, 2);
        assert!(app.world().resource::<crate::testing::Alarms>().shouts.is_empty(), "it knows nothing, so it says nothing");
        let me = player(&mut app);
        crate::testing::alert(&mut app, probe, me);
        // Its signalling first, then the alarm for as long as it remembers
        // where the commando was, which out of sight is a couple of turns.
        crate::testing::pass_turns(&mut app, 8);
        let alarms = app.world().resource::<crate::testing::Alarms>();
        let shouts: Vec<&(Entity, Point, bool)> = alarms.shouts.iter().filter(|(who, ..)| *who == probe).collect();
        assert!(shouts.len() >= 2, "the alarm raised, a shout on each turn of its own it still knows: {shouts:?}");
        assert!(shouts.iter().all(|(.., seen)| !seen), "and all of them out of sight: {shouts:?}");
        assert!(alarms.cues.iter().all(|c| c.actor != probe), "no pulse for a probe nobody sees: {:?}", alarms.cues);
    }

    /// A probe in plain view pulses on every shout, on itself where it
    /// shouted, in the alarm's red.
    #[test]
    fn a_probe_in_sight_pulses_on_itself_every_time_it_shouts() {
        let mut app = crate::testing::headless(RunSeed(1));
        let (probe, me) = crate::testing::droid_facing_player(&mut app, "probe droid", 4);
        crate::testing::record_alarms_from_now(&mut app);
        crate::testing::alert(&mut app, probe, me);
        // Its signalling first, then four turns of the alarm.
        crate::testing::pass_turns(&mut app, 8);
        let alarms = app.world().resource::<crate::testing::Alarms>();
        let shouts: Vec<Point> = alarms.shouts.iter().filter(|(who, _, seen)| *who == probe && *seen).map(|(_, at, _)| *at).collect();
        assert!(shouts.len() >= 4, "four turns of the alarm in plain view, a shout on each: {:?}", alarms.shouts);
        let pulses: Vec<&Cued> = alarms.cues.iter().filter(|c| c.actor == probe).collect();
        assert_eq!(pulses.len(), shouts.len(), "a pulse for every shout: {pulses:?}");
        for (cue, shouted) in pulses.iter().zip(&shouts) {
            let Cue::Burst { on, look: LookOf::Given(look), .. } = &cue.cue else { panic!("a pulse is a burst of its own look: {cue:?}") };
            assert_eq!(on.as_slice(), [Anchor::on(probe, *shouted)], "on the probe, where it shouted");
            assert_eq!(*look, PULSE);
        }
    }

    /// The alarm is a klaxon, not a broadcast: a droid far across the deck
    /// hears it where the probe sounded it, and a rat, being deaf to it,
    /// hears nothing.
    #[test]
    fn a_probe_that_knows_where_the_player_is_sounds_an_alarm_a_droid_across_the_deck_hears_and_a_rat_does_not() {
        let mut app = crate::testing::headless(RunSeed(1));
        let probe = crate::testing::lone_monster(&mut app, "probe droid");
        let (far, _) = crate::testing::out_of_sight(&mut app, "line droid", 1500, 4000);
        let (rat, _) = crate::testing::out_of_sight(&mut app, "coolant rat", 500, 1500);
        crate::testing::record_alarms_from_now(&mut app);
        let me = player(&mut app);
        crate::testing::alert(&mut app, probe, me);
        // Long enough to finish signalling and shout once.
        crate::testing::pass_turns(&mut app, 5);
        let last = app.world().resource::<crate::testing::Alarms>().shouts.last().copied();
        assert_eq!(last.map(|(who, ..)| who), Some(probe), "it shouted");
        assert_eq!(heard(&app, far), last.map(|(_, at, _)| at), "fifteen steps and more round the deck, it heard the alarm, and where");
        assert!(app.world().get::<Heard>(rat).is_none(), "a rat has no ear for it");
    }

    /// A droid that hears the alarm comes to where it sounded, round
    /// whatever walls are in the way.
    #[test]
    fn a_droid_that_hears_the_alarm_goes_to_where_it_sounded() {
        let mut app = crate::testing::headless(RunSeed(1));
        let probe = crate::testing::lone_monster(&mut app, "probe droid");
        let (far, post) = crate::testing::out_of_sight(&mut app, "line droid", 800, 1500);
        let sounded = at(&app, probe);
        let before = crate::testing::walk(&app, post, sounded);
        let me = player(&mut app);
        crate::testing::alert(&mut app, probe, me);
        crate::testing::pass_turns(&mut app, 5);
        let after = crate::testing::walk(&app, at(&app, far), sounded);
        assert!(after < before, "it went to look: {before} hundredths of a step from the alarm, then {after}");
    }

    /// The whole chain in play, with nothing written by hand: a probe with
    /// a clear line to the commando notices it, and the log says so and
    /// then says it sounds the alarm, once, however many turns it goes on
    /// shouting. It never strikes, having nothing to strike with.
    #[test]
    fn a_probe_that_spots_the_commando_in_play_is_logged_noticing_it_then_sounding_the_alarm_once_and_never_strikes() {
        let mut app = crate::testing::headless(RunSeed(1));
        let (probe, _) = crate::testing::droid_facing_player(&mut app, "probe droid", 4);
        assert!(app.world().get::<MeleeAttack>(probe).is_none() && app.world().get::<RangedAttack>(probe).is_none(), "a probe carries no attack");
        let brain = format!("{:?}", app.world().get::<Mind>(probe).expect("a probe has a mind").0);
        assert_eq!(brain, r#"["signal_alarm", "shadow", "hover", "search_last_known", "keep_post", "wander"]"#, "and no tactic that would strike");
        crate::testing::pass_turns(&mut app, 12);
        let lines = lines(&app);
        let noticed = lines.iter().position(|l| l == "The probe droid notices you.").unwrap_or_else(|| panic!("{lines:#?}"));
        let alarms: Vec<usize> = lines.iter().enumerate().filter(|(_, l)| *l == "The probe droid sounds an alarm.").map(|(i, _)| i).collect();
        assert_eq!(alarms.len(), 1, "one line for the alarm, however often it shouts: {lines:#?}");
        assert!(noticed < alarms[0], "noticing, then the alarm: {lines:#?}");
        assert!(!lines.iter().any(|l| l.contains("an alarm sounds")), "the deck's own line is gone: {lines:#?}");
    }

    /// The probe hangs back: closing from eight, it stops inside five, and
    /// right beside the commando it backs off to three.
    #[test]
    fn a_probe_keeps_between_three_and_five_tiles_from_the_commando_it_has_spotted() {
        for (from, why) in [(8, "closing from eight"), (1, "backing off from one")] {
            let mut app = crate::testing::headless(RunSeed(1));
            let (probe, me) = crate::testing::droid_down_a_lane(&mut app, "probe droid", from, 8);
            crate::testing::alert(&mut app, probe, me);
            crate::testing::pass_turns(&mut app, 6);
            let gap = rl_engine::rl_core::geometry::chebyshev(at(&app, probe), at(&app, me));
            assert!((3..=5).contains(&gap), "{why}, it came to rest {gap} off");
        }
    }

    /// The player's gap to the probe after each of `turns` waits, with
    /// every other mind on the deck gone, so nothing but the probe moves.
    fn gaps_while_the_player_waits(seed: u64, start: i32, turns: usize) -> Vec<i32> {
        let mut app = crate::testing::headless(RunSeed(seed));
        let (probe, me) = crate::testing::droid_facing_player(&mut app, "probe droid", start);
        let others: Vec<Entity> = app.world_mut().query_filtered::<Entity, With<Mind>>().iter(app.world()).filter(|e| *e != probe).collect();
        for e in others {
            app.world_mut().despawn(e);
        }
        crate::testing::alert(&mut app, probe, me);
        (0..turns)
            .map(|_| {
                crate::testing::pass_turns(&mut app, 1);
                rl_engine::rl_core::geometry::chebyshev(at(&app, probe), at(&app, me))
            })
            .collect()
    }

    /// A probe that spots a player who then stands still signals where it
    /// is, then backs off and stays off: within six turns it is three away, and it is never
    /// nearer again, even where backing off takes it round a corner out of sight,
    /// since it keeps its distance from where it last saw the player too.
    #[test]
    fn a_probe_beside_an_idle_player_backs_off_and_never_comes_nearer_than_three_again_over_a_range_of_seeds() {
        for seed in [2, 1, 3, 4, 5, 6] {
            let gaps = gaps_while_the_player_waits(seed, 1, 20);
            let off = gaps.iter().position(|g| *g >= 3).unwrap_or_else(|| panic!("seed {seed}: never backed off, gaps per turn {gaps:?}"));
            assert!(off <= 6, "seed {seed}: backed off only by turn {off}, gaps per turn {gaps:?}");
            assert!(gaps[off..].iter().all(|g| *g >= 3), "seed {seed}: came back in, gaps per turn {gaps:?}");
        }
    }

    /// A pulse holds the turns like any cue, and a key pressed while it
    /// plays skips it and hands the commando its turn in the same frame.
    /// A key Foundry binds to nothing, so what is seen is the skip alone
    /// and not the turn the key would then spend.
    #[test]
    fn a_key_pressed_while_a_probe_pulses_skips_the_pulse_and_hands_the_commando_its_turn() {
        let mut app = crate::testing::headless(RunSeed(1));
        app.add_plugins((MapViewPlugin::new(Rect::new(0, 0, 70, 36)), ParticlesPlugin, KeyScriptPlugin));
        app.insert_resource(Terminal::new(100, 40, Vec2::ONE));
        let (probe, me) = crate::testing::droid_facing_player(&mut app, "probe droid", 4);
        crate::testing::alert(&mut app, probe, me);
        crate::testing::record_alarms_from_now(&mut app);
        let mut held = false;
        for _ in 0..4 {
            if app.world().get::<MyTurn>(me).is_some() {
                app.world_mut().write_message(Intent::new(me, Wait));
            }
            app.update();
            if app.world().resource::<TurnHold>().is_held() {
                held = true;
                break;
            }
        }
        assert!(held && app.world().resource::<Particles>().is_holding(), "the pulse holds the turns");
        let last = app.world().resource::<crate::testing::Alarms>().cues.last().cloned();
        assert!(matches!(last, Some(Cued { actor, cue: Cue::Burst { look: LookOf::Given(PULSE), .. } }) if actor == probe), "on the probe's pulse: {last:?}");
        app.world_mut().resource_mut::<rl_engine::rl_bevy::testing::KeyScript>().press(KeyCode::F12);
        app.update();
        assert!(!app.world().resource::<TurnHold>().is_held(), "the key let the turns go");
        assert!(app.world().get::<MyTurn>(me).is_some(), "and the commando holds its turn");
    }
}
