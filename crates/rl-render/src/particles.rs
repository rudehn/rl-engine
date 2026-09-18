//! Particles: what flies, bursts and fades over the map for a moment.
//!
//! The turn loop is instant, so a fireball is cast, lands and burns in one
//! frame, and a player would see a crab die of nothing. This layer plays
//! what happened back over the following frames, from the [`Cued`]
//! messages the resolvers write: a glyph flying from one [`Anchor`] to
//! another, then a burst over the anchors it covered, fading. Anchors are
//! read again every frame, so a flight at someone who walks on lands on
//! them and a pulse on someone who steps aside goes with them.
//!
//! While something that holds the turns plays, the turns wait: the plugin
//! watches the [`TurnHold`] and lets go when the last of it has played, so
//! the eel is seen to spit before the crab moves, and two casters are two
//! flights one after the other. A key pressed while the turns wait is a
//! player who has seen enough: everything that holds them is dropped, the
//! turns run on to the player's own, and the key is read with that turn
//! in hand, so nothing a quick player presses is lost to a fade. Once the
//! player holds a turn nothing holds the turns, since there is nothing
//! left to wait for; what still plays, plays over the player's move.
//!
//! Two shapes cover what a roguelike animates: a [`Trail`] between two
//! anchors and a [`Burst`] over some. An [`Animation`] is a list of them
//! played one after another, one per actor per frame, in the order the
//! actor's cues were written. A game with a look of its own writes to
//! [`Particles`] itself.
//!
//! Everything is drawn from the wall clock, so a frame that comes late
//! shows a later moment rather than a slower one, and a still frame costs
//! no more than the cells it paints.

use bevy::prelude::*;
use rl_bevy::prelude::*;
use rl_bevy::{Anchor, Cue, Cued, LookOf, TurnHold};
use rl_core::Point;
use rl_core::geometry::line;
use rl_core::seed::position_hash;

use crate::map_view::{Glyph, MapView, draw_map};
use crate::terminal::Terminal;

/// Where an anchor is this frame: the entity's cell if it follows one that
/// is still somewhere, else the cell it was written with.
pub type Resolve<'a> = dyn Fn(&Anchor) -> Point + 'a;

/// A glyph flying from one anchor to the other, a cell at a time, with a
/// short fading tail behind it.
///
/// The line is drawn again every frame between where the anchors are
/// now, and the head's place along it is the share of the flight's time
/// that has passed, so a target that moves bends the flight rather than
/// leaving it in the air. The cell it leaves from is not part of it: what
/// stands there is what threw it, and stays drawn.
#[derive(Debug, Clone, PartialEq)]
pub struct Trail {
    /// Where it leaves from.
    pub from: Anchor,
    /// Where it lands.
    pub to: Anchor,
    /// What flies.
    pub glyph: char,
    /// Its colour at the head.
    pub color: Color,
    /// Seconds spent on each cell of the line as it was when written.
    pub cell_secs: f32,
    /// Cells in the line as it was when written; what the time is set by.
    pub cells: usize,
}

impl Trail {
    /// A flight from `from` to `to` at `cell_secs` a cell, timed by the
    /// line between them as they stand now.
    pub fn new(from: Anchor, to: Anchor, glyph: char, color: Color, cell_secs: f32) -> Self {
        Self { from, to, glyph, color, cell_secs, cells: line(from.at, to.at).skip(1).count() }
    }
}

/// Every anchor lit at once and fading out, each showing one of a few
/// glyphs picked by position, so a burst reads as embers rather than a
/// stamp.
#[derive(Debug, Clone, PartialEq)]
pub struct Burst {
    /// Where.
    pub on: Vec<Anchor>,
    /// The glyphs the cells pick from.
    pub glyphs: Vec<char>,
    /// The colour at the start, fading to the map's own.
    pub color: Color,
    /// How long it lasts.
    pub secs: f32,
}

/// One beat of an animation: what plays before the next begins.
#[derive(Debug, Clone, PartialEq)]
pub enum Beat {
    /// A flight.
    Trail(Trail),
    /// A burst.
    Burst(Burst),
}

