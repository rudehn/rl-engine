//! Props: what stands on a map that is neither an actor nor an item.
//!
//! A crate, a pressure plate, a lever, a barrel, a wreck. What a prop is,
//! is a cell, a name and a glyph; everything else here is optional and
//! each part is a separate thing a game may leave out.
//!
//! This is the content half, so nothing here runs: [`PropDef`] is what a
//! `props.ron` says, with names resolved against whatever registries the
//! game lends the load, and [`load`] reports every problem in a file at
//! once rather than the first. What a prop does with what is written here
//! belongs to the Bevy layer, and what any of it means belongs to the
//! game.
//!
//! Three things are deliberately left as names rather than resolved to
//! ids. A verb and a trigger's moment are strings until the engine interns
//! them, the way a sound is, because the id only exists once there is a
//! run. A container's contents
//! are item names because items are a game's own registry and only the
//! game can spawn one; the engine rolls the counts and asks.

use rl_core::Id;
use rl_grid::Rgb;
use serde::Deserialize;

use crate::TagId;
use crate::ability::{EffectSpec, RawValue, TriggerSpec, parse_args};
use crate::content::{ContentError, Named, Registry};
use crate::names::Names;

/// A kind of prop, as the engine reads it.
#[derive(Debug, Clone)]
pub struct PropDef {
    /// The name content refers to it by, and the name a panel shows.
    pub name: String,
    /// How it looks.
    pub look: Look,
    /// Whether it stands in the way. Sight is another question, and one
    /// the engine does not answer yet: see `docs/design/props.md`.
    pub blocks: bool,
    /// What it takes to break it. `None` for a prop nothing can break,
    /// which is most of them.
    pub health: Option<i32>,
    /// What it offers whoever stands beside it or on it, in the order a
    /// list shows them.
    pub offers: Vec<OfferDef>,
    /// What it holds, when it holds anything.
    pub container: Option<ContainerDef>,
    /// What it does at the moments it answers: somebody stepping on it,
    /// its being broken, or a moment a game reports, written as any trigger
    /// is and resolved against the moments the Bevy layer interns.
    pub triggers: Vec<TriggerSpec>,
    /// How hard it is to spot. `None` for a prop in plain sight.
    pub hidden: Option<HiddenDef>,
}

impl Named for PropDef {
    fn name(&self) -> &str {
        &self.name
    }
}

/// A registered prop id.
pub type PropId = Id<PropDef>;

/// How a prop is drawn: content, like a monster's glyph, so it lives with
/// the definition rather than in a renderer's table of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct Look {
    /// The glyph.
    pub glyph: char,
    /// Its colour.
    pub color: Rgb,
    /// Draw order among entities on one cell; higher wins. A body lies
    /// under what is dropped on it, and both lie under whoever walks over
    /// them.
    #[serde(default = "one")]
    pub layer: i32,
}

/// One thing a prop offers to do for whoever reaches it.
///
/// The verb is what the engine answers with and what a game filters on,
/// so two props offering `open` offer the same thing. Effects are what it
/// does when data can say it; a prop whose offer has none is answered by
/// the game, which reads the verb off the message the interaction writes.
#[derive(Debug, Clone)]
pub struct OfferDef {
    /// What it is called: `open`, `search`, or a game's own.
    pub verb: String,
    /// What the turn costs, in hundredths of a step, like every other
    /// clock in the engine.
    pub time: u32,
    /// A tag the actor must carry, or the offer is refused and says so.
    pub needs: Option<TagId>,
    /// What lands when it resolves, in order. Empty for an offer only the
    /// game can answer.
    pub effects: Vec<EffectSpec>,
}

/// What a container holds, and what it takes to open it.
#[derive(Debug, Clone)]
pub struct ContainerDef {
    /// What is in it: an item's name, and how many, rolled when it is
    /// placed. The names stay names because items are a game's own
    /// registry, and only the game can spawn one.
    pub contents: Vec<ContentRoll>,
    /// A tag the actor must carry to open it at all.
    pub locked: Option<TagId>,
    /// The look it takes once it has been emptied, so a crate already
    /// done reads as done. `None` keeps its look and its offer, and the
    /// screen says it is empty.
    pub opened: Option<Look>,
}

