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
//! watches the [`CueWait`] and lets go when the last of it has played, so
//! the eel is seen to spit before the crab moves, and two casters are two
//! flights one after the other. A key pressed while the turns wait is a
//! player who has seen enough: everything that holds them is dropped, the
//! turns run on to the player's own, and the key is read with that turn
//! in hand, so nothing a quick player presses is lost to a fade. Once the
//! player holds a turn nothing holds the turns, since there is nothing
//! left to wait for; what still plays, plays over the player's move.
//!
//! Three shapes cover what a roguelike animates: a `Trail` flying between
//! two anchors, a `Beam` held between two with a pulse running along it,
//! and a `Burst` over some. A burst with a source is a blast: it
//! goes off there and reaches each cell a moment after the one nearer it,
//! white-hot at its front and cooling to embers behind, with sparks thrown
//! ahead of it, and the fire and gas it left are held back until it has
//! reached them (see [`fields`](crate::fields)), so a grenade is seen to go
//! off and then to leave its smoke behind. An `Animation` is a list of
//! them played one after another, one per actor per frame, in the order the
//! actor's cues were written. A game with a look of its own writes a
//! [`Cued`] carrying it, as [`LookOf::Given`], and never an animation:
//! what plays and how it is paced is this plugin's, so a game's own flight
//! waits on the turns and is skipped by a key as the engine's are.
//!
//! Everything is drawn from the wall clock, so a frame that comes late
//! shows a later moment rather than a slower one, and a still frame costs
//! no more than the cells it paints.

use atglyph_bevy::prelude::*;
use atglyph_bevy::{Anchor, Cue, CueWait, Cued, LookOf};
use atglyph_core::Point;
use atglyph_core::geometry::line;
use atglyph_core::seed::position_hash;
use bevy::prelude::*;

use crate::fields::{Wavefront, Wavefronts};
use crate::map_view::{Glyph, MapView, draw_map};
use crate::terminal::Terminal;

/// Where an anchor is this frame: the entity's cell if it follows one that
/// is still somewhere, else the cell it was written with.
pub(crate) type Resolve<'a> = dyn Fn(&Anchor) -> Point + 'a;

/// A glyph flying from one anchor to the other, a cell at a time, with a
/// short fading tail behind it.
///
/// The line is drawn again every frame between where the anchors are
/// now, and the head's place along it is the share of the flight's time
/// that has passed, so a target that moves bends the flight rather than
/// leaving it in the air. The cell it leaves from is not part of it: what
/// stands there is what threw it, and stays drawn.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Trail {
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

/// Every anchor lit and fading out, each showing one of a few glyphs
/// picked by position, so a burst reads as embers rather than a stamp.
///
/// With no source every anchor lights at once. With one it is a blast: each
/// anchor lights [`ring_secs`](Self::ring_secs) later for every cell it is
/// further from the source, give or take a little so the front is ragged,
/// and then burns down through [`blast_glyphs`](ParticleStyle::blast_glyphs)
/// from white-hot to embers, lighting the ground under it as it goes.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Burst {
    /// Where.
    pub on: Vec<Anchor>,
    /// The glyphs the cells pick from: any of them for a burst, and hottest
    /// first for a blast, which moves each cell down them as it cools.
    pub glyphs: Vec<char>,
    /// The colour at the start, fading to the map's own.
    pub color: Color,
    /// How long each cell burns.
    pub secs: f32,
    /// Where it goes off, for a blast.
    pub from: Option<Anchor>,
    /// Seconds the front takes to go one cell further.
    pub ring_secs: f32,
}

impl Burst {
    /// When `cell` lights, in seconds after the blast went off at `from`:
    /// its distance in rings, plus up to half a ring by its position, so a
    /// front is not a square. The same sum [`Wavefront::reaches`] gives
    /// the fields it held back, so smoke shows where the fire has been.
    fn lights(&self, from: Point, cell: Point) -> f32 {
        Wavefront { from, begins: 0.0, ring_secs: self.ring_secs, ends: f32::INFINITY }.reaches(cell)
    }

    /// How long it plays: the last cell to light, and then its burning.
    fn duration(&self) -> f32 {
        match &self.from {
            None => self.secs,
            Some(from) => self.on.iter().map(|a| self.lights(from.at, a.at)).fold(0.0, f32::max) + self.secs,
        }
    }

    /// A blast `t` seconds in, from wherever `at` puts its source now.
    fn blast(&self, from: Point, t: f32, at: &Resolve<'_>) -> Vec<Spark> {
        let mut sparks = Vec::new();
        let ramp = self.glyphs.len().max(1);
        for anchor in &self.on {
            let cell = at(anchor);
            let age = t - self.lights(from, cell);
            if age < 0.0 || age >= self.secs {
                continue;
            }
            let cooled = age / self.secs;
            // Down the ramp as it cools, a cell a step ahead or behind its
            // neighbours by its position, so the ring is not one glyph.
            let step = ((cooled * ramp as f32) as usize + (position_hash(1, cell.x, cell.y) % 2) as usize).min(ramp - 1);
            let glyph = self.glyphs.get(step).copied().unwrap_or('*');
            // White at the front, the look's own colour behind it.
            let heat = (1.0 - cooled * 4.0).max(0.0);
            let color = self.color.mix(&Color::WHITE, heat * 0.8);
            // Nothing fades until the last third: a blast is a flash, not a
            // slow dimming.
            let faded = ((cooled - 0.66) * 3.0).clamp(0.0, 1.0);
            sparks.push(Spark { at: cell, glyph, color, faded, glow: (1.0 - cooled).powi(2) * 0.85 });
        }
        sparks.extend(self.thrown(from, t, at));
        sparks
    }

    /// Sparks thrown ahead of the front to the far edge of the blast, where
    /// it reaches farthest: a few, picked by position, each gone as the
    /// front arrives behind it.
    fn thrown(&self, from: Point, t: f32, at: &Resolve<'_>) -> Vec<Spark> {
        let far = self.on.iter().map(|a| atglyph_core::geometry::chebyshev(from, at(a))).max().unwrap_or(0);
        if far < 2 {
            return Vec::new();
        }
        let edge: Vec<Point> = self.on.iter().map(at).filter(|c| atglyph_core::geometry::chebyshev(from, *c) == far).collect();
        let count = edge.len().div_ceil(3).min(6);
        (0..count)
            .filter_map(|i| {
                let target = edge[position_hash(i as u64, from.x, from.y) as usize % edge.len()];
                // Twice the front's speed, so it is out ahead the whole way.
                let flight = self.lights(from, target) / 2.0;
                if t >= flight || flight <= 0.0 {
                    return None;
                }
                let path: Vec<Point> = line(from, target).skip(1).collect();
                let cell = *path.get(((t / flight) * path.len() as f32) as usize)?;
                Some(Spark { at: cell, glyph: '\'', color: self.color.mix(&Color::WHITE, 0.6), faded: 0.0, glow: 0.0 })
            })
            .collect()
    }
}

/// A line held between two anchors, drawn again every frame between where
/// they are now, with a pulse running along it from the first to the
/// second over its time.
///
/// Only the cells between the two are drawn: what stands at each end stays
/// drawn, as the thrower of a flight does. Two ends side by side have
/// nothing between them, and a beam between them takes no time.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Beam {
    /// The end the pulse leaves from.
    pub from: Anchor,
    /// The end the pulse runs to.
    pub to: Anchor,
    /// What the line is drawn with.
    pub glyph: char,
    /// The line's colour.
    pub color: Color,
    /// What runs along it, and in what colour, if anything.
    pub pulse: Option<(char, Color)>,
    /// Seconds it plays, if there is anything between its ends to see.
    pub secs: f32,
}