impl Beat {
    /// How long it plays.
    pub fn duration(&self) -> f32 {
        match self {
            Beat::Trail(t) => t.cells as f32 * t.cell_secs,
            Beat::Burst(b) => b.secs,
        }
    }

    /// What it shows `t` seconds in, with the anchors where `at` puts
    /// them: cells, glyphs and colours, with the fraction each glyph has
    /// faded, 0 fresh and 1 gone.
    pub fn frame(&self, t: f32, at: &Resolve<'_>) -> Vec<Spark> {
        match self {
            Beat::Trail(trail) => {
                let duration = self.duration();
                if duration <= 0.0 || t >= duration {
                    return Vec::new();
                }
                let path: Vec<Point> = line(at(&trail.from), at(&trail.to)).skip(1).collect();
                let head = ((t / duration) * path.len() as f32).floor() as usize;
                let Some(cell) = path.get(head) else { return Vec::new() };
                let mut sparks = vec![Spark { at: *cell, glyph: trail.glyph, color: trail.color, faded: 0.0 }];
                // Three cells of tail, each older and fainter.
                for back in 1..=3 {
                    if let Some(cell) = head.checked_sub(back).and_then(|i| path.get(i)) {
                        sparks.push(Spark { at: *cell, glyph: '.', color: trail.color, faded: back as f32 / 4.0 });
                    }
                }
                sparks
            }
            Beat::Burst(burst) => {
                if burst.secs <= 0.0 || t >= burst.secs {
                    return Vec::new();
                }
                let faded = t / burst.secs;
                burst
                    .on
                    .iter()
                    .map(|anchor| {
                        let cell = at(anchor);
                        let pick = position_hash(0, cell.x, cell.y) as usize % burst.glyphs.len().max(1);
                        Spark { at: cell, glyph: burst.glyphs.get(pick).copied().unwrap_or('*'), color: burst.color, faded }
                    })
                    .collect()
            }
        }
    }
}

/// One glyph on one cell for one frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spark {
    /// Where.
    pub at: Point,
    /// What.
    pub glyph: char,
    /// Its colour when fresh.
    pub color: Color,
    /// How far it has faded towards nothing, 0 to 1.
    pub faded: f32,
}

/// Steps played one after another on one map.
#[derive(Debug, Clone, PartialEq)]
pub struct Animation {
    /// The map it plays on. Left behind when the player leaves it.
    pub map: MapId,
    /// The steps, in order.
    pub steps: Vec<Beat>,
    /// Whether the turns wait for it.
    pub holds: bool,
}

impl Animation {
    /// How long the whole thing plays.
    pub fn duration(&self) -> f32 {
        self.steps.iter().map(Beat::duration).sum()
    }

    /// What shows `t` seconds in, with the anchors where `at` puts them.
    pub fn frame(&self, t: f32, at: &Resolve<'_>) -> Vec<Spark> {
        let mut begun = 0.0;
        for step in &self.steps {
            let ends = begun + step.duration();
            if t < ends {
                return step.frame(t - begun, at);
            }
            begun = ends;
        }
        Vec::new()
    }
}

/// The animations playing, and when each began.
#[derive(Resource, Debug, Default)]
pub struct Particles {
    playing: Vec<(Animation, Option<f32>)>,
}

impl Particles {
    /// Plays `animation` from the next frame drawn. One that would take no
    /// time is not played, and holds nothing.
    pub fn play(&mut self, animation: Animation) {
        if animation.duration() > 0.0 {
            self.playing.push((animation, None));
        }
    }

    /// Whether anything is playing.
    pub fn is_playing(&self) -> bool {
        !self.playing.is_empty()
    }

    /// Whether anything playing holds the turns.
    pub fn is_holding(&self) -> bool {
        self.playing.iter().any(|(a, _)| a.holds)
    }

    /// Drops whatever holds the turns, leaving the rest to play out.
    pub fn skip_held(&mut self) {
        self.playing.retain(|(a, _)| !a.holds);
    }