/// How many of one item a container holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentRoll {
    /// The item's name, in the game's own registry.
    pub item: String,
    /// The fewest.
    pub min: u32,
    /// The most.
    pub max: u32,
}

/// How hard a prop is to spot, for one that is not in plain sight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HiddenDef {
    /// Percent chance a turn to spot it while it is in sight.
    pub spot: u8,
}

/// Loads props from RON, resolving every name through `names`.
///
/// Reports every problem in the file at once rather than the first, so a
/// content file with three typos names three.
pub fn load(text: &str, names: &Names<'_>) -> Result<Registry<PropDef>, ContentError> {
    let authored: Registry<Authored> = Registry::from_ron_str(text)?;
    let mut errors = Vec::new();
    let mut defs = Vec::new();
    for (_, a) in authored.iter() {
        let mut offers = Vec::new();
        for o in &a.offers {
            if o.verb.trim().is_empty() {
                errors.push(format!("{}: an offer with no verb is an offer nothing can ask for", a.name));
            }
            if o.time == 0 {
                errors.push(format!("{}: offer {:?} costs no time, so it could be repeated forever", a.name, o.verb));
            }
            let needs = match o.needs.as_deref().map(|tag| names.tag(tag)) {
                Some(Ok(tag)) => Some(tag),
                Some(Err(e)) => {
                    errors.push(format!("{}: offer {:?} needs {e}", a.name, o.verb));
                    None
                }
                None => None,
            };
            offers.push(OfferDef { verb: o.verb.clone(), time: o.time, needs, effects: read_effects(&a.name, &o.effects, &mut errors) });
        }
        let container = a.container.as_ref().map(|c| {
            for ContentRon(item, min, max) in &c.contents {
                if min > max {
                    errors.push(format!("{}: {item:?} is written as {min} to {max}, which is no range at all", a.name));
                }
            }
            let locked = match c.locked.as_deref().map(|tag| names.tag(tag)) {
                Some(Ok(tag)) => Some(tag),
                Some(Err(e)) => {
                    errors.push(format!("{}: its lock needs {e}", a.name));
                    None
                }
                None => None,
            };
            ContainerDef {
                contents: c.contents.iter().map(|ContentRon(item, min, max)| ContentRoll { item: item.clone(), min: *min, max: *max }).collect(),
                locked,
                opened: c.opened,
            }
        });
        for t in &a.triggers {
            if t.fires == Some(0) {
                errors.push(format!("{}: a trigger that fires no times never fires; leave it out instead", a.name));
            }
        }
        if let Some(h) = a.hidden
            && h.spot > 100
        {
            errors.push(format!("{}: it is spotted on a roll of {} percent, above 100", a.name, h.spot));
        }
        if a.container.is_some() && !a.offers.iter().any(|o| o.verb == OPEN) {
            errors.push(format!("{}: it holds things and offers no way to {OPEN} it", a.name));
        }
        defs.push(PropDef {
            name: a.name.clone(),
            look: Look { glyph: a.glyph, color: a.color, layer: a.layer },
            blocks: a.blocks,
            health: a.health,
            offers,
            container,
            triggers: a.triggers.clone(),
            hidden: a.hidden.map(|h| HiddenDef { spot: h.spot }),
        });
    }
    if !errors.is_empty() {
        return Err(ContentError::Invalid(errors));
    }
    Registry::from_defs(defs)
}

/// The verb for opening what holds something. The engine's own, so that
/// two games spell it the same way and the engine can say there is
/// nothing here to open.
pub const OPEN: &str = "open";

/// The verb for going through what is left of something. The engine's
/// own for the same reason, and what a body offers.
pub const SEARCH: &str = "search";

