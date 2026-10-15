//! The nearby rail: who is in sight, and what is lying about.
//!
//! The row picked out is drawn on the selection tone, and so is the map
//! tile it names while no cursor is up, so a player stepping down the list
//! with Tab sees which glyph each row is. A cursor draws its own mark on
//! the map, and the rail leaves the map to it.

use std::collections::BTreeMap;

use bevy::prelude::*;
use rl_bevy::PresentSet;
use rl_core::Rect;
use rl_render::{Cell, Terminal};
use rl_rules::Relation;

use crate::cursor::{CursorMap, CursorStyle};
use crate::facet::{Facet, FacetId, Facets};
use crate::modal::Modals;
use crate::panel::{clear, clip, frame, section, wrap};
use crate::tone::{Palette, ToneId, Tones};
use crate::view::{Alert, NearbyFollowers, NearbyView, NearbyViewPlugin, Row};

/// Where the nearby rail is drawn and what the headings say.
#[derive(Resource, Debug, Clone)]
pub struct NearbyLayout {
    /// The terminal cells it occupies, border included.
    pub rect: Rect,
    /// The title in the top border. Empty draws no frame.
    pub title: String,
    /// The heading over the actors, with how many of them are hostile at
    /// its right when any are: nought is left unsaid, since a `0` over a
    /// list of people reads as a count of them that is wrong.
    pub actors: String,
    /// The heading over the things: every item and prop in sight, nearest
    /// first, which is more than what lies underfoot.
    pub things: String,
    /// What each state is called, in the game's own words. An empty word
    /// says nothing at all for that state, which is what a game that
    /// wants only "hunting" written does with the other two.
    pub alerts: AlertWords,
    /// What a monster is doing, by the tactic that decided its last turn, in
    /// the game's own words; written in place of the alert word for a tactic
    /// the game named.
    pub activities: ActivityWords,
    /// How the cell of the row picked out is marked on the map. A glow by
    /// default, since the rail's own highlight is a glow and the two read
    /// as one thing.
    pub cursor: CursorStyle,
    /// The facets drawn on lines of their own beneath a row, by key,
    /// rather than after its name: a note too long to share a line with
    /// a name on a narrow rail, such as what someone carries. Empty draws
    /// every facet after the name.
    pub beneath: Vec<String>,
}

/// What each [`Alert`] is called on a row, in the game's own words.
///
/// The engine knows the three states and will not name them: one game's
/// monsters sleep where another's stand idle, and a droid does neither.
/// The defaults are plain enough to ship with and plain enough to
/// replace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlertWords {
    /// What something that knows of nothing is called.
    pub unaware: String,
    /// What something on its way to a noise is called.
    pub searching: String,
    /// What something that has noticed you is called.
    pub hunting: String,
}

impl Default for AlertWords {
    fn default() -> Self {
        Self { unaware: "unaware".into(), searching: "searching".into(), hunting: "hunting".into() }
    }
}

impl AlertWords {
    /// The three, in order.
    pub fn new(unaware: impl Into<String>, searching: impl Into<String>, hunting: impl Into<String>) -> Self {
        Self { unaware: unaware.into(), searching: searching.into(), hunting: hunting.into() }
    }

    /// What `alert` is called, or `None` where the game left it unsaid.
    pub fn get(&self, alert: Alert) -> Option<&str> {
        let word = match alert {
            Alert::Unaware => &self.unaware,
            Alert::Searching => &self.searching,
            Alert::Hunting => &self.hunting,
        };
        (!word.is_empty()).then_some(word.as_str())
    }
}

/// What a monster is doing, by the name of the tactic that decided its last
/// turn, in the game's own words.
///
/// An alert says whether a monster has noticed you, which is not what it is
/// doing about it: one that has you in sight and is running is not hunting
/// you. The engine names no activity itself, since what a retreat is called
/// is a game's, so a game names the tactics worth naming, its own among
/// them, and a tactic it leaves unnamed reads as the alert. Keyed by the
/// tactic's own trace name, the one [`Doing`](rl_bevy::Doing) records.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ActivityWords(BTreeMap<String, String>);

impl ActivityWords {
    /// No activity named: every row reads its alert.
    pub fn new() -> Self {
        Self::default()
    }

    /// The same, with the tactic called `tactic` written as `word`.
    pub fn word(mut self, tactic: impl Into<String>, word: impl Into<String>) -> Self {
        self.0.insert(tactic.into(), word.into());
        self
    }