    /// What every animation shows at `now` with the anchors where `at`
    /// puts them, starting the ones that have not begun and dropping the
    /// ones that have finished.
    pub fn frame(&mut self, now: f32, map: MapId, at: &Resolve<'_>) -> Vec<Spark> {
        self.playing.retain(|(animation, _)| animation.map == map);
        let mut sparks = Vec::new();
        for (animation, began) in &mut self.playing {
            let began = *began.get_or_insert(now);
            sparks.extend(animation.frame(now - began, at));
        }
        self.playing.retain(|(animation, began)| began.is_none_or(|b| now - b < animation.duration()));
        sparks
    }
}

/// How the engine's own animations look: the flight and burst of anything
/// with no look of its own, and the pace of everything.
#[derive(Resource, Debug, Clone)]
pub struct ParticleStyle {
    /// The glyph and colour of a cue with no look.
    pub plain: (char, Color),
    /// Seconds a flight spends on each cell.
    pub cell_secs: f32,
    /// Seconds a burst takes to fade.
    pub burst_secs: f32,
    /// What a burst's cells show.
    pub burst_glyphs: Vec<char>,
}

impl Default for ParticleStyle {
    fn default() -> Self {
        Self { plain: ('*', Color::srgb(0.9, 0.9, 0.8)), cell_secs: 0.035, burst_secs: 0.4, burst_glyphs: vec!['*', '+', '\u{00b7}', 'x'] }
    }
}

impl ParticleStyle {
    /// A style that takes no time, so nothing plays and the turns are
    /// never held: what a headless test wants, where even a frame's hold
    /// would put the player's turn past the frame a key is read in.
    pub fn instant() -> Self {
        Self { cell_secs: 0.0, burst_secs: 0.0, ..Self::default() }
    }

    /// Whether anything drawn in this style takes time to watch.
    pub fn takes_time(&self) -> bool {
        self.cell_secs > 0.0 || self.burst_secs > 0.0
    }
}

/// Plays every cue the turns write, holds the turns while what they cued
/// plays, and draws whatever is playing over the map.
///
/// Draws after the map and before the overlays, so a burst shows through
/// a targeting cursor and under a menu.
pub struct ParticlesPlugin;

impl Plugin for ParticlesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Particles>().init_resource::<ParticleStyle>().init_resource::<TurnHold>();
        app.add_message::<Cued>().add_message::<MapChanged>();
        app.add_systems(rl_bevy::EndRun, |mut particles: ResMut<Particles>| *particles = Particles::default())
            .add_systems(OnEnter(EngineState::Playing), watch_unless_instant)
            .add_systems(Update, skip_on_key.before(EngineSet::Input).run_if(in_state(EngineState::Playing)))
            .add_systems(Update, play_cues.in_set(PresentSet::Narrate))
            .add_systems(Update, (draw_particles, hold_turns).chain().in_set(PresentSet::Map).after(draw_map));
    }

    fn finish(&self, app: &mut App) {
        rl_bevy::depends_on::<crate::map_view::MapViewPlugin>(app, "ParticlesPlugin");
    }
}

/// Takes the turns' hold, unless the style plays nothing worth waiting
/// for. Decided when play begins, so a game's style, inserted after the
/// plugin, is the one read.
pub fn watch_unless_instant(style: Res<ParticleStyle>, mut hold: ResMut<TurnHold>) {
    if style.takes_time() {
        hold.watch();
    }
}

/// What the plugin reads to turn a cue into a beat.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Cues<'w, 's> {
    cued: MessageReader<'w, 's, Cued>,
    abilities: Option<Res<'w, Abilities>>,
    map: Res<'w, WorldMap>,
    style: Res<'w, ParticleStyle>,
    glyphs: Query<'w, 's, &'static Glyph>,
}

impl Cues<'_, '_> {
    /// The glyph and colour a look names.
    fn look(&self, look: LookOf) -> (char, Color) {
        let given = |l: rl_rules::ability::Look| (l.glyph, Color::srgb_u8(l.color.r, l.color.g, l.color.b));
        match look {
            LookOf::Ability(id) => self.abilities.as_deref().and_then(|a| a.get(id).look).map(given).unwrap_or(self.style.plain),
            LookOf::Item(item) => self.glyphs.get(item).map(|g| (g.ch, g.fg)).unwrap_or(self.style.plain),
            LookOf::Given(l) => given(l),
            LookOf::Plain => self.style.plain,
        }
    }