/// Reads a list of authored effects, reporting each problem under `what`.
fn read_effects(what: &str, authored: &[EffectRon], errors: &mut Vec<String>) -> Vec<EffectSpec> {
    let mut effects = Vec::new();
    for e in authored {
        if e.chance > 100 {
            errors.push(format!("{what}: effect {:?} has a chance of {}, above 100", e.kind, e.chance));
        }
        match parse_args(e.args.get_ron()) {
            Ok(args) => effects.push(EffectSpec { kind: e.kind.clone(), chance: e.chance, args }),
            Err(err) => errors.push(format!("{what}: effect {:?} arguments: {err}", e.kind)),
        }
    }
    effects
}

/// A prop as authored, before its names are resolved.
///
/// Unknown fields are refused: a misspelt or renamed one would otherwise
/// load as a prop without it, and a plate whose trigger was dropped that
/// way is a trap that silently does nothing.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Authored {
    name: String,
    glyph: char,
    color: Rgb,
    #[serde(default = "one")]
    layer: i32,
    #[serde(default)]
    blocks: bool,
    #[serde(default)]
    health: Option<i32>,
    #[serde(default)]
    offers: Vec<OfferRon>,
    #[serde(default)]
    container: Option<ContainerRon>,
    #[serde(default)]
    triggers: Vec<TriggerSpec>,
    #[serde(default)]
    hidden: Option<HiddenRon>,
}

impl Named for Authored {
    fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Debug, Deserialize)]
struct OfferRon {
    verb: String,
    #[serde(default = "one_step")]
    time: u32,
    #[serde(default)]
    needs: Option<String>,
    #[serde(default)]
    effects: Vec<EffectRon>,
}

#[derive(Debug, Deserialize)]
struct ContainerRon {
    #[serde(default)]
    contents: Vec<ContentRon>,
    #[serde(default)]
    locked: Option<String>,
    #[serde(default)]
    opened: Option<Look>,
}

/// What a container holds, as authored: `("slug", 8, 12)`, the way a
/// monster's drops are written.
#[derive(Debug, Deserialize)]
struct ContentRon(String, u32, u32);

#[derive(Debug, Clone, Copy, Deserialize)]
struct HiddenRon {
    spot: u8,
}

#[derive(Debug, Deserialize)]
struct EffectRon {
    kind: String,
    #[serde(default = "hundred")]
    chance: u8,
    #[serde(default = "nothing")]
    args: Box<RawValue>,
}

fn one() -> i32 {
    1
}

fn one_step() -> u32 {
    rl_core::turn::BASE_ACTION_COST
}

fn hundred() -> u8 {
    100
}