    /// What the tactic called `tactic` is written as, if the game named it.
    pub fn get(&self, tactic: &str) -> Option<&str> {
        self.0.get(tactic).map(String::as_str).filter(|w| !w.is_empty())
    }
}

/// Draws [`NearbyView`] as a rail.
///
/// Adds [`NearbyViewPlugin`] if the game has not, because a panel with no
/// view behind it can only draw nothing.
pub struct NearbyPanel(NearbyLayout, NearbyFollowers);

impl NearbyPanel {
    /// A rail in `rect`, with the engine's default headings: `In sight`
    /// over the actors and `Things in sight` over the things, which says
    /// what that list holds where a game has not said it in its own words.
    pub fn new(rect: Rect) -> Self {
        Self(
            NearbyLayout {
                rect,
                title: "Nearby".into(),
                actors: "In sight".into(),
                things: "Things in sight".into(),
                alerts: AlertWords::default(),
                activities: ActivityWords::default(),
                cursor: CursorStyle::glow(Tones::SELECT),
                beneath: Vec::new(),
            },
            NearbyFollowers::Listed,
        )
    }

    /// Replaces the title in the top border. Empty draws no frame, which
    /// is what a rail flush against the map wants.
    pub fn titled(mut self, title: impl Into<String>) -> Self {
        self.0.title = title.into();
        self
    }

    /// Sets how the cell of the row picked out is marked on the map.
    pub fn cursor(mut self, style: CursorStyle) -> Self {
        self.0.cursor = style;
        self
    }

    /// Replaces the two headings.
    pub fn headings(mut self, actors: impl Into<String>, things: impl Into<String>) -> Self {
        self.0.actors = actors.into();
        self.0.things = things.into();
        self
    }

    /// Sets what each state is called on a row.
    pub fn alerts(mut self, words: AlertWords) -> Self {
        self.0.alerts = words;
        self
    }

    /// Sets what each named activity is called on a row.
    pub fn activities(mut self, words: ActivityWords) -> Self {
        self.0.activities = words;
        self
    }

    /// Draws the facets under `key` beneath their row rather than after
    /// its name: wrapped whole, one facet after another, while the rail
    /// has room for every row so drawn, and each row's on one clipped line
    /// when it has not, joined as the facets after a name are. A row picked
    /// out is picked out with its lines.
    pub fn beneath(mut self, key: impl Into<String>) -> Self {
        self.0.beneath.push(key.into());
        self
    }

    /// Leaves the player's followers off the rail, for a game whose party
    /// panel lists them: listed here too, each would spend a second row.
    /// Followers are listed by default, as anyone in sight is, which is
    /// what a game with no party panel wants. The view is what changes, as
    /// [`NearbyFollowers::LeftOut`] says, so Tab steps past them too.
    pub fn without_followers(mut self) -> Self {
        self.1 = NearbyFollowers::LeftOut;
        self
    }
}

impl Plugin for NearbyPanel {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<NearbyViewPlugin>() {
            app.add_plugins(NearbyViewPlugin);
        }
        if self.1 == NearbyFollowers::LeftOut {
            app.insert_resource(self.1);
        }
        app.insert_resource(self.0.clone()).add_systems(Update, draw_nearby.in_set(PresentSet::Chrome));
    }
}

/// The tone a row reads in: what it is to the player, or plain text when
/// it takes no side.
pub fn relation_tone(relation: Option<Relation>) -> ToneId {
    match relation {
        Some(Relation::Hostile) => Tones::BAD,
        Some(Relation::Allied) => Tones::GOOD,
        _ => Tones::TEXT,
    }
}

/// What the rail draws: its layout, its view, and the facet keys the
/// layout names, so the system that paints it reads them as one.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Rail<'w> {
    layout: Res<'w, NearbyLayout>,
    view: Res<'w, NearbyView>,
    facets: Res<'w, Facets>,
}