impl Beam {
    /// Whether the ends, as written, have a cell between them.
    fn spans(&self) -> bool {
        line(self.from.at, self.to.at).count() > 2
    }

    /// What it shows `t` seconds in, the line between where `at` puts its
    /// ends now. The line holds until its last quarter and then fades; the
    /// pulse stays bright to the end.
    fn frame(&self, t: f32, at: &Resolve<'_>) -> Vec<Spark> {
        let (from, to) = (at(&self.from), at(&self.to));
        let path: Vec<Point> = line(from, to).filter(|p| *p != from && *p != to).collect();
        let faded = ((t / self.secs - 0.75) * 4.0).clamp(0.0, 1.0);
        let head = ((t / self.secs) * path.len() as f32).floor() as usize;
        path.iter()
            .enumerate()
            .map(|(i, cell)| match self.pulse {
                Some((glyph, color)) if i == head => Spark { at: *cell, glyph, color, faded: 0.0, glow: 0.35 },
                _ => Spark { at: *cell, glyph: self.glyph, color: self.color, faded, glow: 0.0 },
            })
            .collect()
    }
}

/// An actor seen moving over cells it did not step.
///
/// Draws no sparks of its own: the map draws the actor where
/// [`Particles::sliders`] says it is while the slide plays, and the cell
/// it landed on shows whatever lies there until it arrives.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Slide {
    /// Who.
    pub who: Entity,
    /// The cells it passed, from where it stood to where it landed.
    pub path: Vec<Point>,
    /// Seconds on each cell.
    pub cell_secs: f32,
}

/// One beat of an animation: what plays before the next begins.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Beat {
    /// A flight.
    Trail(Trail),
    /// A burst.
    Burst(Burst),
    /// A beam.
    Beam(Beam),
    /// A slide.
    Slide(Slide),
}

impl Beat {
    /// How long it plays.
    pub(crate) fn duration(&self) -> f32 {
        match self {
            Beat::Trail(t) => t.cells as f32 * t.cell_secs,
            Beat::Burst(b) => b.duration(),
            Beat::Beam(b) if b.spans() => b.secs,
            Beat::Beam(_) => 0.0,
            Beat::Slide(s) => s.path.len().saturating_sub(1) as f32 * s.cell_secs,
        }
    }

    /// What it shows `t` seconds in, with the anchors where `at` puts
    /// them: cells, glyphs and colours, with the fraction each glyph has
    /// faded, 0 fresh and 1 gone.
    pub(crate) fn frame(&self, t: f32, at: &Resolve<'_>) -> Vec<Spark> {
        match self {
            Beat::Trail(trail) => {
                let duration = self.duration();
                if duration <= 0.0 || t >= duration {
                    return Vec::new();
                }
                let path: Vec<Point> = line(at(&trail.from), at(&trail.to)).skip(1).collect();
                let head = ((t / duration) * path.len() as f32).floor() as usize;
                let Some(cell) = path.get(head) else { return Vec::new() };
                let mut sparks = vec![Spark { at: *cell, glyph: trail.glyph, color: trail.color, faded: 0.0, glow: 0.0 }];
                // Three cells of tail, each older and fainter.
                for back in 1..=3 {
                    if let Some(cell) = head.checked_sub(back).and_then(|i| path.get(i)) {
                        sparks.push(Spark { at: *cell, glyph: '.', color: trail.color, faded: back as f32 / 4.0, glow: 0.0 });
                    }
                }
                sparks
            }
            Beat::Beam(beam) => {
                if t >= self.duration() {
                    return Vec::new();
                }
                beam.frame(t, at)
            }
            Beat::Slide(_) => Vec::new(),
            Beat::Burst(burst) => {
                if burst.secs <= 0.0 || t >= burst.duration() {
                    return Vec::new();
                }
                if let Some(from) = &burst.from {
                    return burst.blast(at(from), t, at);
                }
                let faded = t / burst.secs;
                burst
                    .on
                    .iter()
                    .map(|anchor| {
                        let cell = at(anchor);
                        let pick = position_hash(0, cell.x, cell.y) as usize % burst.glyphs.len().max(1);
                        Spark { at: cell, glyph: burst.glyphs.get(pick).copied().unwrap_or('*'), color: burst.color, faded, glow: 0.0 }
                    })
                    .collect()
            }
        }
    }
}

/// One glyph on one cell for one frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Spark {
    /// Where.
    pub at: Point,
    /// What.
    pub glyph: char,
    /// Its colour when fresh.
    pub color: Color,
    /// How far it has faded towards nothing, 0 to 1.
    pub faded: f32,
    /// How far it lights the ground under it towards its colour, 0 to 1:
    /// a blast's flash, where a flight and a plain burst light nothing.
    pub glow: f32,
}

/// Steps played one after another on one map.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Animation {
    /// The map it plays on. Left behind when the player leaves it.
    pub map: MapId,
    /// The steps, in order.
    pub steps: Vec<Beat>,
    /// Whether the turns wait for it.
    pub waits: bool,
}

impl Animation {
    /// How long the whole thing plays.
    pub(crate) fn duration(&self) -> f32 {
        self.steps.iter().map(Beat::duration).sum()
    }