fn nothing() -> Box<RawValue> {
    parse_args("()").expect("the unit is valid ron")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TagDef;

    fn tags() -> Registry<TagDef> {
        Registry::from_defs(vec![TagDef::new("cutter")]).expect("one tag")
    }

    /// The file in `docs/design/props.md`, near enough: a crate that
    /// holds things, a plate that is hidden and fires once, and a console
    /// whose verb only the game understands.
    #[test]
    fn a_props_file_loads_with_every_optional_part_left_out_of_something() {
        let tags = tags();
        let names = Names::new().tags(&tags);
        let props = load(
            r#"#![enable(implicit_some)]
            [
                (name: "supply crate", glyph: '&', color: (r: 190, g: 165, b: 115), blocks: true, health: 6,
                 container: (contents: [("slug", 8, 12)], opened: (glyph: '"', color: (r: 128, g: 115, b: 90))),
                 offers: [(verb: "open", time: 200)]),
                (name: "locked cache", glyph: '&', color: (r: 204, g: 204, b: 217), blocks: true,
                 container: (contents: [("composite plate", 1, 1)], locked: "cutter"),
                 offers: [(verb: "open", time: 300)]),
                (name: "fuel-line plate", glyph: '^', color: (r: 230, g: 140, b: 51),
                 hidden: (spot: 40),
                 triggers: [(on: "entered", fires: 1, effects: [(kind: "Emit", args: (gas: "fuel vapour", amount: 90))])]),
                (name: "reactor console", glyph: '%', color: (r: 89, g: 217, b: 230), blocks: true,
                 offers: [(verb: "charge", time: 300)]),
            ]"#,
            &names,
        )
        .expect("the file loads");

        let crate_def = props.get(props.expect("supply crate"));
        assert_eq!(crate_def.look.layer, 1, "a layer nobody wrote is one");
        assert_eq!(crate_def.health, Some(6));
        assert_eq!(crate_def.offers[0].time, 200);
        assert_eq!(crate_def.container.as_ref().expect("it holds things").contents[0], ContentRoll { item: "slug".into(), min: 8, max: 12 });
        assert!(crate_def.container.as_ref().expect("it holds things").locked.is_none(), "an unlocked crate needs nothing");

        let cache = props.get(props.expect("locked cache"));
        assert_eq!(cache.container.as_ref().and_then(|c| c.locked), Some(tags.expect("cutter")), "the lock resolved to the tag");

        let plate = props.get(props.expect("fuel-line plate"));
        assert!(!plate.blocks, "a plate nobody wrote as blocking does not block");
        assert_eq!(plate.hidden.map(|h| h.spot), Some(40));
        let trigger = plate.triggers.first().expect("it fires");
        assert_eq!((trigger.on.as_str(), trigger.fires, trigger.effects.as_ref().map(Vec::len)), ("entered", Some(1), Some(1)));

        let console = props.get(props.expect("reactor console"));
        assert!(console.offers[0].effects.is_empty(), "a verb only the game answers carries nothing");
        assert_eq!(console.offers[0].verb, "charge");
    }

    /// A props file written for 0.3.0, with `trigger:` where `triggers:`
    /// now goes, is refused naming the field, rather than loading a plate
    /// with no trigger at all, which is a trap that silently does nothing.
    #[test]
    fn a_leftover_trigger_field_is_refused_rather_than_loading_a_plate_that_does_nothing() {
        let tags = tags();
        let names = Names::new().tags(&tags);
        let err = load(
            r#"#![enable(implicit_some)]
            [
                (name: "old plate", glyph: '^', color: (r: 1, g: 2, b: 3),
                 trigger: (on: Entered, fires: 1, effects: [])),
            ]"#,
            &names,
        )
        .expect_err("an old field is refused");
        let said = format!("{err}");
        assert!(said.contains("trigger") && said.contains("triggers"), "names the field and the one it became: {said}");
    }

    /// One load, every complaint: a file with five mistakes names five,
    /// so a content author fixes them in one pass rather than five.
    #[test]
    fn every_problem_in_a_file_is_reported_at_once() {
        let tags = tags();
        let names = Names::new().tags(&tags);
        let err = load(
            r#"#![enable(implicit_some)]
            [
                (name: "bad crate", glyph: '&', color: (r: 1, g: 2, b: 3),
                 container: (contents: [("slug", 9, 4)], locked: "skeleton key")),
                (name: "bad plate", glyph: '^', color: (r: 1, g: 2, b: 3),
                 hidden: (spot: 140),
                 triggers: [(on: "entered", fires: 0, effects: [])]),
                (name: "bad lever", glyph: '\\', color: (r: 1, g: 2, b: 3),
                 offers: [(verb: "pull", time: 0)]),
            ]"#,
            &names,
        )
        .expect_err("a file this wrong does not load");

        let said = format!("{err}");
        for want in [
            "9 to 4",           // a range that is no range
            "skeleton key",     // a tag nothing registered
            "above 100",        // a spotting roll that cannot fail
            "never fires",      // a trigger that cannot fire
            "repeated forever", // an offer that costs no time
            "no way to open",   // a container nothing can open
        ] {
            assert!(said.contains(want), "every problem is named at once, and {want:?} is missing from: {said}");
        }
    }
}