/// Paints the rail, and the tile of the row picked out.
pub fn draw_nearby(mut terminal: ResMut<Terminal>, rail: Rail, palette: Res<Palette>, modals: Res<Modals>, map: CursorMap, time: Res<Time>) {
    let (layout, view) = (&*rail.layout, &*rail.view);
    if let Some(focused) = view.picked()
        && !modals.any_open()
    {
        // The same mark the look cursor and the targeting cursor use, so
        // the row picked out and the cell it stands on read as one thing.
        map.mark(&mut terminal, focused.at, layout.cursor, &palette, time.elapsed_secs());
    }
    let rect = layout.rect;
    if rect.width < 6 || rect.height < 4 {
        return;
    }
    clear(&mut terminal, rect, &palette);
    let inner = if layout.title.is_empty() {
        rect
    } else {
        frame(&mut terminal, rect, &layout.title, "", &palette);
        Rect::new(rect.x + 1, rect.y + 1, rect.width - 2, rect.height - 2)
    };
    let beneath = Beneath::new(&layout.beneath, &rail.facets, inner.width);
    let threats = Some(view.threats()).filter(|n| *n > 0);
    let sections = [(&layout.actors, &view.actors, threats), (&layout.things, &view.things, None)];
    // Every row's notes wrapped whole when the whole rail fits so, and
    // each row's on one clipped line when it does not: a list that ran
    // off the bottom to say all of the first row's would hide the rows
    // under it, which are what the rail is for.
    let wrapped = |rows: &Vec<Row>| rows.iter().map(|r| 1 + beneath.lines(r, false).len()).sum::<usize>();
    let whole: usize = sections.iter().filter(|(_, rows, _)| !rows.is_empty()).map(|(_, rows, _)| 3 + wrapped(rows)).sum();
    let compact = whole > inner.height as usize + 1;
    let mut y = inner.y;
    let bottom = inner.bottom();
    for (heading, rows, count) in sections {
        if rows.is_empty() || y + 3 > bottom {
            continue;
        }
        section(&mut terminal, inner, y, heading, count, &palette);
        y += 2;
        let lines: Vec<Vec<(String, ToneId)>> = rows.iter().map(|r| beneath.lines(r, compact)).collect();
        // Keep the row picked out on the rail, notes and all: when it
        // falls below what fits, the list starts late enough to show it
        // last.
        let room = (bottom - y).max(0) as usize;
        let skip = rows.iter().position(|r| view.is_focused(r)).map_or(0, |focused| {
            let mut used = 0;
            (0..=focused)
                .rev()
                .find(|&i| {
                    used += 1 + lines[i].len();
                    used > room
                })
                .map_or(0, |i| i + 1)
                .min(focused)
        });
        for (row, notes) in rows.iter().zip(&lines).skip(skip) {
            if y >= bottom {
                break;
            }
            let focused = view.is_focused(row);
            draw_row(&mut terminal, Rect::new(inner.x, y, inner.width, 1), row, focused, &beneath, layout, &palette);
            y += 1;
            let base = palette.get(if focused { Tones::SELECT } else { Tones::SURFACE });
            y = paint_beneath(&mut terminal, inner, y, bottom, notes, base, &palette);
        }
        y += 1;
    }
}

/// How much of the health's tone a row's wash is mixed of, the rest the
/// row's own background.
///
/// A fifth: enough that a row reads at a glance as a bar and a wounded
/// one stands out by its colour, and little enough that the name over it
/// keeps its own. At more than a third, every unhurt row was a slab of
/// the good tone, an ally's name in that same tone read green on green,
/// and the row picked out was barely told from the rest.
pub(crate) const WASH: f32 = 0.2;

/// Draws `row` on `line`, one cell high, with the facets `beneath` does
/// not take after its name.
fn draw_row(terminal: &mut Terminal, line: Rect, row: &Row, focused: bool, beneath: &Beneath, layout: &NearbyLayout, palette: &Palette) {
    let base = palette.get(if focused { Tones::SELECT } else { Tones::SURFACE });
    let mut name = row.label.clone();
    for facet in row.facets.iter().filter(|f| !beneath.takes(f)) {
        name.push_str(" \u{00b7} ");
        name.push_str(&facet.text);
    }
    // What it is doing, in the panel's own words and after the name: a
    // state a player reads rather than a mark they learn. What it is busy
    // with outranks what it knows of you, since something at work is not
    // coming for anyone, whatever it has noticed.
    let state = row
        .work
        .as_ref()
        .map(|w| w.doing.as_str())
        .or_else(|| row.doing.and_then(|tactic| layout.activities.get(tactic)))
        .or_else(|| row.alert.and_then(|alert| layout.alerts.get(alert)));
    let state = state.map(|word| format!(" ({word})")).unwrap_or_default();
    // Something that knows of nothing reads muted, so the names in their
    // own colour are the ones that know you are there.
    let tone = if row.alert == Some(Alert::Unaware) { Tones::MUTED } else { relation_tone(row.relation) };
    let text = clip_keeping(&name, &state, (line.width - 2).max(0) as usize);
    paint_row(terminal, line, row, base, &text, tone, palette);
}