    /// The beat a cue plays as.
    fn beat(&self, cue: &Cue) -> Beat {
        match cue {
            Cue::Flight { from, to, look } => {
                let (glyph, color) = self.look(*look);
                Beat::Trail(Trail::new(*from, *to, glyph, color, self.style.cell_secs))
            }
            Cue::Burst { on, look } => {
                let (_, color) = self.look(*look);
                Beat::Burst(Burst { on: on.clone(), glyphs: self.style.burst_glyphs.clone(), color, secs: self.style.burst_secs })
            }
        }
    }
}

/// Queues one animation per actor that cued something this frame, its
/// cues in the order written, each holding the turns.
pub fn play_cues(mut particles: ResMut<Particles>, mut cues: Cues) {
    let here = cues.map.current();
    let mut per_actor: Vec<(Entity, Animation)> = Vec::new();
    let cued: Vec<Cued> = cues.cued.read().cloned().collect();
    for Cued { actor, cue } in &cued {
        let beat = cues.beat(cue);
        match per_actor.iter_mut().find(|(a, _)| a == actor) {
            Some((_, animation)) => animation.steps.push(beat),
            None => per_actor.push((*actor, Animation { map: here, steps: vec![beat], holds: true })),
        }
    }
    for (_, animation) in per_actor {
        particles.play(animation);
    }
}

/// Paints whatever is playing over the map, keeping each cell's
/// background and fading the glyph towards it, with every anchor where
/// what it follows stands now.
pub fn draw_particles(
    mut terminal: ResMut<Terminal>,
    mut particles: ResMut<Particles>,
    view: Res<MapView>,
    map: Res<WorldMap>,
    time: Res<Time>,
    positions: Query<&Position>,
) {
    if !particles.is_playing() {
        return;
    }
    let at = |anchor: &Anchor| anchor.follow.and_then(|e| positions.get(e).ok()).map(|p| p.0).unwrap_or(anchor.at);
    for spark in particles.frame(time.elapsed_secs(), map.current(), &at) {
        let Some(screen) = view.to_screen(spark.at) else { continue };
        let Some(mut cell) = terminal.get(screen.x, screen.y) else { continue };
        cell.glyph = spark.glyph;
        cell.fg = bevy::color::Mix::mix(&spark.color, &cell.bg, spark.faded.clamp(0.0, 1.0));
        terminal.set(screen.x, screen.y, cell);
    }
}

/// A key pressed while the turns wait skips what holds them and runs the
/// turns on at once, before the game reads the key, so the key finds the
/// player holding the turn it was pressed for rather than being dropped
/// on a frame nobody held one.
pub fn skip_on_key(world: &mut World) {
    if !world.resource::<TurnHold>().is_held() {
        return;
    }
    let pressed = world.get_resource::<ButtonInput<KeyCode>>().is_some_and(|keys| keys.get_just_pressed().next().is_some());
    if !pressed {
        return;
    }
    // Through every hold in the way: a flight skipped lands, and its
    // burst would hold again, and a second caster's flight after that.
    // Bounded, since the turn loop's own bound is per call.
    for _ in 0..MAX_SKIPS {
        world.resource_mut::<Particles>().skip_held();
        world.resource_mut::<TurnHold>().release();
        rl_bevy::plugin::run_turns(world);
        if !world.resource::<TurnHold>().is_held() {
            return;
        }
    }
}

/// Holds the loop skips through at most on one key.
const MAX_SKIPS: usize = 64;

