//! Particles: what flies, bursts and fades over the map for a moment.
//!
//! The turn loop is instant, so a fireball is cast, lands and burns in one
//! frame, and a player sees a crab die of nothing. This layer plays what
//! happened back over the following frames: a glyph flying the path the
//! projectile took, then a burst over the cells it covered, fading. It is
//! cosmetic and never waits for: the world has already moved on, and a
//! player who acts while a burst fades gets the next turn at once, with
//! its own burst queued behind.
//!
//! Two shapes cover what a roguelike animates: a [`Trail`] along a path
//! and a [`Burst`] over cells. An [`Animation`] is a list of them played
//! one after another. [`ParticlesPlugin`] plays one for every ability
//! used and every item thrown, in the ability's own [`Look`](rl_rules::ability::Look) or the item's
//! glyph; a game with a look of its own writes to [`Particles`] itself.
//!
//! Everything is drawn from the wall clock, so a frame that comes late
//! shows a later moment rather than a slower one, and a still frame costs
//! no more than the cells it paints.

use bevy::prelude::*;
use rl_bevy::prelude::*;
use rl_bevy::{AbilityEvent, ItemEvent, MapChanged};
use rl_core::Point;
use rl_core::seed::position_hash;

use crate::map_view::{Glyph, MapView, draw_map};
use crate::terminal::Terminal;

/// A glyph flying along a path, a cell at a time, with a short fading
/// tail behind it.
#[derive(Debug, Clone, PartialEq)]
pub struct Trail {
    /// The cells, in order.
    pub path: Vec<Point>,
    /// What flies.
    pub glyph: char,
    /// Its colour at the head.
    pub color: Color,
    /// Seconds spent on each cell.
    pub cell_secs: f32,
}