/// Paints `row` on `line`, one cell high: its glyph, then `text` in
/// `tone`, over the row as its own health bar.
///
/// Health fills the line from the left in the health's own tone, mixed
/// into `base` rather than painted over it, so a wounded thing reads at
/// a glance without a column of its own and without shouting. A row
/// with no health is `base` edge to edge. The nearby rail and the party
/// rail both draw their rows so, and a follower reads as hurt by the
/// same wash a foe beside it does.
pub(crate) fn paint_row(terminal: &mut Terminal, line: Rect, row: &Row, base: Color, text: &str, tone: ToneId, palette: &Palette) {
    let (x0, y) = (line.x, line.y);
    terminal.fill(line, Cell::new(' ', base).on(base));
    let filled = match row.health {
        Some(_) => ((line.width as f32 * row.health_fraction()).round() as i32).clamp(0, line.width),
        None => 0,
    };
    let hurt = palette.get(row.health.map_or(Tones::GOOD, |(current, max)| crate::view::health_tone(current, max)));
    let wash = base.mix(&hurt, WASH);
    if filled > 0 {
        terminal.fill(Rect::new(x0, y, filled, 1), Cell::new(' ', base).on(wash));
    }
    let bg_at = |x: i32| if x < x0 + filled { wash } else { base };
    terminal.print_on(x0, y, &row.glyph.ch.to_string(), row.glyph.fg, bg_at(x0));
    for (i, ch) in text.chars().enumerate() {
        let x = x0 + 2 + i as i32;
        terminal.print_on(x, y, &ch.to_string(), palette.get(tone), bg_at(x));
    }
}

/// Paints `lines` under a row from `y`, set in under its name, each on
/// `base` edge to edge so a row and its notes read as one block, and
/// stops at `bottom`. Returns the line after the last one drawn.
pub(crate) fn paint_beneath(terminal: &mut Terminal, inner: Rect, mut y: i32, bottom: i32, lines: &[(String, ToneId)], base: Color, palette: &Palette) -> i32 {
    for (line, tone) in lines {
        if y >= bottom {
            break;
        }
        terminal.fill(Rect::new(inner.x, y, inner.width, 1), Cell::new(' ', base).on(base));
        terminal.print_on(inner.x + 2, y, line, palette.get(*tone), base);
        y += 1;
    }
    y
}

/// The facets a rail draws beneath a row rather than after its name, and
/// how they are laid out there.
///
/// Each is wrapped whole onto as many lines as it takes, as the inspect
/// panel wraps its facets, set in two cells under the name; or, where the
/// rail has no room for that, all of a row's on one line, joined as the
/// facets after a name are and clipped, so every row keeps one line of
/// what it says rather than the first row keeping all of it.
pub(crate) struct Beneath {
    keys: Vec<FacetId>,
    width: usize,
}

impl Beneath {
    /// The facets keyed `names`, in a rail `width` cells wide. A key no
    /// game has declared is a key no row carries.
    pub(crate) fn new(names: &[String], facets: &Facets, width: i32) -> Self {
        Self { keys: names.iter().filter_map(|n| facets.get(n)).collect(), width: (width - 2).max(0) as usize }
    }

    /// No facet drawn beneath: every one after the name.
    #[cfg(test)]
    pub(crate) fn none() -> Self {
        Self { keys: Vec::new(), width: 0 }
    }

    /// Whether `facet` is drawn beneath its row.
    pub(crate) fn takes(&self, facet: &Facet) -> bool {
        self.keys.contains(&facet.key)
    }

    /// The lines beneath `row`, each in its facet's tone: every facet
    /// wrapped, or all of them on one clipped line when `compact`.
    pub(crate) fn lines(&self, row: &Row, compact: bool) -> Vec<(String, ToneId)> {
        let notes: Vec<&Facet> = row.facets.iter().filter(|f| self.takes(f)).collect();
        let Some(first) = notes.first() else { return Vec::new() };
        if compact {
            let joined = notes.iter().map(|f| f.text.as_str()).collect::<Vec<_>>().join(" \u{00b7} ");
            return vec![(clip(&joined, self.width), first.tone)];
        }
        notes.iter().flat_map(|f| wrap(&f.text, self.width).into_iter().map(|line| (line, f.tone))).collect()
    }
}

/// The fewest of a name's own characters, its ellipsis included, kept
/// beside the state after it: a row narrower than that clips the two
/// together from the end, since a state with no name to it says nothing.
const NAME_KEPT: usize = 4;