/// Keeps the turns held exactly while something that holds them plays and
/// there is a turn to wait for, after the frame that would have dropped
/// what finished. With the player holding a turn nothing is waited for:
/// the loop would not run anyway, and holding it would only keep the
/// player's next key from being read.
pub fn hold_turns(particles: Res<Particles>, mut hold: ResMut<TurnHold>, player: Query<(), (With<Player>, With<MyTurn>)>) {
    if particles.is_holding() && player.is_empty() {
        hold.hold();
    } else {
        hold.release();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed(anchor: &Anchor) -> Point {
        anchor.at
    }

    fn trail() -> Trail {
        Trail::new(Anchor::cell(Point::new(0, 0)), Anchor::cell(Point::new(4, 0)), '*', Color::WHITE, 0.1)
    }

    #[test]
    fn a_trail_moves_its_head_one_cell_per_cell_time_with_a_fading_tail() {
        let step = Beat::Trail(trail());
        assert_eq!(step.duration(), 0.4);
        let first = step.frame(0.0, &fixed);
        assert_eq!((first[0].at, first[0].glyph, first.len()), (Point::new(1, 0), '*', 1), "one cell out, and no tail yet");
        let third = step.frame(0.25, &fixed);
        assert_eq!(third[0].at, Point::new(3, 0), "the third cell, a quarter second in");
        assert_eq!(third.len(), 3, "the head and two cells of tail");
        assert!(third[1].faded < third[2].faded, "the older, the fainter");
        assert!(step.frame(0.4, &fixed).is_empty(), "flown");
    }

    /// A flight at someone who moves bends to where they are: the head
    /// is always on the line to them now, and the last moment is on them.
    #[test]
    fn a_trail_follows_what_its_anchor_follows() {
        let them = Entity::from_bits(7);
        let step = Beat::Trail(Trail::new(Anchor::cell(Point::new(0, 0)), Anchor::on(them, Point::new(4, 0)), '*', Color::WHITE, 0.1));
        assert_eq!(step.duration(), 0.4, "timed by the line as it was, the cell it leaves from left out");
        let moved = |a: &Anchor| if a.follow == Some(them) { Point::new(4, 2) } else { a.at };
        let midway = step.frame(0.25, &moved);
        let bent: Vec<Point> = line(Point::new(0, 0), Point::new(4, 2)).skip(1).collect();
        assert_eq!(midway[0].at, bent[2], "on the line to where they are now");
        assert_eq!(step.frame(0.39, &moved)[0].at, Point::new(4, 2), "and lands on them");
        assert_eq!(step.frame(0.39, &fixed)[0].at, Point::new(4, 0), "or where they were, if they are gone");
    }

    #[test]
    fn a_burst_lights_every_cell_at_once_and_fades_to_nothing() {
        let on = vec![Anchor::cell(Point::new(0, 0)), Anchor::cell(Point::new(1, 0)), Anchor::cell(Point::new(0, 1))];
        let step = Beat::Burst(Burst { on, glyphs: vec!['*', '+'], color: Color::WHITE, secs: 0.5 });
        let fresh = step.frame(0.0, &fixed);
        assert_eq!(fresh.len(), 3);
        assert!(fresh.iter().all(|s| s.faded == 0.0));
        assert!(fresh.iter().all(|s| s.glyph == '*' || s.glyph == '+'));
        assert_eq!(step.frame(0.25, &fixed)[0].faded, 0.5);
        assert!(step.frame(0.5, &fixed).is_empty(), "gone");
    }

    /// A pulse on someone who steps aside steps aside with them.
    #[test]
    fn a_burst_goes_where_what_it_is_on_goes() {
        let me = Entity::from_bits(3);
        let step = Beat::Burst(Burst { on: vec![Anchor::on(me, Point::new(2, 2))], glyphs: vec!['*'], color: Color::WHITE, secs: 0.5 });
        let stepped = |a: &Anchor| if a.follow == Some(me) { Point::new(3, 2) } else { a.at };
        assert_eq!(step.frame(0.1, &stepped)[0].at, Point::new(3, 2));
    }

    #[test]
    fn steps_play_one_after_another_and_a_finished_animation_is_dropped() {
        let burst = Burst { on: vec![Anchor::cell(Point::new(4, 0))], glyphs: vec!['*'], color: Color::WHITE, secs: 0.2 };
        let animation = Animation { map: MapId::SURFACE, steps: vec![Beat::Trail(trail()), Beat::Burst(burst)], holds: true };
        assert_eq!(animation.duration(), 0.6);
        assert_eq!(animation.frame(0.35, &fixed)[0].glyph, '*');
        assert_eq!(animation.frame(0.45, &fixed).len(), 1, "the burst, once the trail has flown");
        assert!(animation.frame(0.7, &fixed).is_empty());

        let mut particles = Particles::default();
        particles.play(animation);
        assert!(particles.is_playing() && particles.is_holding());
        assert_eq!(particles.frame(10.0, MapId::SURFACE, &fixed).len(), 1, "began on the first frame drawn");
        assert_eq!(particles.frame(10.45, MapId::SURFACE, &fixed).len(), 1, "the burst");
        assert!(particles.frame(10.7, MapId::SURFACE, &fixed).is_empty());
        assert!(!particles.is_playing() && !particles.is_holding(), "and it is gone, and holds nothing");
    }

    #[test]
    fn an_animation_is_left_behind_on_the_map_it_played_on() {
        let mut particles = Particles::default();
        particles.play(Animation { map: MapId(3), steps: vec![Beat::Trail(trail())], holds: true });
        assert!(particles.frame(0.0, MapId(4), &fixed).is_empty());
        assert!(!particles.is_playing(), "dropped with the map");
    }

    /// Through the plugin: one actor's cues in one frame are one animation
    /// in the order written, holding the turns, one at the player as much
    /// as any; and the hold lets go when the last of it has played.
    #[test]
    fn cues_become_one_animation_per_actor_that_holds_the_turns_until_it_has_played() {
        use rl_core::Rect;
        let mut app = rl_bevy::plugin::headless_app();
        app.add_plugins((crate::map_view::MapViewPlugin::new(Rect::new(0, 0, 40, 20)), ParticlesPlugin));
        app.insert_resource(Terminal::new(40, 20, Vec2::ONE));
        app.insert_resource(ParticleStyle { cell_secs: 0.01, burst_secs: 0.05, ..ParticleStyle::default() });
        let start = rl_bevy::testing::surface(&mut app);
        let me = app.world_mut().spawn((Actor, Player, Position(start), Viewshed::new(8), RevealsMap)).id();
        let (eel, crab) =
            (app.world_mut().spawn((Actor, Position(start.offset(3, 0)))).id(), app.world_mut().spawn((Actor, Position(start.offset(-3, 0)))).id());
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        assert!(app.world().resource::<TurnHold>().is_watched(), "the plugin watches");

        let look = LookOf::Plain;
        app.world_mut().write_message(Cued {
            actor: eel,
            cue: Cue::Flight { from: Anchor::on(eel, start.offset(3, 0)), to: Anchor::on(crab, start.offset(-3, 0)), look },
        });
        app.world_mut().write_message(Cued { actor: eel, cue: Cue::Burst { on: vec![Anchor::on(crab, start.offset(-3, 0))], look } });
        app.world_mut().write_message(Cued { actor: crab, cue: Cue::Flight { from: Anchor::on(crab, start.offset(-3, 0)), to: Anchor::on(me, start), look } });
        app.update();
        let particles = app.world().resource::<Particles>();
        assert_eq!(particles.playing.len(), 2, "one per actor");
        let (eels, crabs) = (&particles.playing[0].0, &particles.playing[1].0);
        assert!(matches!(eels.steps.as_slice(), [Beat::Trail(_), Beat::Burst(_)]) && eels.holds, "the flight, then the burst, and the turns wait");
        assert!(matches!(crabs.steps.as_slice(), [Beat::Trail(_)]) && crabs.holds, "at the player, and seen to arrive like any other");
        assert!(app.world().resource::<TurnHold>().is_held());

        let began = app.world().resource::<Time>().elapsed_secs();
        while app.world().resource::<Time>().elapsed_secs() - began < 0.2 {
            app.update();
        }
        assert!(!app.world().resource::<Particles>().is_playing(), "played out");
        assert!(!app.world().resource::<TurnHold>().is_held(), "and let go");
    }

    /// A key pressed while the turns wait drops what holds them and lets
    /// the turns go, in the frame the key is read.
    #[test]
    fn a_key_pressed_while_the_turns_wait_skips_what_holds_them() {
        use rl_core::Rect;
        let mut app = rl_bevy::plugin::headless_app();
        app.add_plugins((crate::map_view::MapViewPlugin::new(Rect::new(0, 0, 40, 20)), ParticlesPlugin, rl_bevy::testing::KeyScriptPlugin));
        app.insert_resource(Terminal::new(40, 20, Vec2::ONE));
        app.insert_resource(ParticleStyle { cell_secs: 1.0, burst_secs: 1.0, ..ParticleStyle::default() });
        let start = rl_bevy::testing::surface(&mut app);
        app.world_mut().spawn((Actor, Player, Position(start), Viewshed::new(8), RevealsMap));
        let eel = app.world_mut().spawn((Actor, Position(start.offset(3, 0)))).id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.world_mut().write_message(Cued { actor: eel, cue: Cue::Burst { on: vec![Anchor::on(eel, start.offset(3, 0))], look: LookOf::Plain } });
        app.update();
        assert!(app.world().resource::<TurnHold>().is_held() && app.world().resource::<Particles>().is_holding());
        app.world_mut().resource_mut::<rl_bevy::testing::KeyScript>().press(KeyCode::KeyH);
        app.update();
        assert!(!app.world().resource::<Particles>().is_holding(), "dropped");
        assert!(!app.world().resource::<TurnHold>().is_held(), "and let go, without waiting for the frame's end");
    }

    /// The player cancels an animation and takes the hit: a shot fired at
    /// the player is in the air, the turns wait on its flight, and a key
    /// skips it through the real skip path. The key finds the player
    /// holding its turn with the shot already landed, so nothing a quick
    /// player presses is read against a health bar the shot has not yet
    /// reached.
    #[test]
    fn a_key_pressed_while_a_shot_at_the_player_flies_lands_it_before_the_player_moves() {
        use rl_core::{DiceRoll, Rect};
        use rl_rules::ai::{Brain, tactics::ShootAtRange};
        let mut app = rl_bevy::plugin::headless_app();
        app.add_plugins((crate::map_view::MapViewPlugin::new(Rect::new(0, 0, 40, 20)), ParticlesPlugin, rl_bevy::testing::KeyScriptPlugin));
        app.add_plugins((FovPlugin, CombatPlugin, MindsPlugin, rl_bevy::world::StreamingPlugin));
        app.insert_resource(Terminal::new(40, 20, Vec2::ONE));
        app.insert_resource(ParticleStyle { cell_secs: 1.0, burst_secs: 1.0, ..ParticleStyle::default() });
        let start = rl_bevy::testing::surface(&mut app);
        let sides = rl_bevy::testing::two_sides(&mut app);
        let me = app.world_mut().spawn((Actor, Player, Blocks, Position(start), Viewshed::new(8), RevealsMap, Health::full(30), Faction(sides.ours))).id();
        let look = rl_rules::ability::Look { glyph: '*', color: rl_grid::Rgb::new(255, 80, 40) };
        app.world_mut().spawn((
            Actor,
            Blocks,
            Position(start.offset(4, 0)),
            Health::full(10),
            Faction(sides.theirs),
            Perception(8),
            RangedAttack::new(sides.kind, DiceRoll::flat(3), 6).looking(look),
            Mind(std::sync::Arc::new(Brain::new().then(ShootAtRange::default()))),
        ));
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        assert!(app.world().get::<MyTurn>(me).is_some(), "the player goes first");

        app.world_mut().write_message(Intent::new(me, Wait));
        app.update();
        app.update();
        let hp = |app: &App| app.world().get::<Health>(me).unwrap().current;
        assert!(app.world().resource::<TurnHold>().in_flight(), "the shot is in the air");
        assert!(app.world().resource::<TurnHold>().is_held() && app.world().resource::<Particles>().is_holding(), "and the turns wait on it");
        assert_eq!(hp(&app), 30, "it has not arrived");
        assert!(app.world().get::<MyTurn>(me).is_none());

        app.world_mut().resource_mut::<rl_bevy::testing::KeyScript>().press(KeyCode::KeyH);
        app.update();
        assert_eq!(hp(&app), 27, "the key landed the shot");
        assert!(!app.world().resource::<TurnHold>().in_flight());
        assert!(app.world().get::<MyTurn>(me).is_some(), "and the player holds its turn, hurt, in the frame the key was read");
    }
}