/// Every cell of a footprint lit at once and fading out, each showing one
/// of a few glyphs picked by position, so a burst reads as embers rather
/// than a stamp.
#[derive(Debug, Clone, PartialEq)]
pub struct Burst {
    /// The cells.
    pub cells: Vec<Point>,
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
            Beat::Trail(t) => t.path.len() as f32 * t.cell_secs,
            Beat::Burst(b) => b.secs,
        }
    }

    /// What it shows `t` seconds in: cells, glyphs and colours, with the
    /// fraction each glyph has faded, 0 fresh and 1 gone.
    pub fn frame(&self, t: f32) -> Vec<Spark> {
        match self {
            Beat::Trail(trail) => {
                let head = (t / trail.cell_secs).floor() as usize;
                let Some(at) = trail.path.get(head) else { return Vec::new() };
                let mut sparks = vec![Spark { at: *at, glyph: trail.glyph, color: trail.color, faded: 0.0 }];
                // Three cells of tail, each older and fainter.
                for back in 1..=3 {
                    if let Some(cell) = head.checked_sub(back).and_then(|i| trail.path.get(i)) {
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
                    .cells
                    .iter()
                    .map(|cell| {
                        let pick = position_hash(0, cell.x, cell.y) as usize % burst.glyphs.len().max(1);
                        Spark { at: *cell, glyph: burst.glyphs.get(pick).copied().unwrap_or('*'), color: burst.color, faded }
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
}

impl Animation {
    /// How long the whole thing plays.
    pub fn duration(&self) -> f32 {
        self.steps.iter().map(Beat::duration).sum()
    }

    /// What shows `t` seconds in.
    pub fn frame(&self, t: f32) -> Vec<Spark> {
        let mut begun = 0.0;
        for step in &self.steps {
            let ends = begun + step.duration();
            if t < ends {
                return step.frame(t - begun);
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
    /// Plays `animation` from the next frame drawn.
    pub fn play(&mut self, animation: Animation) {
        if animation.duration() > 0.0 {
            self.playing.push((animation, None));
        }
    }

    /// Whether anything is playing.
    pub fn is_playing(&self) -> bool {
        !self.playing.is_empty()
    }

    /// What every animation shows at `now`, starting the ones that have
    /// not begun and dropping the ones that have finished.
    pub fn frame(&mut self, now: f32, map: MapId) -> Vec<Spark> {
        self.playing.retain(|(animation, _)| animation.map == map);
        let mut sparks = Vec::new();
        for (animation, began) in &mut self.playing {
            let began = *began.get_or_insert(now);
            sparks.extend(animation.frame(now - began));
        }
        self.playing.retain(|(animation, began)| began.is_none_or(|b| now - b < animation.duration()));
        sparks
    }
}

/// How the engine's own animations look: the flight and burst of an
/// ability with no [`Look`](rl_rules::ability::Look) of its own, and the pace of everything.
#[derive(Resource, Debug, Clone)]
pub struct ParticleStyle {
    /// The glyph and colour of an ability with no look.
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

/// Plays a flight and a burst for every ability used and every item
/// thrown, and draws whatever is playing over the map.
///
/// Draws after the map and before the overlays, so a burst shows through
/// a targeting cursor and under a menu.
pub struct ParticlesPlugin;

impl Plugin for ParticlesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Particles>().init_resource::<ParticleStyle>();
        app.add_message::<AbilityEvent>().add_message::<ItemEvent>().add_message::<MapChanged>();
        app.add_systems(Update, play_what_happened.in_set(PresentSet::Narrate)).add_systems(Update, draw_particles.in_set(PresentSet::Map).after(draw_map));
    }

    fn finish(&self, app: &mut App) {
        rl_bevy::depends_on::<crate::map_view::MapViewPlugin>(app, "ParticlesPlugin");
    }
}

/// What the plugin reads to turn an event into an animation.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Happened<'w, 's> {
    used: MessageReader<'w, 's, AbilityEvent>,
    thrown: MessageReader<'w, 's, ItemEvent>,
    abilities: Option<Res<'w, Abilities>>,
    map: Res<'w, WorldMap>,
    style: Res<'w, ParticleStyle>,
    glyphs: Query<'w, 's, &'static Glyph>,
}

/// Queues an animation for each ability used and item thrown this frame.
pub fn play_what_happened(mut particles: ResMut<Particles>, mut happened: Happened) {
    let here = happened.map.current();
    let style = happened.style.clone();
    for ev in happened.used.read() {
        let AbilityEvent::Used { ability, path, cells, .. } = ev else { continue };
        let (glyph, color) = happened
            .abilities
            .as_deref()
            .and_then(|a| a.get(*ability).look)
            .map(|look| (look.glyph, Color::srgb_u8(look.color.r, look.color.g, look.color.b)))
            .unwrap_or(style.plain);
        let mut steps = Vec::new();
        if !path.is_empty() {
            steps.push(Beat::Trail(Trail { path: path.clone(), glyph, color, cell_secs: style.cell_secs }));
        }
        if !cells.is_empty() {
            steps.push(Beat::Burst(Burst { cells: cells.clone(), glyphs: style.burst_glyphs.clone(), color, secs: style.burst_secs }));
        }
        particles.play(Animation { map: here, steps });
    }
    for ev in happened.thrown.read() {
        let ItemEvent::Thrown { item, path, .. } = ev else { continue };
        let (glyph, color) = happened.glyphs.get(*item).map(|g| (g.ch, g.fg)).unwrap_or(style.plain);
        if !path.is_empty() {
            particles.play(Animation { map: here, steps: vec![Beat::Trail(Trail { path: path.clone(), glyph, color, cell_secs: style.cell_secs })] });
        }
    }
}

/// Paints whatever is playing over the map, keeping each cell's
/// background and fading the glyph towards it.
pub fn draw_particles(mut terminal: ResMut<Terminal>, mut particles: ResMut<Particles>, view: Res<MapView>, map: Res<WorldMap>, time: Res<Time>) {
    if !particles.is_playing() {
        return;
    }
    for spark in particles.frame(time.elapsed_secs(), map.current()) {
        let Some(screen) = view.to_screen(spark.at) else { continue };
        let Some(mut cell) = terminal.get(screen.x, screen.y) else { continue };
        cell.glyph = spark.glyph;
        cell.fg = bevy::color::Mix::mix(&spark.color, &cell.bg, spark.faded.clamp(0.0, 1.0));
        terminal.set(screen.x, screen.y, cell);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trail() -> Trail {
        Trail { path: (1..=4).map(|x| Point::new(x, 0)).collect(), glyph: '*', color: Color::WHITE, cell_secs: 0.1 }
    }

    #[test]
    fn a_trail_moves_its_head_one_cell_per_cell_time_with_a_fading_tail() {
        let step = Beat::Trail(trail());
        assert_eq!(step.duration(), 0.4);
        let first = step.frame(0.0);
        assert_eq!((first[0].at, first[0].glyph, first.len()), (Point::new(1, 0), '*', 1), "no tail yet");
        let third = step.frame(0.25);
        assert_eq!(third[0].at, Point::new(3, 0), "the third cell, a quarter second in");
        assert_eq!(third.len(), 3, "the head and two cells of tail");
        assert!(third[1].faded < third[2].faded, "the older, the fainter");
        assert!(step.frame(0.4).is_empty(), "flown");
    }

    #[test]
    fn a_burst_lights_every_cell_at_once_and_fades_to_nothing() {
        let step =
            Beat::Burst(Burst { cells: vec![Point::new(0, 0), Point::new(1, 0), Point::new(0, 1)], glyphs: vec!['*', '+'], color: Color::WHITE, secs: 0.5 });
        let fresh = step.frame(0.0);
        assert_eq!(fresh.len(), 3);
        assert!(fresh.iter().all(|s| s.faded == 0.0));
        assert!(fresh.iter().all(|s| s.glyph == '*' || s.glyph == '+'));
        assert_eq!(step.frame(0.25)[0].faded, 0.5);
        assert!(step.frame(0.5).is_empty(), "gone");
    }

    #[test]
    fn steps_play_one_after_another_and_a_finished_animation_is_dropped() {
        let animation = Animation {
            map: MapId::SURFACE,
            steps: vec![Beat::Trail(trail()), Beat::Burst(Burst { cells: vec![Point::new(4, 0)], glyphs: vec!['*'], color: Color::WHITE, secs: 0.2 })],
        };
        assert_eq!(animation.duration(), 0.6);
        assert_eq!(animation.frame(0.35)[0].glyph, '*');
        assert_eq!(animation.frame(0.45).len(), 1, "the burst, once the trail has flown");
        assert!(animation.frame(0.7).is_empty());

        let mut particles = Particles::default();
        particles.play(animation);
        assert!(particles.is_playing());
        assert_eq!(particles.frame(10.0, MapId::SURFACE).len(), 1, "began on the first frame drawn");
        assert_eq!(particles.frame(10.45, MapId::SURFACE).len(), 1, "the burst");
        assert!(particles.frame(10.7, MapId::SURFACE).is_empty());
        assert!(!particles.is_playing(), "and it is gone");
    }

    #[test]
    fn an_animation_is_left_behind_on_the_map_it_played_on() {
        let mut particles = Particles::default();
        particles.play(Animation { map: MapId(3), steps: vec![Beat::Trail(trail())] });
        assert!(particles.frame(0.0, MapId(4)).is_empty());
        assert!(!particles.is_playing(), "dropped with the map");
    }
}