/// `name` then `tail` within `width`, the name clipped to make room for the
/// tail when the two do not fit.
///
/// The tail is what the thing is doing, and cut from the end with the
/// rest, as `Grubnak the goblin king (unawar…` was, it says neither who
/// nor what; the name's end is the part a reader can most do without. A
/// row too narrow to keep [`NAME_KEPT`] of the name beside the tail is
/// clipped from the end as a whole instead.
pub(crate) fn clip_keeping(name: &str, tail: &str, width: usize) -> String {
    let (named, told) = (name.chars().count(), tail.chars().count());
    if named + told <= width {
        return format!("{name}{tail}");
    }
    if width >= told + NAME_KEPT {
        return format!("{}{tail}", clip(name, width - told));
    }
    clip(&format!("{name}{tail}"), width)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cursor::CursorKeys;
    use crate::harness::Stage;
    use rl_render::MapView;

    #[test]
    fn the_rail_prints_a_heading_a_glyph_a_name_and_the_row_itself_is_the_health_bar() {
        let mut stage = Stage::new(NearbyPanel::new(Rect::new(0, 0, 24, 10)).titled("")).screen(24, 10);
        stage.actor("crab", 'c', 2, 0);
        stage.thing("rum", '!', 1, 0);
        stage.tick();

        let rows = stage.rows();
        assert_eq!(rows[0], "In sight               1", "the heading carries the threat count");
        assert!(rows[1].starts_with('\u{2500}'), "underlined: {:?}", rows[1]);
        assert!(rows[2].starts_with("c crab"), "glyph then name: {:?}", rows[2]);
        assert_eq!(rows[4], "Things in sight", "the second heading says what it lists, and has no count");
        assert!(rows[6].starts_with("! rum"), "{:?}", rows[6]);

        // The row is the bar: a crab at full health is washed edge to
        // edge, and a bottle on the floor, which has no health, is not
        // washed at all.
        let (surface, good) = {
            let palette = stage.app.world().resource::<Palette>();
            (palette.get(Tones::SURFACE), palette.get(Tones::GOOD))
        };
        let full = surface.mix(&good, WASH);
        for x in [0, 12, 23] {
            assert_eq!(bg(&stage, x, 2), full, "the crab's row is washed at column {x}");
        }
        assert_eq!(bg(&stage, 12, 6), surface, "a thing on the floor has no health to show");
    }

    /// With nobody hostile in sight the heading carries no count, so a
    /// `0` never stands over a list of people as though it counted them.
    #[test]
    fn the_heading_says_no_count_when_nothing_in_sight_is_hostile() {
        let mut stage = Stage::new(NearbyPanel::new(Rect::new(0, 0, 24, 10)).titled("")).screen(24, 10);
        let keeper = stage.actor("keeper", 'k', 2, 0);
        stage.app.world_mut().entity_mut(keeper).remove::<rl_bevy::Faction>();
        stage.tick();
        let rows = stage.rows();
        assert_eq!(rows[0], "In sight", "no count at nought");
        assert!(rows[2].starts_with("k keeper"), "{:?}", rows[2]);
    }

    #[test]
    fn one_coming_to_look_at_a_sound_carries_a_question_and_one_that_has_seen_you_the_bang_instead() {
        let rules = rl_bevy::NoiseRules { step: 0, strike: 0, door: 0, landing: 0, door_muffle: 0 };
        let panel = NearbyPanel::new(Rect::new(0, 0, 24, 10)).titled("");
        let mut stage = Stage::new((panel, rl_bevy::NoisePlugin::new(rules))).screen(24, 10);
        let rat = stage.actor("rat", 'r', 2, 0);
        stage.app.world_mut().entity_mut(rat).insert(rl_bevy::Hearing::default());
        let at = stage.at;
        stage.app.world_mut().get_mut::<rl_bevy::Heard>(rat).unwrap().0 = rl_rules::Awareness::Alert { at, stale_turns: 0 };
        stage.tick();
        assert!(stage.rows()[2].contains("rat (searching)"), "{:?}", stage.rows()[2]);

        // And one that has seen you is hunting, whatever it heard.
        let mut row = stage.app.world().resource::<NearbyView>().actors[0].clone();
        row.alert = Some(Alert::Hunting);
        let palette = stage.app.world().resource::<Palette>().clone();
        let mut terminal = Terminal::new(24, 1, bevy::math::Vec2::ONE);
        draw_row(&mut terminal, Rect::new(0, 0, 24, 1), &row, false, &Beneath::none(), &NearbyPanel::new(Rect::new(0, 0, 24, 10)).0, &palette);
        let said: String = (0..24).filter_map(|x| terminal.get(x, 0).map(|c| c.glyph)).collect();
        assert!(said.contains("(hunting)"), "{said:?}");
    }

    /// What a monster is doing, in the game's words, outranks whether it has
    /// seen you: one that has you in sight and is running reads fleeing, not
    /// hunting. A tactic the game named nothing falls back to the alert, and
    /// work outranks both.
    #[test]
    fn what_an_actor_is_doing_is_written_in_the_games_words_before_its_alert() {
        let mut stage = Stage::new(NearbyPanel::new(Rect::new(0, 0, 30, 10)).titled("")).screen(30, 10);
        stage.actor("rat", 'r', 2, 0);
        stage.tick();
        let palette = stage.app.world().resource::<Palette>().clone();
        let layout = NearbyPanel::new(Rect::new(0, 0, 30, 10)).activities(ActivityWords::new().word("flee_when_hurt", "fleeing")).0;
        let say = |doing: Option<&'static str>, work: bool| {
            let mut row = stage.app.world().resource::<NearbyView>().actors[0].clone();
            row.alert = Some(Alert::Hunting);
            row.doing = doing;
            row.work = work.then(|| crate::view::WorkRow { doing: "mending".into(), target: None, left: 3 });
            let mut terminal = Terminal::new(30, 1, bevy::math::Vec2::ONE);
            draw_row(&mut terminal, Rect::new(0, 0, 30, 1), &row, false, &Beneath::none(), &layout, &palette);
            (0..30).filter_map(|x| terminal.get(x, 0).map(|c| c.glyph)).collect::<String>()
        };
        assert!(say(Some("flee_when_hurt"), false).contains("rat (fleeing)"), "{:?}", say(Some("flee_when_hurt"), false));
        assert!(say(Some("hunt"), false).contains("rat (hunting)"), "a tactic the game named nothing reads as the alert");
        assert!(say(Some("flee_when_hurt"), true).contains("rat (mending)"), "and work outranks both");
    }

    #[test]
    fn what_an_actor_is_working_at_is_written_where_its_alert_would_be() {
        let mut stage = Stage::new(NearbyPanel::new(Rect::new(0, 0, 24, 10)).titled("")).screen(24, 10);
        stage.actor("drone", 'u', 2, 0);
        stage.tick();
        let mut row = stage.app.world().resource::<NearbyView>().actors[0].clone();
        row.alert = Some(Alert::Hunting);
        row.work = Some(crate::view::WorkRow { doing: "mending".into(), target: None, left: 3 });
        let palette = stage.app.world().resource::<Palette>().clone();
        let mut terminal = Terminal::new(24, 1, bevy::math::Vec2::ONE);
        draw_row(&mut terminal, Rect::new(0, 0, 24, 1), &row, false, &Beneath::none(), &NearbyPanel::new(Rect::new(0, 0, 24, 10)).0, &palette);
        let said: String = (0..24).filter_map(|x| terminal.get(x, 0).map(|c| c.glyph)).collect();
        assert!(said.contains("drone (mending)"), "{said:?}");
        assert!(!said.contains("hunting"), "what it is busy with, not what it knows: {said:?}");
    }

    #[test]
    fn a_facet_is_printed_after_the_name_and_a_long_row_is_clipped_not_wrapped() {
        let mut stage = Stage::new(NearbyPanel::new(Rect::new(0, 0, 24, 10)).titled("")).screen(24, 10);
        stage.actor("crab", 'c', 2, 0);
        stage.tick();
        stage.app.add_systems(
            Update,
            (|mut view: ResMut<NearbyView>, mut facets: ResMut<crate::Facets>| {
                for row in view.rows_mut() {
                    row.facets.push(facets.facet("wielding", "a very long cutlass indeed"));
                }
            })
            .in_set(crate::ViewSet::Annotate),
        );
        stage.tick();

        let row = stage.rows()[2].clone();
        assert!(row.contains("crab \u{00b7} a very"), "the facet follows the name: {row:?}");
        assert!(row.chars().count() <= 24, "clipped to the panel, never wrapped: {row:?}");
        assert!(row.contains('\u{2026}'), "and says it was clipped: {row:?}");
    }

    /// A name too long for the row gives up its end before what the thing
    /// is doing does: "Grubnak the… (unaware)", never "(unawar…".
    #[test]
    fn a_long_name_is_clipped_and_the_state_after_it_is_kept_whole() {
        let mut stage = Stage::new(NearbyPanel::new(Rect::new(0, 0, 24, 10)).titled("")).screen(24, 10);
        stage.actor("Grubnak the goblin king", 'g', 2, 0);
        stage.tick();
        let mut row = stage.app.world().resource::<NearbyView>().actors[0].clone();
        row.alert = Some(Alert::Unaware);
        let palette = stage.app.world().resource::<Palette>().clone();
        let mut terminal = Terminal::new(24, 1, bevy::math::Vec2::ONE);
        draw_row(&mut terminal, Rect::new(0, 0, 24, 1), &row, false, &Beneath::none(), &NearbyPanel::new(Rect::new(0, 0, 24, 10)).0, &palette);
        let said: String = (0..24).filter_map(|x| terminal.get(x, 0).map(|c| c.glyph)).collect();
        assert_eq!(said.trim_end(), "g Grubnak the\u{2026} (unaware)", "the name clipped, the state whole");

        // A row too narrow to keep four of the name's own characters
        // beside the state is clipped from the end as before, so the
        // name is never lost to the word after it.
        let mut terminal = Terminal::new(14, 1, bevy::math::Vec2::ONE);
        draw_row(&mut terminal, Rect::new(0, 0, 14, 1), &row, false, &Beneath::none(), &NearbyPanel::new(Rect::new(0, 0, 14, 10)).0, &palette);
        let said: String = (0..14).filter_map(|x| terminal.get(x, 0).map(|c| c.glyph)).collect();
        assert_eq!(said.trim_end(), "g Grubnak the\u{2026}", "too narrow for both, the name comes first");
    }

    /// Pushes what each actor holds under `kit`, and a mark after its name.
    fn kit_and_mark(mut view: ResMut<NearbyView>, mut facets: ResMut<crate::Facets>) {
        for row in view.rows_mut() {
            row.facets.push(facets.facet("mark", "marked"));
            row.facets.push(facets.facet("kit", "wields a long spear with a hooked head").toned(Tones::MUTED));
        }
    }

    /// A facet the layout puts beneath its row is wrapped whole under the
    /// name, where the rail has room for every row's so, on the row's own
    /// background, so the row picked out is picked out notes and all; the
    /// facet it does not stays after the name.
    #[test]
    fn a_facet_put_beneath_is_wrapped_under_the_name_and_picked_out_with_its_row() {
        let mut stage = Stage::new(NearbyPanel::new(Rect::new(0, 0, 24, 10)).titled("").beneath("kit")).screen(24, 10);
        stage.app.add_systems(Update, kit_and_mark.in_set(crate::ViewSet::Annotate));
        stage.actor("crab", 'c', 2, 0);
        stage.actor("gull", 'g', 3, 0);
        stage.tick();
        let rows = stage.rows();
        let lines = [
            "c crab \u{00b7} marked",
            "  wields a long spear",
            "  with a hooked head",
            "g gull \u{00b7} marked",
            "  wields a long spear",
            "  with a hooked head",
        ];
        assert_eq!(&rows[2..8], lines, "{rows:?}");
        let palette = stage.app.world().resource::<Palette>().clone();
        assert_eq!(stage.app.world().resource::<Terminal>().get(2, 3).map(|c| c.fg), Some(palette.get(Tones::MUTED)), "in the facet's tone");

        stage.press(CursorKeys::default().next);
        stage.press(CursorKeys::default().next);
        for y in [5, 6, 7] {
            assert_ne!(bg(&stage, 23, y), palette.get(Tones::SURFACE), "the gull's row and notes are picked out at row {y}");
        }
        assert_eq!(bg(&stage, 23, 6), palette.get(Tones::SELECT), "its notes on the selection itself");
        assert_eq!(bg(&stage, 23, 3), palette.get(Tones::SURFACE), "and the crab's are not");
    }

    /// With no room to wrap every row's notes, each row keeps one clipped
    /// line of them, and the row picked out below what fits scrolls into
    /// view with its line.
    #[test]
    fn with_no_room_to_wrap_every_rows_notes_each_keeps_one_clipped_line_and_scrolls_with_it() {
        let mut stage = Stage::new(NearbyPanel::new(Rect::new(0, 0, 24, 6)).titled("").beneath("kit")).screen(24, 6);
        stage.app.add_systems(Update, kit_and_mark.in_set(crate::ViewSet::Annotate));
        stage.actor("crab", 'c', 2, 0);
        stage.actor("gull", 'g', 3, 0);
        stage.actor("newt", 'n', 4, 0);
        stage.tick();
        let rows = stage.rows();
        assert_eq!(&rows[2..6], ["c crab \u{00b7} marked", "  wields a long spear w\u{2026}", "g gull \u{00b7} marked", "  wields a long spear w\u{2026}"]);
        for _ in 0..3 {
            stage.press(CursorKeys::default().next);
        }
        let rows = stage.rows();
        assert_eq!(&rows[2..6], ["g gull \u{00b7} marked", "  wields a long spear w\u{2026}", "n newt \u{00b7} marked", "  wields a long spear w\u{2026}"]);
    }

    #[test]
    fn an_empty_rail_draws_no_headings_at_all() {
        let stage = Stage::new(NearbyPanel::new(Rect::new(0, 0, 24, 10)).titled("")).screen(24, 10);
        assert!(stage.rows().iter().all(|r| r.is_empty()), "nothing in sight is nothing drawn: {:?}", stage.rows());
    }

    /// The background of the terminal cell at `x, y`.
    fn bg(stage: &Stage, x: i32, y: i32) -> Color {
        stage.app.world().resource::<Terminal>().get(x, y).expect("on the screen").bg
    }

    #[test]
    fn the_row_picked_out_is_drawn_on_the_selection_bar_and_all_and_so_is_its_tile() {
        let mut stage = Stage::new_with(NearbyPanel::new(Rect::new(40, 0, 24, 10)).titled(""), |app| {
            app.add_plugins(rl_render::MapViewPlugin::new(Rect::new(0, 0, 40, 20)));
        })
        .screen(64, 20);
        stage.actor("crab", 'c', 2, 0);
        stage.actor("gull", 'g', 3, 0);
        stage.tick();
        let (select, surface) = {
            let palette = stage.app.world().resource::<Palette>();
            (palette.get(Tones::SELECT), palette.get(Tones::SURFACE))
        };
        let tile = stage.app.world().resource::<MapView>().to_screen(stage.at.offset(3, 0)).expect("the gull is on the map");
        // Unpicked and unhurt, so the row carries its own health wash
        // rather than the selection.
        let unpicked = surface.mix(&stage.app.world().resource::<Palette>().get(Tones::GOOD), WASH);
        assert_eq!(bg(&stage, 45, 3), unpicked, "nothing is picked out yet");

        stage.press(CursorKeys::default().next);
        stage.press(CursorKeys::default().next);
        assert!(stage.rows()[3].contains("g gull"), "{:?}", stage.rows()[3]);
        // Picked out and unhurt: the selection is what the health washes
        // over, so the row reads as picked whatever its health.
        let picked = select.mix(&stage.app.world().resource::<Palette>().get(Tones::GOOD), WASH);
        for x in [40, 45, 63] {
            assert_eq!(bg(&stage, x, 3), picked, "the gull's row, edge to edge, at column {x}");
        }
        assert_eq!(bg(&stage, 45, 2), unpicked, "and not the crab's");
        assert_eq!(bg(&stage, tile.x, tile.y), select, "the gull's tile on the map");
        assert_eq!(stage.app.world().resource::<Terminal>().get(tile.x, tile.y).map(|c| c.glyph), Some('g'), "still showing the gull");

        stage.press(CursorKeys::default().close);
        assert_eq!(bg(&stage, 45, 3), unpicked, "let go, and back to its own health wash");
        assert_ne!(bg(&stage, tile.x, tile.y), select);
    }

    #[test]
    fn a_row_picked_out_below_the_rail_scrolls_the_list_to_show_it() {
        let mut stage = Stage::new(NearbyPanel::new(Rect::new(0, 0, 24, 5)).titled("")).screen(24, 5);
        for (i, name) in ["ant", "bee", "cat", "dog", "eel"].into_iter().enumerate() {
            stage.actor(name, name.chars().next().unwrap(), i as i32 + 1, 0);
        }
        stage.tick();
        assert!(stage.rows()[4].starts_with("c cat"), "three rows fit: {:?}", stage.rows());
        for _ in 0..5 {
            stage.press(CursorKeys::default().next);
        }
        let rows = stage.rows();
        assert!(rows[4].starts_with("e eel"), "the eel is shown last: {rows:?}");
        assert!(rows[2].starts_with("c cat"), "and the list starts late enough: {rows:?}");
    }
}