    /// What shows `t` seconds in, with the anchors where `at` puts them.
    pub(crate) fn frame(&self, t: f32, at: &Resolve<'_>) -> Vec<Spark> {
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
    /// time is not played, and nothing waits on it.
    pub(crate) fn play(&mut self, animation: Animation) {
        if animation.duration() > 0.0 {
            self.playing.push((animation, None));
        }
    }

    /// Whether anything is playing.
    pub(crate) fn is_playing(&self) -> bool {
        !self.playing.is_empty()
    }

    /// Whether the turns wait on anything playing.
    pub fn is_waited_on(&self) -> bool {
        self.playing.iter().any(|(a, _)| a.waits)
    }

    /// Drops whatever the turns wait on, leaving the rest to play out.
    pub(crate) fn skip_waited(&mut self) {
        self.playing.retain(|(a, _)| !a.waits);
    }

    /// Every blast playing or still to play on `map`, as the front the
    /// fields wait for, with one that has not begun taken to begin `now`,
    /// which is when the next frame drawn will begin it.
    pub(crate) fn wavefronts(&self, now: f32, map: MapId, at: &Resolve<'_>) -> Vec<Wavefront> {
        let mut fronts = Vec::new();
        for (animation, began) in self.playing.iter().filter(|(a, _)| a.map == map) {
            let mut begins = began.unwrap_or(now);
            for step in &animation.steps {
                if let Beat::Burst(Burst { from: Some(from), ring_secs, .. }) = step {
                    fronts.push(Wavefront { from: at(from), begins, ring_secs: *ring_secs, ends: begins + step.duration() });
                }
                begins += step.duration();
            }
        }
        fronts
    }

    /// Every actor a slide is showing on its way at `now`, and the cell it
    /// is on: the first cell of its path until the slide begins, even
    /// while what plays before it in the same animation plays, then each
    /// cell for a cell's time. An actor in two slides is where the first
    /// that has not ended puts it, so a second shove follows the first.
    pub(crate) fn sliders(&self, now: f32) -> Vec<(Entity, Point)> {
        let mut out: Vec<(Entity, Point)> = Vec::new();
        for (animation, began) in &self.playing {
            let t = began.map_or(0.0, |b| now - b);
            let mut begun = 0.0;
            for step in &animation.steps {
                let ends = begun + step.duration();
                if let Beat::Slide(slide) = step
                    && t < ends
                    && !out.iter().any(|(e, _)| *e == slide.who)
                {
                    let cell = if t <= begun || slide.cell_secs <= 0.0 { 0 } else { ((t - begun) / slide.cell_secs).floor() as usize };
                    if let Some(at) = slide.path.get(cell.min(slide.path.len().saturating_sub(1))) {
                        out.push((slide.who, *at));
                    }
                }
                begun = ends;
            }
        }
        out
    }

    /// What every animation shows at `now` with the anchors where `at`
    /// puts them, starting the ones that have not begun and dropping the
    /// ones that have finished.
    pub(crate) fn frame(&mut self, now: f32, map: MapId, at: &Resolve<'_>) -> Vec<Spark> {
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

/// Seconds a shoved actor is seen on each cell it passes: quicker than a
/// walk, so it reads as thrown, and slow enough to be seen at all.
pub const SLIDE_SECS: f32 = 0.04;

/// How the engine's own animations look: the flight and burst of anything
/// with no look of its own, and the pace of everything.
#[derive(Resource, Debug, Clone)]
pub struct ParticleStyle {
    /// The glyph and colour of a cue with no look.
    pub plain: (char, Color),
    /// Seconds a flight spends on each cell.
    pub cell_secs: f32,
    /// Seconds a burst takes to fade, and each cell of a blast to burn.
    pub burst_secs: f32,
    /// What a burst's cells show.
    pub burst_glyphs: Vec<char>,
    /// Seconds a blast's front takes to go one cell further.
    pub ring_secs: f32,
    /// What a blast's cells show as they cool, hottest first.
    pub blast_glyphs: Vec<char>,
    /// Seconds a beam is held; `None` holds it as long as a flight over
    /// the same cells would take, and never less than a burst.
    pub beam_secs: Option<f32>,
    /// Seconds a shoved or pulled actor spends on each cell it is seen to
    /// slide over.
    pub slide_secs: f32,
}

impl Default for ParticleStyle {
    fn default() -> Self {
        Self {
            plain: ('*', Color::srgb(0.9, 0.9, 0.8)),
            cell_secs: 0.035,
            burst_secs: 0.4,
            burst_glyphs: vec!['*', '+', '\u{00b7}', 'x'],
            ring_secs: 0.06,
            // Not `#`, which is a wall in most games and would read as
            // one going up.
            blast_glyphs: vec!['*', '%', '+', '\u{00b7}'],
            beam_secs: None,
            slide_secs: SLIDE_SECS,
        }
    }
}

impl ParticleStyle {
    /// A style that plays nothing worth waiting for: every pace nought, so
    /// nothing holds the turns, the looks as the default's. The one way to
    /// write it that a pace added later cannot leave behind.
    pub fn instant() -> Self {
        Self { cell_secs: 0.0, burst_secs: 0.0, ring_secs: 0.0, beam_secs: Some(0.0), slide_secs: 0.0, ..Self::default() }
    }

    /// Whether anything drawn in this style takes time to watch.
    pub(crate) fn takes_time(&self) -> bool {
        self.cell_secs > 0.0 || self.burst_secs > 0.0 || self.slide_secs > 0.0 || self.beam_secs.is_some_and(|s| s > 0.0)
    }

    /// How long a beam over `cells` cells is held.
    pub(crate) fn beam_time(&self, cells: usize) -> f32 {
        self.beam_secs.unwrap_or_else(|| (cells as f32 * self.cell_secs).max(self.burst_secs))
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
        app.init_resource::<Particles>().init_resource::<ParticleStyle>().init_resource::<CueWait>();
        app.add_message::<Cued>().add_message::<MapChanged>();
        app.add_systems(atglyph_bevy::EndRun, |mut particles: ResMut<Particles>| *particles = Particles::default())
            .add_systems(OnEnter(EngineState::Playing), watch_unless_instant)
            .add_systems(Update, skip_on_key.before(EngineSet::Input).run_if(in_state(EngineState::Playing)))
            .add_systems(Update, (play_cues, place_sliders, publish_wavefronts).chain().in_set(PresentSet::Narrate))
            .add_systems(Update, (draw_particles, wait_on_particles).chain().in_set(PresentSet::Map).after(draw_map));
    }

    fn finish(&self, app: &mut App) {
        app.depends_on::<crate::map_view::MapViewPlugin>("ParticlesPlugin");
    }
}

/// Takes the turns' hold, unless the style plays nothing worth waiting
/// for. Decided when play begins, so a game's style, inserted after the
/// plugin, is the one read.
pub(crate) fn watch_unless_instant(style: Res<ParticleStyle>, mut cue_wait: ResMut<CueWait>) {
    if style.takes_time() {
        cue_wait.watch();
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
        let given = |l: atglyph_rules::ability::ParticleLook| (l.glyph, Color::srgb_u8(l.color.r, l.color.g, l.color.b));
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
            Cue::Beam { from, to, look, pulse } => {
                let (glyph, color) = self.look(*look);
                let secs = self.style.beam_time(line(from.at, to.at).skip(1).count());
                Beat::Beam(Beam { from: *from, to: *to, glyph, color, pulse: pulse.map(|p| self.look(p)), secs })
            }
            Cue::Burst { on, look, from } => {
                let (_, color) = self.look(*look);
                let glyphs = if from.is_some() { &self.style.blast_glyphs } else { &self.style.burst_glyphs };
                Beat::Burst(Burst { on: on.clone(), glyphs: glyphs.clone(), color, secs: self.style.burst_secs, from: *from, ring_secs: self.style.ring_secs })
            }
            Cue::Slide { who, path } => Beat::Slide(Slide { who: *who, path: path.clone(), cell_secs: self.style.slide_secs }),
        }
    }
}

/// Queues one animation per actor that cued something this frame, its
/// cues in the order written, each holding the turns; a slide goes in the
/// animation of whoever slides.
pub(crate) fn play_cues(mut particles: ResMut<Particles>, mut cues: Cues) {
    let here = cues.map.current();
    let mut per_actor: Vec<(Entity, Animation)> = Vec::new();
    let cued: Vec<Cued> = cues.cued.read().cloned().collect();
    for Cued { actor, cue } in &cued {
        let beat = cues.beat(cue);
        // A slide plays in the moved one's own animation, so two shoves on
        // it in one pass slide it one way and then the other, in turn.
        let actor = match cue {
            Cue::Slide { who, .. } => who,
            _ => actor,
        };
        match per_actor.iter_mut().find(|(a, _)| a == actor) {
            Some((_, animation)) => animation.steps.push(beat),
            None => per_actor.push((*actor, Animation { map: here, steps: vec![beat], waits: true })),
        }
    }
    for (_, animation) in per_actor {
        particles.play(animation);
    }
}

/// Where `anchor` is now: on whom it follows, where that one is drawn, on
/// its way through a slide or else where it stands, or the cell it was
/// written with once that one is gone. So a blow's flash on a creature
/// then shoved plays where it was struck, as the creature is drawn.
fn followed(anchor: &Anchor, sliding: &crate::map_view::Sliding, positions: &Query<&Position>) -> Point {
    anchor.follow.and_then(|e| sliding.at(e).or_else(|| positions.get(e).ok().map(|p| p.0))).unwrap_or(anchor.at)
}

/// Tells the map view where every sliding actor is to be drawn this frame.
pub(crate) fn place_sliders(particles: Res<Particles>, mut sliding: ResMut<crate::map_view::Sliding>, time: Res<Time>) {
    sliding.0 = particles.sliders(time.elapsed_secs());
}

/// Tells the map view where every blast's front is, so the fire and gas a
/// blast left show only once it has reached them.
pub(crate) fn publish_wavefronts(
    particles: Res<Particles>,
    mut fronts: ResMut<Wavefronts>,
    (map, time, sliding): (Res<WorldMap>, Res<Time>, Res<crate::map_view::Sliding>),
    positions: Query<&Position>,
) {
    let at = |anchor: &Anchor| followed(anchor, &sliding, &positions);
    fronts.0 = particles.wavefronts(time.elapsed_secs(), map.current(), &at);
}

/// Paints whatever is playing over the map, keeping each cell's
/// background and fading the glyph towards it, and lighting the background
/// under a blast's flash, with every anchor where what it follows stands
/// now.
pub(crate) fn draw_particles(
    mut terminal: ResMut<Terminal>,
    mut particles: ResMut<Particles>,
    view: Res<MapView>,
    map: Res<WorldMap>,
    time: Res<Time>,
    (positions, sliding): (Query<&Position>, Res<crate::map_view::Sliding>),
) {
    if !particles.is_playing() {
        return;
    }
    let at = |anchor: &Anchor| followed(anchor, &sliding, &positions);
    for spark in particles.frame(time.elapsed_secs(), map.current(), &at) {
        let Some(mut cell) = view.get(&terminal, spark.at) else { continue };
        cell.glyph = spark.glyph;
        // Lit toward a darker shade of itself, so the glyph over it still
        // reads.
        cell.bg = cell.bg.mix(&spark.color.mix(&Color::BLACK, 0.45), spark.glow.clamp(0.0, 1.0));
        cell.fg = spark.color.mix(&cell.bg, spark.faded.clamp(0.0, 1.0));
        view.set(&mut terminal, spark.at, cell);
    }
}

/// A key pressed while the turns wait skips what holds them and runs the
/// turns on at once, before the game reads the key, so the key finds the
/// player holding the turn it was pressed for rather than being dropped
/// on a frame nobody held one.
pub fn skip_on_key(world: &mut World) {
    if !world.resource::<CueWait>().is_waiting() {
        return;
    }
    // A key pressed, or a key held: a repeat is paced by `atglyph-ui`, which
    // this crate is below, so it arrives as an ask rather than as a
    // `just_pressed` this would see. Taken either way, so an ask that
    // arrives while nothing holds does not bank a skip for later.
    let asked = world.resource_mut::<CueWait>().take_skip_asked();
    let pressed = world.get_resource::<ButtonInput<KeyCode>>().is_some_and(|keys| keys.get_just_pressed().next().is_some());
    if !pressed && !asked {
        return;
    }
    // Through every hold in the way: a flight skipped lands, and its
    // burst would hold again, and a second caster's flight after that.
    // Bounded, since the turn loop's own bound is per call.
    for _ in 0..MAX_SKIPS {
        world.resource_mut::<Particles>().skip_waited();
        world.resource_mut::<CueWait>().release();
        atglyph_bevy::plugin::run_turns(world);
        if !world.resource::<CueWait>().is_waiting() {
            return;
        }
    }
}

/// Holds the loop skips through at most on one key.
const MAX_SKIPS: usize = 64;

/// Keeps the turns waiting exactly while something they wait on plays and
/// there is a turn to wait for, after the frame that would have dropped
/// what finished. With the player holding a turn nothing is waited for:
/// the loop would not run anyway, and waiting would only keep the
/// player's next key from being read.
pub(crate) fn wait_on_particles(particles: Res<Particles>, mut cue_wait: ResMut<CueWait>, player: Query<(), (With<Player>, With<MyTurn>)>) {
    if particles.is_waited_on() && player.is_empty() {
        cue_wait.wait();
    } else {
        cue_wait.release();
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

    /// An instant style takes no time, so nothing holds the turns, and every
    /// pace in it is nought, a slide's among them.
    #[test]
    fn an_instant_style_takes_no_time() {
        let instant = ParticleStyle::instant();
        assert!(!instant.takes_time());
        assert_eq!((instant.cell_secs, instant.burst_secs, instant.slide_secs, instant.ring_secs), (0.0, 0.0, 0.0, 0.0));
    }

    /// A slide shows its actor on the cell it was moved from until it
    /// begins, then on each cell it passed in turn, a cell's time apiece,
    /// and nowhere once it has landed, where the map draws it as ever.
    #[test]
    fn a_slide_shows_its_actor_at_the_start_until_it_begins_then_along_its_path() {
        let who = Entity::from_bits(9);
        let path = vec![Point::new(1, 1), Point::new(2, 1), Point::new(3, 1)];
        let slide = Beat::Slide(Slide { who, path: path.clone(), cell_secs: 0.1 });
        assert_eq!(slide.duration(), 0.2, "two cells to cross");
        assert!(slide.frame(0.05, &fixed).is_empty(), "the map draws it; nothing sparks");
        let mut particles = Particles::default();
        particles.play(Animation { map: MapId::ZERO, steps: vec![Beat::Trail(trail()), slide], waits: true });
        assert_eq!(particles.sliders(5.0), vec![(who, path[0])], "not begun: where it stood");
        particles.frame(5.0, MapId::ZERO, &fixed);
        assert_eq!(particles.sliders(5.3), vec![(who, path[0])], "the flight before it is still playing");
        assert_eq!(particles.sliders(5.45), vec![(who, path[0])], "a cell's time on its first cell");
        assert_eq!(particles.sliders(5.55), vec![(who, path[1])], "then the next");
        assert!(particles.sliders(5.65).is_empty(), "and landed");
    }

    /// Through the plugin: a shoved actor is drawn on each cell it passed,
    /// its own cell showing the ground while it is on its way, and on its
    /// own cell once it lands; and the turns wait for it.
    #[test]
    fn a_sliding_actor_is_drawn_along_its_path_and_at_its_cell_once_it_lands() {
        use atglyph_core::GridRect;
        let mut app = atglyph_bevy::testing::headless_app();
        app.add_plugins((atglyph_bevy::fov::FovPlugin, crate::map_view::MapViewPlugin::new(GridRect::new(0, 0, 40, 20)), ParticlesPlugin));
        app.insert_resource(Terminal::new(40, 20, Vec2::ONE));
        app.insert_resource(ParticleStyle { slide_secs: 0.05, ..ParticleStyle::default() });
        let start = atglyph_bevy::testing::ground(&mut app);
        app.world_mut().spawn((Player, Position(start), Viewshed::new(8), RevealsMap));
        let landed = start.offset(4, 0);
        let crab = app.world_mut().spawn((Position(landed), Glyph::new('c', Color::WHITE))).id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        let shown = |app: &App, p: Point| app.world().resource::<crate::map_view::MapView>().get(app.world().resource::<Terminal>(), p).map(|c| c.glyph);
        assert_eq!(shown(&app, landed), Some('c'), "drawn where it stands");

        let path = vec![start.offset(2, 0), start.offset(3, 0), landed];
        app.world_mut().write_message(Cued { actor: crab, cue: Cue::Slide { who: crab, path: path.clone() } });
        app.update();
        assert_eq!(shown(&app, path[0]), Some('c'), "seen where it was moved from");
        assert_ne!(shown(&app, landed), Some('c'), "and not yet where it landed");
        assert!(app.world().resource::<CueWait>().is_waiting(), "the turns wait for it");
        let mut seen = Vec::new();
        let began = app.world().resource::<Time>().elapsed_secs();
        while app.world().resource::<Time>().elapsed_secs() - began < 0.3 {
            app.update();
            seen.extend(path.iter().copied().filter(|p| shown(&app, *p) == Some('c')));
        }
        assert!(seen.contains(&path[1]), "it passed the cell between: {seen:?}");
        assert_eq!(shown(&app, landed), Some('c'), "and stands where it landed");
        assert!(!app.world().resource::<CueWait>().is_waiting(), "and let the turns go");
    }

    /// A creature shoved by two in one pass is seen to slide the first way
    /// and then the second, from where the first left it, never both at
    /// once nor jumping to the end: its slides are one animation in the
    /// order written, whoever shoved it.
    #[test]
    fn a_creature_shoved_twice_in_one_pass_slides_each_way_in_turn() {
        use atglyph_core::GridRect;
        let mut app = atglyph_bevy::testing::headless_app();
        app.add_plugins((crate::map_view::MapViewPlugin::new(GridRect::new(0, 0, 40, 20)), ParticlesPlugin));
        app.insert_resource(Terminal::new(40, 20, Vec2::ONE));
        let start = atglyph_bevy::testing::ground(&mut app);
        app.world_mut().spawn((Player, Position(start), Viewshed::new(8), RevealsMap));
        let crab = app.world_mut().spawn(Position(start.offset(4, 2))).id();
        let (one, two) = (app.world_mut().spawn_empty().id(), app.world_mut().spawn_empty().id());
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        let first = vec![start.offset(2, 0), start.offset(3, 0), start.offset(4, 0)];
        let second = vec![start.offset(4, 0), start.offset(4, 1), start.offset(4, 2)];
        app.world_mut().write_message(Cued { actor: one, cue: Cue::Slide { who: crab, path: first.clone() } });
        app.world_mut().write_message(Cued { actor: two, cue: Cue::Slide { who: crab, path: second.clone() } });
        app.update();
        let particles = app.world().resource::<Particles>();
        let slides: Vec<usize> =
            particles.playing.iter().map(|(a, _)| a.steps.iter().filter(|s| matches!(s, Beat::Slide(_))).count()).filter(|n| *n > 0).collect();
        assert_eq!(slides, [2], "both slides in one animation, one after the other");
    }

    /// A blow's burst on a creature then shoved is seen where it was hit,
    /// on the cell it slides from, not on the empty cell it is about to
    /// land on.
    #[test]
    fn a_burst_on_a_shoved_creature_plays_where_it_was_struck() {
        use atglyph_core::GridRect;
        let mut app = atglyph_bevy::testing::headless_app();
        app.add_plugins((atglyph_bevy::fov::FovPlugin, crate::map_view::MapViewPlugin::new(GridRect::new(0, 0, 40, 20)), ParticlesPlugin));
        app.insert_resource(Terminal::new(40, 20, Vec2::ONE));
        app.insert_resource(ParticleStyle { slide_secs: 0.05, burst_secs: 0.2, ..ParticleStyle::default() });
        let start = atglyph_bevy::testing::ground(&mut app);
        app.world_mut().spawn((Player, Position(start), Viewshed::new(8), RevealsMap));
        let landed = start.offset(4, 0);
        let crab = app.world_mut().spawn((Position(landed), Glyph::new('c', Color::WHITE))).id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        let path = vec![start.offset(2, 0), start.offset(3, 0), landed];
        app.world_mut().write_message(Cued { actor: crab, cue: Cue::Burst { on: vec![Anchor::on(crab, landed)], look: LookOf::Plain, from: None } });
        app.world_mut().write_message(Cued { actor: crab, cue: Cue::Slide { who: crab, path: path.clone() } });
        app.update();
        let shown = |app: &App, p: Point| app.world().resource::<crate::map_view::MapView>().get(app.world().resource::<Terminal>(), p).map(|c| c.glyph);
        let burst = ParticleStyle::default().burst_glyphs;
        assert!(shown(&app, path[0]).is_some_and(|g| burst.contains(&g)), "the flash where it was struck: {:?}", shown(&app, path[0]));
        assert!(!shown(&app, landed).is_some_and(|g| burst.contains(&g)), "and none where it has yet to land");
    }

    /// A slide whose actor is gone before it ends still ends, leaves no
    /// glyph behind, and lets the turns go.
    #[test]
    fn a_slide_whose_actor_is_gone_ends_and_releases_the_turn() {
        use atglyph_core::GridRect;
        let mut app = atglyph_bevy::testing::headless_app();
        app.add_plugins((atglyph_bevy::fov::FovPlugin, crate::map_view::MapViewPlugin::new(GridRect::new(0, 0, 40, 20)), ParticlesPlugin));
        app.insert_resource(Terminal::new(40, 20, Vec2::ONE));
        app.insert_resource(ParticleStyle { slide_secs: 0.05, ..ParticleStyle::default() });
        let start = atglyph_bevy::testing::ground(&mut app);
        app.world_mut().spawn((Player, Position(start), Viewshed::new(8), RevealsMap));
        let crab = app.world_mut().spawn((Position(start.offset(4, 0)), Glyph::new('c', Color::WHITE))).id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        let path = vec![start.offset(2, 0), start.offset(3, 0), start.offset(4, 0)];
        app.world_mut().write_message(Cued { actor: crab, cue: Cue::Slide { who: crab, path: path.clone() } });
        app.update();
        app.world_mut().despawn(crab);
        let began = app.world().resource::<Time>().elapsed_secs();
        while app.world().resource::<Time>().elapsed_secs() - began < 0.3 {
            app.update();
        }
        let shown = |p: Point| app.world().resource::<crate::map_view::MapView>().get(app.world().resource::<Terminal>(), p).map(|c| c.glyph);
        assert!(path.iter().all(|p| shown(*p) != Some('c')), "nothing left of it");
        assert!(!app.world().resource::<CueWait>().is_waiting(), "and the turns go on");
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

    /// A beam from the cell at the origin to the one six east, its pulse
    /// if `pulse`, playing for six tenths of a second.
    fn beam(pulse: bool) -> Beam {
        let to = Anchor::on(Entity::from_bits(9), Point::new(6, 0));
        Beam { from: Anchor::cell(Point::new(0, 0)), to, glyph: '~', color: Color::WHITE, pulse: pulse.then_some(('*', Color::BLACK)), secs: 0.6 }
    }

    /// Every cell between the two ends carries the beam the whole time it
    /// plays, the ends themselves left to whoever stands on them, and
    /// nothing once it is over.
    #[test]
    fn a_beam_draws_its_line_for_its_time_and_ends() {
        let step = Beat::Beam(beam(false));
        assert_eq!(step.duration(), 0.6);
        let between: Vec<Point> = (1..6).map(|x| Point::new(x, 0)).collect();
        for t in [0.0, 0.3, 0.59] {
            let drawn = step.frame(t, &fixed);
            assert_eq!(drawn.iter().map(|s| s.at).collect::<Vec<_>>(), between, "every cell between the ends at {t}");
            assert!(drawn.iter().all(|s| s.glyph == '~'), "and the beam's glyph on each");
        }
        assert!(step.frame(0.6, &fixed).is_empty(), "over");
        let side_by_side = Beam { to: Anchor::cell(Point::new(1, 0)), ..beam(true) };
        assert_eq!(Beat::Beam(side_by_side).duration(), 0.0, "two ends side by side have nothing between them to show");
    }

    /// The pulse runs along the beam from its first end to its second, over
    /// the beam's glyph where it stands; a beam with none shows only itself.
    #[test]
    fn a_pulse_travels_from_the_first_anchor_to_the_second() {
        let step = Beat::Beam(beam(true));
        let pulse = |t: f32| {
            let drawn = step.frame(t, &fixed);
            let pulses: Vec<Point> = drawn.iter().filter(|s| s.glyph == '*').map(|s| s.at).collect();
            assert_eq!(pulses.len(), 1, "one pulse at {t}");
            assert_eq!(drawn.len(), 5, "over the beam, not beside it");
            pulses[0]
        };
        let (early, late) = (pulse(0.15), pulse(0.45));
        assert!(early.x < 3 && late.x > 3, "nearer the first end, then the second: {early:?}, {late:?}");
        assert!(Beat::Beam(beam(false)).frame(0.3, &fixed).iter().all(|s| s.glyph == '~'), "and no pulse unless asked");
    }

    /// The ends go where what they follow goes, and an end whose entity is
    /// gone holds the cell it was written with, as a flight's does.
    #[test]
    fn a_beam_follows_its_anchors() {
        let step = Beat::Beam(beam(false));
        let moved = |a: &Anchor| if a.follow.is_some() { Point::new(6, 3) } else { a.at };
        let bent: Vec<Point> = line(Point::new(0, 0), Point::new(6, 3)).skip(1).take(5).collect();
        assert_eq!(step.frame(0.3, &moved).iter().map(|s| s.at).collect::<Vec<_>>(), bent, "bent to where it stands now");
        assert_eq!(step.frame(0.3, &fixed).last().map(|s| s.at), Some(Point::new(5, 0)), "or ending where it stood");
    }

    #[test]
    fn a_burst_lights_every_cell_at_once_and_fades_to_nothing() {
        let on = vec![Anchor::cell(Point::new(0, 0)), Anchor::cell(Point::new(1, 0)), Anchor::cell(Point::new(0, 1))];
        let step = Beat::Burst(Burst { on, glyphs: vec!['*', '+'], color: Color::WHITE, secs: 0.5, from: None, ring_secs: 0.0 });
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
        let step =
            Beat::Burst(Burst { on: vec![Anchor::on(me, Point::new(2, 2))], glyphs: vec!['*'], color: Color::WHITE, secs: 0.5, from: None, ring_secs: 0.0 });
        let stepped = |a: &Anchor| if a.follow == Some(me) { Point::new(3, 2) } else { a.at };
        assert_eq!(step.frame(0.1, &stepped)[0].at, Point::new(3, 2));
    }

    fn blast(radius: i32) -> Burst {
        let on = (-radius..=radius).flat_map(|y| (-radius..=radius).map(move |x| Anchor::cell(Point::new(x, y)))).collect();
        Burst { on, glyphs: vec!['*', '%', '+', '.'], color: Color::srgb(1.0, 0.5, 0.1), secs: 0.4, from: Some(Anchor::cell(Point::new(0, 0))), ring_secs: 0.1 }
    }

    /// A blast goes off where it landed and reaches each ring after the
    /// one inside it, white-hot and lighting the ground at its front, and
    /// is over once its last cell has burned down.
    #[test]
    fn a_blast_goes_off_at_its_source_and_spreads_ring_by_ring() {
        let step = Beat::Burst(blast(3));
        let lit = |t: f32| step.frame(t, &fixed).into_iter().filter(|s| s.glyph != '\'').map(|s| s.at).collect::<Vec<_>>();
        assert_eq!(lit(0.0), vec![Point::new(0, 0)], "only where it went off, at first");
        let early = lit(0.16);
        assert!(early.iter().all(|p| atglyph_core::geometry::chebyshev(*p, Point::new(0, 0)) <= 1), "a ring out a tenth of a second on: {early:?}");
        assert!(early.len() > 1);
        let at_front = step.frame(0.0, &fixed)[0];
        assert_eq!(at_front.glyph, '*', "hottest glyph first");
        assert!(at_front.glow > 0.5 && at_front.faded == 0.0, "lighting the ground, not yet fading");
        assert!(at_front.color.to_srgba().blue > 0.5, "white at the front, not yet the look's own orange");
        let later = step.frame(0.35, &fixed).into_iter().find(|s| s.at == Point::new(0, 0)).expect("still burning");
        assert!(later.glow < at_front.glow, "cooling");
        assert!(
            step.duration() > 3.0 * 0.1 + 0.4 - 1e-4 && step.duration() < 3.5 * 0.1 + 0.4 + 1e-4,
            "the last ring, give or take its jitter, then its burning: {}",
            step.duration()
        );
        assert!(step.frame(step.duration(), &fixed).is_empty(), "and over");
    }

    /// Sparks fly out ahead of a blast to its far edge, and a blast too
    /// small to have an edge throws none.
    #[test]
    fn a_blast_throws_sparks_ahead_of_its_front() {
        let sparks = |b: &Burst, t: f32| b.thrown(Point::new(0, 0), t, &fixed);
        let wide = blast(3);
        let early = sparks(&wide, 0.05);
        assert!(!early.is_empty(), "thrown as soon as it goes off");
        assert!(early.iter().all(|s| atglyph_core::geometry::chebyshev(s.at, Point::new(0, 0)) <= 3), "and within its reach: {early:?}");
        assert!(sparks(&wide, 0.3).len() < early.len() || sparks(&wide, 0.3).is_empty(), "gone as the front catches up");
        assert!(sparks(&blast(1), 0.01).is_empty(), "nothing thrown from a blast a cell wide");
    }

    /// The front the fields wait for is where the blast is: after a flight
    /// before it, from where it went off, and gone once the blast is over.
    #[test]
    fn a_blast_after_a_flight_tells_the_fields_when_its_front_starts() {
        let mut particles = Particles::default();
        let (flight, burst) = (Beat::Trail(trail()), Beat::Burst(blast(2)));
        let (flown, burned) = (flight.duration(), burst.duration());
        particles.play(Animation { map: MapId::ZERO, steps: vec![flight, burst], waits: true });
        let fronts = particles.wavefronts(2.0, MapId::ZERO, &fixed);
        assert_eq!(fronts.len(), 1);
        let front = fronts[0];
        assert_eq!(front.from, Point::new(0, 0));
        assert!((front.begins - (2.0 + flown)).abs() < 1e-4, "after the flight, taking an animation not yet begun to begin now");
        assert!((front.ends - (2.0 + flown + burned)).abs() < 1e-4);
        assert!(particles.wavefronts(2.0, MapId(9), &fixed).is_empty(), "and nothing on another map");
    }

    #[test]
    fn steps_play_one_after_another_and_a_finished_animation_is_dropped() {
        let burst = Burst { on: vec![Anchor::cell(Point::new(4, 0))], glyphs: vec!['*'], color: Color::WHITE, secs: 0.2, from: None, ring_secs: 0.0 };
        let animation = Animation { map: MapId::ZERO, steps: vec![Beat::Trail(trail()), Beat::Burst(burst)], waits: true };
        assert_eq!(animation.duration(), 0.6);
        assert_eq!(animation.frame(0.35, &fixed)[0].glyph, '*');
        assert_eq!(animation.frame(0.45, &fixed).len(), 1, "the burst, once the trail has flown");
        assert!(animation.frame(0.7, &fixed).is_empty());

        let mut particles = Particles::default();
        particles.play(animation);
        assert!(particles.is_playing() && particles.is_waited_on());
        assert_eq!(particles.frame(10.0, MapId::ZERO, &fixed).len(), 1, "began on the first frame drawn");
        assert_eq!(particles.frame(10.45, MapId::ZERO, &fixed).len(), 1, "the burst");
        assert!(particles.frame(10.7, MapId::ZERO, &fixed).is_empty());
        assert!(!particles.is_playing() && !particles.is_waited_on(), "and it is gone, and holds nothing");
    }

    #[test]
    fn an_animation_is_left_behind_on_the_map_it_played_on() {
        let mut particles = Particles::default();
        particles.play(Animation { map: MapId(3), steps: vec![Beat::Trail(trail())], waits: true });
        assert!(particles.frame(0.0, MapId(4), &fixed).is_empty());
        assert!(!particles.is_playing(), "dropped with the map");
    }

    /// Through the plugin: one actor's cues in one frame are one animation
    /// in the order written, holding the turns, one at the player as much
    /// as any; and the hold lets go when the last of it has played.
    #[test]
    fn cues_become_one_animation_per_actor_that_holds_the_turns_until_it_has_played() {
        use atglyph_core::GridRect;
        let mut app = atglyph_bevy::testing::headless_app();
        app.add_plugins((crate::map_view::MapViewPlugin::new(GridRect::new(0, 0, 40, 20)), ParticlesPlugin));
        app.insert_resource(Terminal::new(40, 20, Vec2::ONE));
        app.insert_resource(ParticleStyle { cell_secs: 0.01, burst_secs: 0.05, ..ParticleStyle::default() });
        let start = atglyph_bevy::testing::ground(&mut app);
        let me = app.world_mut().spawn((Player, Position(start), Viewshed::new(8), RevealsMap)).id();
        let (eel, crab) = (app.world_mut().spawn(Position(start.offset(3, 0))).id(), app.world_mut().spawn(Position(start.offset(-3, 0))).id());
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();

        let look = LookOf::Plain;
        app.world_mut().write_message(Cued {
            actor: eel,
            cue: Cue::Flight { from: Anchor::on(eel, start.offset(3, 0)), to: Anchor::on(crab, start.offset(-3, 0)), look },
        });
        app.world_mut().write_message(Cued { actor: eel, cue: Cue::Burst { on: vec![Anchor::on(crab, start.offset(-3, 0))], look, from: None } });
        app.world_mut().write_message(Cued { actor: crab, cue: Cue::Flight { from: Anchor::on(crab, start.offset(-3, 0)), to: Anchor::on(me, start), look } });
        app.update();
        let particles = app.world().resource::<Particles>();
        assert_eq!(particles.playing.len(), 2, "one per actor");
        let (eels, crabs) = (&particles.playing[0].0, &particles.playing[1].0);
        assert!(matches!(eels.steps.as_slice(), [Beat::Trail(_), Beat::Burst(_)]) && eels.waits, "the flight, then the burst, and the turns wait");
        assert!(matches!(crabs.steps.as_slice(), [Beat::Trail(_)]) && crabs.waits, "at the player, and seen to arrive like any other");
        assert!(app.world().resource::<CueWait>().is_waiting());

        let began = app.world().resource::<Time>().elapsed_secs();
        while app.world().resource::<Time>().elapsed_secs() - began < 0.2 {
            app.update();
        }
        assert!(!app.world().resource::<Particles>().is_playing(), "played out");
        assert!(!app.world().resource::<CueWait>().is_waiting(), "and let go");
    }

    /// Through the plugin: a beam cued holds the turns while it plays and
    /// lets go when it ends, its time the style's, or by default as long as
    /// a flight over the same cells and never shorter than a burst.
    #[test]
    fn a_beam_cued_holds_the_turns_until_it_has_played() {
        use atglyph_core::GridRect;
        let mut app = atglyph_bevy::testing::headless_app();
        app.add_plugins((crate::map_view::MapViewPlugin::new(GridRect::new(0, 0, 40, 20)), ParticlesPlugin));
        app.insert_resource(Terminal::new(40, 20, Vec2::ONE));
        app.insert_resource(ParticleStyle { beam_secs: Some(0.05), ..ParticleStyle::default() });
        let start = atglyph_bevy::testing::ground(&mut app);
        app.world_mut().spawn((Player, Position(start), Viewshed::new(8), RevealsMap));
        let caster = app.world_mut().spawn(Position(start.offset(4, 0))).id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        let cue = Cue::Beam { from: Anchor::cell(start), to: Anchor::on(caster, start.offset(4, 0)), look: LookOf::Plain, pulse: Some(LookOf::Plain) };
        app.world_mut().write_message(Cued { actor: caster, cue });
        app.update();
        let particles = app.world().resource::<Particles>();
        let [(animation, _)] = particles.playing.as_slice() else { panic!("one animation") };
        assert!(matches!(animation.steps.as_slice(), [Beat::Beam(b)] if b.secs == 0.05 && b.pulse.is_some()), "a beam, timed by the style");
        assert!(app.world().resource::<CueWait>().is_waiting(), "and the turns wait");
        let began = app.world().resource::<Time>().elapsed_secs();
        while app.world().resource::<Time>().elapsed_secs() - began < 0.2 {
            app.update();
        }
        assert!(!app.world().resource::<CueWait>().is_waiting(), "let go once played");

        let style = ParticleStyle::default();
        assert_eq!(style.beam_time(3), style.burst_secs, "a short beam lasts as long as a burst");
        assert_eq!(style.beam_time(40), 40.0 * style.cell_secs, "a long one as long as a flight over it");
    }

    /// A key pressed while the turns wait drops what holds them and lets
    /// the turns go, in the frame the key is read.
    #[test]
    fn a_key_pressed_while_the_turns_wait_skips_what_holds_them() {
        use atglyph_core::GridRect;
        let mut app = atglyph_bevy::testing::headless_app();
        app.add_plugins((crate::map_view::MapViewPlugin::new(GridRect::new(0, 0, 40, 20)), ParticlesPlugin, atglyph_bevy::testing::KeyScriptPlugin));
        app.insert_resource(Terminal::new(40, 20, Vec2::ONE));
        app.insert_resource(ParticleStyle { cell_secs: 1.0, burst_secs: 1.0, ..ParticleStyle::default() });
        let start = atglyph_bevy::testing::ground(&mut app);
        app.world_mut().spawn((Player, Position(start), Viewshed::new(8), RevealsMap));
        let eel = app.world_mut().spawn(Position(start.offset(3, 0))).id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.world_mut().write_message(Cued { actor: eel, cue: Cue::Burst { on: vec![Anchor::on(eel, start.offset(3, 0))], look: LookOf::Plain, from: None } });
        app.update();
        assert!(app.world().resource::<CueWait>().is_waiting() && app.world().resource::<Particles>().is_waited_on());
        app.world_mut().resource_mut::<atglyph_bevy::testing::KeyScript>().press(KeyCode::KeyH);
        app.update();
        assert!(!app.world().resource::<Particles>().is_waited_on(), "dropped");
        assert!(!app.world().resource::<CueWait>().is_waiting(), "and let go, without waiting for the frame's end");
    }

    /// The same, for a key the player is *holding*: a repeat is paced by
    /// `atglyph-ui` and never arrives as a `just_pressed`, so walking with a
    /// direction key down waited out every cue in sight. It asks through
    /// `CueWait::ask_skip`, and this is the answering half.
    #[test]
    fn a_skip_asked_for_while_the_turns_wait_skips_what_holds_them_the_way_a_press_does() {
        use atglyph_core::GridRect;
        let mut app = atglyph_bevy::testing::headless_app();
        app.add_plugins((crate::map_view::MapViewPlugin::new(GridRect::new(0, 0, 40, 20)), ParticlesPlugin));
        app.insert_resource(Terminal::new(40, 20, Vec2::ONE));
        app.insert_resource(ParticleStyle { cell_secs: 1.0, burst_secs: 1.0, ..ParticleStyle::default() });
        let start = atglyph_bevy::testing::ground(&mut app);
        app.world_mut().spawn((Player, Position(start), Viewshed::new(8), RevealsMap));
        let eel = app.world_mut().spawn(Position(start.offset(3, 0))).id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.world_mut().write_message(Cued { actor: eel, cue: Cue::Burst { on: vec![Anchor::on(eel, start.offset(3, 0))], look: LookOf::Plain, from: None } });
        app.update();
        assert!(app.world().resource::<CueWait>().is_waiting(), "the burst holds the turns");

        // No key is pressed at all: this is the repeat's own path.
        app.world_mut().resource_mut::<CueWait>().ask_skip();
        app.update();
        assert!(!app.world().resource::<Particles>().is_waited_on(), "dropped");
        assert!(!app.world().resource::<CueWait>().is_waiting(), "and let go");
    }

    #[test]
    fn an_ask_is_taken_once_so_one_repeat_skips_one_hold() {
        let mut cue_wait = CueWait::default();
        assert!(!cue_wait.take_skip_asked(), "nothing asked");
        cue_wait.ask_skip();
        assert!(cue_wait.take_skip_asked(), "asked");
        assert!(!cue_wait.take_skip_asked(), "and not still asking, or a key down would skip everything after it");
    }

    /// The player cancels an animation and takes the hit: a shot fired at
    /// the player is in the air, the turns wait on its flight, and a key
    /// skips it through the real skip path. The key finds the player
    /// holding its turn with the shot already landed, so nothing a quick
    /// player presses is read against a health bar the shot has not yet
    /// reached.
    #[test]
    fn a_key_pressed_while_a_shot_at_the_player_flies_lands_it_before_the_player_moves() {
        use atglyph_core::{DiceRoll, GridRect};
        use atglyph_rules::ai::{Brain, tactics::ShootAtRange};
        let mut app = atglyph_bevy::testing::headless_app();
        app.add_plugins((crate::map_view::MapViewPlugin::new(GridRect::new(0, 0, 40, 20)), ParticlesPlugin, atglyph_bevy::testing::KeyScriptPlugin));
        app.add_plugins((FovPlugin, CombatPlugin, MindsPlugin));
        app.insert_resource(Terminal::new(40, 20, Vec2::ONE));
        app.insert_resource(ParticleStyle { cell_secs: 1.0, burst_secs: 1.0, ..ParticleStyle::default() });
        let start = atglyph_bevy::testing::ground(&mut app);
        let sides = atglyph_bevy::testing::two_sides(&mut app);
        let me = app.world_mut().spawn((Actor, Player, Blocks, Position(start), Viewshed::new(8), RevealsMap, Health::full(30), Faction(sides.ours))).id();
        let look = atglyph_rules::ability::ParticleLook { glyph: '*', color: atglyph_grid::Rgb::new(255, 80, 40) };
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
        assert!(app.world().resource::<CueWait>().in_flight(), "the shot is in the air");
        assert!(app.world().resource::<CueWait>().is_waiting() && app.world().resource::<Particles>().is_waited_on(), "and the turns wait on it");
        assert_eq!(hp(&app), 30, "it has not arrived");
        assert!(app.world().get::<MyTurn>(me).is_none());

        app.world_mut().resource_mut::<atglyph_bevy::testing::KeyScript>().press(KeyCode::KeyH);
        app.update();
        assert_eq!(hp(&app), 27, "the key landed the shot");
        assert!(!app.world().resource::<CueWait>().in_flight());
        assert!(app.world().get::<MyTurn>(me).is_some(), "and the player holds its turn, hurt, in the frame the key was read");
    }

    /// The cells the head of `animation` is drawn on, in order, each once
    /// however many frames it stays there, with every anchor where it was
    /// written.
    fn head_path(animation: &Animation) -> Vec<Point> {
        let mut path: Vec<Point> = Vec::new();
        let (duration, frames) = (animation.duration(), 1000);
        for i in 0..frames {
            let Some(head) = animation.frame(duration * i as f32 / frames as f32, &fixed).first().map(|s| s.at) else { continue };
            if path.last() != Some(&head) {
                path.push(head);
            }
        }
        path
    }

    /// The path a bolt is drawn along, from a real cast through the
    /// plugin: turned back, out to the body that turned it, onto its cell,
    /// and home to the caster's own, so the picture agrees with the log's
    /// line that it was reflected; not turned back, out to the body and no
    /// further, as before. Over a span of seeds, so both outcomes are seen.
    #[test]
    fn a_reflected_bolt_is_drawn_to_the_body_that_turned_it_and_back_to_its_caster() {
        use atglyph_core::{GridRect, RunSeed};
        let (mut reflected, mut landed) = (0, 0);
        for seed in 0..40 {
            let mut app = atglyph_bevy::testing::headless_app();
            app.add_plugins((crate::map_view::MapViewPlugin::new(GridRect::new(0, 0, 40, 20)), ParticlesPlugin));
            app.add_plugins((FovPlugin, CombatPlugin, MindsPlugin, AbilitiesPlugin));
            app.add_engine_effects();
            app.insert_resource(Terminal::new(40, 20, Vec2::ONE));
            app.insert_resource(ParticleStyle { cell_secs: 0.01, burst_secs: 0.05, ..ParticleStyle::default() });
            let start = atglyph_bevy::testing::ground(&mut app);
            let sides = atglyph_bevy::testing::two_sides(&mut app);
            app.insert_resource(atglyph_bevy::Seed(RunSeed(seed)));
            let kinds = atglyph_rules::Registry::from_defs(vec![atglyph_rules::DamageKind::new("kinetic")]).unwrap();
            let abilities = Abilities::load(
                r#"[(name: "bolt", mode: Bolt(range: 8), effects: [(kind: "Harm", args: (kind: "kinetic", roll: "1"))])]"#,
                app.world().resource::<EffectKinds>(),
                &atglyph_rules::Names::new().damage_kinds(&kinds),
            )
            .expect("the bolt loads");
            let bolt = abilities.expect("bolt");
            app.insert_resource(abilities);
            let me = app
                .world_mut()
                .spawn((Actor, Player, Blocks, Position(start), Viewshed::new(8), RevealsMap, Health::full(30), Faction(sides.ours), Grants(vec![bolt])))
                .id();
            let there = start.offset(3, 0);
            app.world_mut().spawn((Actor, Blocks, Position(there), Health::full(30), Faction(sides.theirs), Reflects::default()));
            app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
            app.update();
            app.update();
            assert!(app.world().get::<MyTurn>(me).is_some(), "seed {seed}: the caster holds the turn");

            app.world_mut().write_message(Intent::new(me, Use { ability: bolt, aim: there }));
            app.update();
            let turned = app.world_mut().resource_mut::<Messages<Reflected>>().drain().count() > 0;
            let particles = app.world().resource::<Particles>();
            let [(flight, _)] = particles.playing.as_slice() else { panic!("seed {seed}: the cast is one animation: {:?}", particles.playing) };
            let path = head_path(flight);
            let out = vec![start.offset(1, 0), start.offset(2, 0), there];
            if turned {
                reflected += 1;
                assert!(matches!(flight.steps.as_slice(), [Beat::Trail(_), Beat::Trail(_)]), "seed {seed}: two flights: {:?}", flight.steps);
                let back = vec![start.offset(2, 0), start.offset(1, 0), start];
                assert_eq!(path, [out, back].concat(), "seed {seed}: out to the body that turned it, and home to the caster");
            } else {
                landed += 1;
                assert_eq!(path, out, "seed {seed}: out to the body it lands on, and no further");
            }
        }
        assert!(reflected > 0 && landed > 0, "both outcomes over forty seeds: {reflected} reflected, {landed} landed");
    }
}
