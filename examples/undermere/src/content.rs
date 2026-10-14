//! What Undermere's world is made of: its sides, its kinds of harm, its
//! stats, statuses, slots and tags, the props that stand in its places,
//! the abilities its monsters know, the rules a run reads its stats by,
//! and the order every content file is read in.
//!
//! Every name here is content the engine knows nothing about; the engine
//! sees ids. The registries are filled first, since every file names
//! them, and [`Library::load`] reads the files after them in the order
//! their names need: abilities, then the traits that grant them, then the
//! items that lend traits, then the monsters that are written as traits
//! and drop items, and last the faces the things of a kind wear until
//! they are known.
//!
//! The stats are the run's numbers that things change: [`STRENGTH`],
//! which heavy gear asks for, [`ACCURACY`] and [`EVASION`], which the hit
//! model weighs against each other, [`SPEED`], [`STEALTH`], [`MENDING`],
//! [`LIGHT`] and [`ARMOR`], and the four the rings give besides,
//! [`REGENERATION`], [`WISDOM`], [`REAPING`] and [`AWARENESS`]. Each is
//! read where the engine or the game already reads the number it changes:
//! [`combat_rules`] names four of them to the engine's blows and heals,
//! [`turn_rules`] names the speed to the turn loop, and the rest are
//! Undermere's own to read.

use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_rules::faction::FactionDef;

use crate::armory::Armory;
use crate::bestiary::Bestiary;
use crate::traits::TraitDef;

/// The abilities file, compiled in so the binary runs from anywhere.
const ABILITIES_RON: &str = include_str!("../assets/abilities.ron");
/// The props file, compiled in beside it.
const PROPS_RON: &str = include_str!("../assets/props.ron");

/// How bright every light Undermere's files write is at its heart, out of
/// 255, and the brightness the lantern is lit at again once a darkness
/// lifts.
///
/// One brightness for every light, so how far a light reaches is what
/// tells how bright it is: a lantern and a glowing cap differ by their
/// radius and their colour, and the lantern dimming as the adventurer
/// descends is its radius shrinking. 200 is what Foundry's shoulder lamp
/// sheds too: the light an example hands whoever carries it. Every file
/// writes it on every light, as the engine's light is written, and a test
/// holds them to it.
pub const LIGHT_INTENSITY: u8 = 200;

/// What heavy gear asks of its wearer: the adventurer's is 12, and a
/// potion of strength raises it by one for good.
pub const STRENGTH: &str = "strength";
/// Out of a hundred, before a target's evasion is taken off: what the hit
/// model reads as the chance a blow or a shot lands.
pub const ACCURACY: &str = "accuracy";
/// Taken off an attacker's accuracy: a shield's, and nothing else's yet.
pub const EVASION: &str = "evasion";
/// A percentage of an actor's own speed added to it wherever the turn
/// loop reads it: haste is a hundred, a chill minus twenty-five.
pub const SPEED: &str = "speed";
/// Percentage points off a watcher's chance of noticing the adventurer.
pub const STEALTH: &str = "stealth";
/// A percentage added to every heal its holder is given.
pub const MENDING: &str = "mending";
/// Tiles added to the reach of the adventurer's lantern.
pub const LIGHT: &str = "light";
/// Added to whatever armor a defender and its gear already have: what
/// gear too heavy to carry well takes away.
pub const ARMOR: &str = "armor";
/// How many steps of loudness each of an actor's steps carries over open
/// ground: what heavy armor adds to.
pub const FOOTFALL: &str = "footfall";
/// Health regained a hundred turns, a point at a time: what a ring of
/// regeneration gives, since nothing else under the lake heals by itself.
pub const REGENERATION: &str = "regeneration";
/// Percent faster every staff its holder carries refills: a ring of
/// wisdom's.
pub const WISDOM: &str = "wisdom";
/// Turns taken off every carried staff's next charge by each blow its
/// holder lands: a ring of reaping's.
pub const REAPING: &str = "reaping";
/// Percentage points added to the chance a turn of spotting what is hidden
/// in sight: a ring of awareness's.
pub const AWARENESS: &str = "awareness";
/// Taken off the stealth engine's own `quiet`, so a value below nought
/// widens every watcher's certain notice of its holder: the Crown of
/// Grubnak's own gleam, which nothing else under the lake bestows.
pub const QUIET: &str = "quiet";
/// Added to the roll of every blow and shot its holder makes, read by
/// `CombatRules::attack`: nothing bestows it, since a weapon's own roll is
/// still its own, but the Crown of Grubnak's aura puts it on an ally in
/// sight for as long as they stay there.
pub const ATTACK: &str = "attack";

/// A potion of haste's: its holder's speed doubled.
pub const HASTED: &str = "hasted";
/// A potion of fire immunity's: fire does nothing to its holder.
pub const FIREPROOF: &str = "fireproof";
/// A potion of darkness's: its holder's lantern out, and thirty points of
/// stealth.
pub const SHROUDED: &str = "shrouded";
/// A paralytic cloud's: every turn its holder has is lost.
pub const PARALYSED: &str = "paralysed";
/// A confusing cloud's: every step its holder takes goes a random way.
pub const CONFUSED: &str = "confused";
/// A scroll of discord's, and the side its holder fights for while it
/// lasts, at odds with every side and with itself.
pub const DISCORDANT: &str = "discordant";
/// The run's captive: a shackled monster of the floor while it is still
/// shackled, neutral to every side, the adventurer's captors among them,
/// until it is freed and put on the adventurer's own.
pub const CAPTIVE: &str = "captive";
/// Part 5's keepers and the Mereholt crowd around them: at peace with
/// everyone, the adventurer included, and excluded from the discord loop
/// in [`combat_rules`] rather than merely unnamed in it, so nobody is ever
/// hostile to it, not even a victim of the scroll. Doing without the harm
/// the town's peace already drops, as its build marks it peaceful, would
/// still leave a discordant townsperson swinging at a neighbour, which the
/// design rules out by the faction alone.
pub const TOWNSFOLK: &str = "townsfolk";

/// The small life of the place: a moth, a blind cave rat, neutral to
/// every side and never worth fighting, which `combat_rules` leaves off
/// every `hostile` call for the same reason it leaves off [`CAPTIVE`].
pub const WILDLIFE: &str = "wildlife";
/// The cloud a potion of paralysis bursts into.
pub const PARALYTIC: &str = "paralytic";
/// The cloud a caustic vent breathes out, a risk-and-reward piece's own.
pub const CAUSTIC: &str = "caustic";
/// The cloud a potion of confusion bursts into.
pub const CONFUSING: &str = "confusing";
/// The cloud a potion of darkness gives off: it shrouds whoever breathes
/// it, and hides what is behind its thickest.
pub const DARKNESS: &str = "darkness";
/// A spider's web, a necromancer's bolt: holds its holder as `paralysed`
/// does, for a trait to inflict rather than a potion.
pub const WEBBED: &str = "webbed";
/// A goblin mystic's ward: three more armor while it lasts.
pub const WARDED: &str = "warded";
/// A wraith's touch: a point off strength, stacked by every touch that
/// lands, so the heaviest gear grows heavier to carry the longer a fight
/// runs.
pub const WEAKENED: &str = "weakened";
/// Part 2's engine `sleeps`, named for Undermere's own placement and its
/// `sleepy` and `wakeful` traits: holds its holder until a noise, a blow
/// or a notice wakes them, as every sleeper does.
pub const ASLEEP: &str = "asleep";

/// What each cloud tints the ground under it, `(r, g, b)` in `0..=1`, so a
/// paralytic cloud is told from a confusing one at a glance rather than
/// both drawn in the engine's grey: a sickly violet, a churning teal, and
/// a dark the colour of the lake at night.
pub const CLOUD_TINTS: [(&str, (f32, f32, f32)); 3] = [(PARALYTIC, (0.78, 0.45, 0.9)), (CONFUSING, (0.35, 0.85, 0.7)), (DARKNESS, (0.06, 0.06, 0.12))];

/// The slots that go in pairs, by name: a thing that fits `either` of a
/// pair is worn in whichever is free, a one-handed blade in either hand
/// and a ring on either hand's finger. Written onto the slot registry, the
/// first of each pair paired with the second, which is where the engine's
/// `Wear::shape` finds a pair.
pub const PAIRS: [(&str, &str); 2] = [("main hand", "off hand"), ("left finger", "right finger")];

/// Undermere's stats, each at the value an actor has with nothing on it.
///
/// One base for every actor, the adventurer's: a monster's strength is
/// never read, since only the adventurer pays for what is too heavy, and
/// every other stat starts where theirs does, accuracy at a hundred and
/// the rest at nought. Speed's base is nought, not a hundred, because the
/// turn loop adds it to an actor's own speed as a percentage of it: a base
/// of a hundred would have everyone move twice as fast. It is held between
/// a tenth of an actor's speed and four times it, so no stack of chills
/// stops a body dead. Footfall's base is the house's step,
/// [`NOISE`](crate::plugin::NOISE)'s, so a body with nothing on it steps
/// exactly as loud as it did before footfall was a stat. The four the
/// rings give, regeneration, wisdom, reaping and awareness, start at
/// nought, since only a ring gives any; so do quiet and attack, which
/// nothing but the Crown of Grubnak ever touches.
pub fn stats() -> Registry<StatDef> {
    Registry::from_defs(vec![
        StatDef::new(STRENGTH, 12),
        StatDef::new(ACCURACY, 100),
        StatDef::new(EVASION, 0),
        StatDef::new(SPEED, 0).clamp(-90, 300),
        StatDef::new(STEALTH, 0),
        StatDef::new(MENDING, 0),
        StatDef::new(LIGHT, 0),
        StatDef::new(ARMOR, 0),
        StatDef::new(FOOTFALL, crate::plugin::NOISE.step),
        StatDef::new(REGENERATION, 0),
        StatDef::new(WISDOM, 0),
        StatDef::new(REAPING, 0),
        StatDef::new(AWARENESS, 0),
        StatDef::new(QUIET, 0),
        StatDef::new(ATTACK, 0),
    ])
    .unwrap()
}

/// Every registry Undermere's content names, filled before anything is
/// loaded against them.
///
/// Assembled the way Foundry's is: one `Registry::from_defs` per table,
/// then the struct literal, the statuses after the kinds they deal and the
/// stats they change, and the props last, since a chest asks for what it
/// holds by tag. Panics naming `assets/props.ron` and every problem in
/// it, since a prop that does not load is a place that cannot be filled.
pub fn registries() -> Registries {
    // Armor turns a blow and nothing else. The elements go through plate,
    // since a tick of `poisoned` or `scorched` that armor could stop would
    // never tick at all, and a resistance is what answers them; so does a
    // staff's lightning, which nothing yet resists.
    let damage_kinds = Registry::from_defs(vec![
        DamageKind::new("slash"),
        DamageKind::new("pierce"),
        DamageKind::new("crush"),
        DamageKind::new("fire").unarmored(),
        DamageKind::new("cold").unarmored(),
        DamageKind::new("poison").unarmored(),
        DamageKind::new("lightning").unarmored(),
    ])
    .unwrap();
    let (fire, poison) = (damage_kinds.expect("fire"), damage_kinds.expect("poison"));
    let stats = stats();
    // `poisoned` is a point of poison a turn. `scorched` is what standing
    // in fire leaves, a point a turn for as long as the run's `FireRules`
    // says. `mending` is two a turn, for whatever content puts it on.
    // `regenerating` is what the trait that names it holds for life, a
    // point a turn, with no badge, since it is what the monster is rather
    // than something that happened to it. Both are boons, so the log tells
    // them as good news. `chilled` is the drowning grasp's, a quarter off
    // its holder's speed, which the turn loop reads through the speed
    // stat; `slowed`, a staff's, is half off it, and `hasted`, a potion's,
    // its own again. `fireproof` is a potion's hundred percent against
    // fire, which `conditions` keeps on its holder's resistances;
    // `shrouded` is darkness's thirty points of stealth, with the lantern
    // put out; `paralysed` holds its holder's turns; `confused` turns
    // every step it takes a random way; and `discordant`, a scroll's,
    // fights for the side of the same name while it lasts. `webbed` holds
    // as `paralysed` does, for a web rather than a cloud; `warded` is
    // three armor, a ward's gift, so a boon; `weakened` is a point off
    // strength, and stacks, a wraith's touch making the next heavier than
    // the last; `asleep` is the engine's own `sleeps`, named for
    // Undermere's placement and its `sleepy` and `wakeful` traits.
    let statuses = Registry::from_defs(vec![
        StatusDef { badge: Some('p'), ..StatusDef::new("poisoned").ticks(poison, 1) },
        StatusDef { badge: Some('*'), ..StatusDef::new("scorched").ticks(fire, 1) },
        StatusDef { badge: Some('+'), ..StatusDef::new("mending").mends(2).boon() },
        StatusDef::new("regenerating").mends(1).boon(),
        StatusDef { badge: Some('c'), ..StatusDef::new("chilled").modifies(stats.expect(SPEED), Op::Add(-25)) },
        StatusDef { badge: Some('s'), ..StatusDef::new("slowed").modifies(stats.expect(SPEED), Op::Add(-50)) },
        StatusDef { badge: Some('h'), ..StatusDef::new(HASTED).modifies(stats.expect(SPEED), Op::Add(100)).boon() },
        StatusDef { badge: Some('f'), ..StatusDef::new(FIREPROOF).boon() },
        StatusDef { badge: Some('d'), ..StatusDef::new(SHROUDED).modifies(stats.expect(STEALTH), Op::Add(30)).boon() },
        StatusDef { badge: Some('P'), ..StatusDef::new(PARALYSED).holds() },
        StatusDef { badge: Some('?'), ..StatusDef::new(CONFUSED) },
        StatusDef { badge: Some('!'), ..StatusDef::new(DISCORDANT) },
        StatusDef { badge: Some('b'), ..StatusDef::new(WEBBED).holds() },
        StatusDef { badge: Some('w'), ..StatusDef::new(WARDED).modifies(stats.expect(ARMOR), Op::Add(3)).boon() },
        StatusDef { badge: Some('-'), ..StatusDef::new(WEAKENED).modifies(stats.expect(STRENGTH), Op::Add(-1)).stacking(Stacking::Stack) },
        StatusDef { badge: Some('Z'), ..StatusDef::new(ASLEEP).sleeps() },
    ])
    .unwrap();
    // Each potion's cloud: what breathing it does, while it is thick
    // enough to, for a few turns more than that, so whoever stands in it is
    // held, turned about or hidden for as long as it hangs. Darkness is
    // thick enough to see through only at its edges.
    // ANCHOR: gases
    let gases = Registry::from_defs(vec![
        GasDef::new(PARALYTIC).spread(40).fade(20).inflicts(25, statuses.expect(PARALYSED), 3),
        GasDef::new(CONFUSING).spread(50).fade(15).inflicts(25, statuses.expect(CONFUSED), 3),
        GasDef::new(DARKNESS).spread(30).fade(8).veils_at(120).inflicts(25, statuses.expect(SHROUDED), 5),
        GasDef::new(CAUSTIC).spread(40).fade(20).inflicts(30, statuses.expect("poisoned"), 4),
    ])
    .unwrap();
    // ANCHOR_END: gases
    let mut registries = Registries {
        damage_kinds,
        stats,
        statuses,
        gases,
        factions: Registry::from_defs(
            ["adventurer", "beasts", "goblins", "undead", "dar", CAPTIVE, DISCORDANT, TOWNSFOLK, WILDLIFE].map(FactionDef::new).to_vec(),
        )
        .unwrap(),
        tags: tags(),
        slots: Registry::from_defs(
            ["main hand", "off hand", "head", "body", "left finger", "right finger", "light"]
                .map(|slot| match PAIRS.iter().find(|(one, _)| *one == slot) {
                    Some((_, other)) => SlotDef::new(slot).paired_with(*other),
                    None => SlotDef::new(slot),
                })
                .to_vec(),
        )
        .unwrap(),
        ..Registries::default()
    };
    registries.props = rl_engine::rl_rules::prop::load(PROPS_RON, &registries.names()).unwrap_or_else(|e| panic!("assets/props.ron: {e}"));
    registries
}

/// Who fights whom: the adventurer against every side, beasts and goblins
/// against each other, and the dead against everything living, so a
/// crypt's dead and a goblin band fight and the dungeon fights itself. The
/// dar fight only the adventurer, a deep culture of their own that neither
/// raids the goblins nor hunts the beasts. The discordant fight everyone,
/// each other among them, which is what a scroll of discord turns a foe
/// into for a while, Mereholt's [`TOWNSFOLK`] the one side left out of
/// that: nobody is ever hostile to it, so a townsperson never has to be
/// asked whether it fights, a scroll's victim included.
///
/// And which stats a blow and a heal read: accuracy against evasion for
/// whether it lands, the armor stat beside whatever armor is worn, mending
/// on every heal, and the attack stat added to every blow and shot's
/// roll. A blow on a foe that has not noticed its attacker lands at
/// [`UNAWARE`] percent, unless the weapon names its own.
///
/// The attack stat starts at nought for everyone and nothing in
/// `items.ron` bestows it: a blow's roll is still the weapon's own. It
/// exists for [`crate::artifacts::crown_aura`] to put on an ally in the
/// Crown of Grubnak's wearer's sight, the one bonus the content sheet
/// gives to somebody other than whoever wears the thing.
pub fn combat_rules(registries: &Registries) -> CombatRules {
    let [adventurer, beasts, goblins, undead, dar] = ["adventurer", "beasts", "goblins", "undead", "dar"].map(|name| registries.factions.expect(name));
    let discord = registries.factions.expect(DISCORDANT);
    let townsfolk = registries.factions.expect(TOWNSFOLK);
    let stat = |name: &str| registries.stats.expect(name);
    let mut rules = CombatRules::new(&registries.factions)
        .hostile(adventurer, beasts)
        .hostile(adventurer, goblins)
        .hostile(adventurer, undead)
        .hostile(adventurer, dar)
        .hostile(beasts, goblins)
        .hostile(undead, beasts)
        .hostile(undead, goblins);
    for (side, _) in registries.factions.iter() {
        if side != townsfolk {
            rules = rules.hostile(discord, side);
        }
    }
    rules
        .accuracy_stat(stat(ACCURACY))
        .evasion_stat(stat(EVASION))
        .armor_stat(stat(ARMOR))
        .mending_stat(stat(MENDING))
        .attack_stat(stat(ATTACK))
        .unaware(UNAWARE)
}

/// What a blow on a foe that has not noticed its attacker is worth, in
/// percent of its roll: three times, Brogue's sneak attack, which is what
/// creeping up unseen in the dark pays for. A dagger's own five outranks
/// it. Only the adventurer can be unnoticed, since only they carry a
/// stealth for a watcher to fail to notice.
pub const UNAWARE: u32 = 300;

/// How fast everyone is: their own speed, with the speed stat added as a
/// percentage of it, so a haste or a chill is a status changing a stat.
pub fn turn_rules(registries: &Registries) -> TurnRules {
    TurnRules::new().speed_stat(registries.stats.expect(SPEED))
}

/// Whether a blow or a shot lands: the engine's percent roll, accuracy
/// less evasion, so a blow that meets nothing in the way lands, a shield
/// turns one in ten, and every point of strength a weapon wants and its
/// wielder lacks is four in a hundred more misses.
///
/// A shot loses ten a tile past its weapon's effective reach, and thirty
/// at a target in the dark, which only dark sight could see it in. Nothing
/// for a dim one: every floor is dim beyond the lantern, so a cost for dim
/// light would be a cost on nearly every shot, and true dark is rare.
pub fn hit_rules() -> HitRules {
    HitRules(Box::new(rl_engine::rl_rules::accuracy::Percent::new(10, 0, 30)))
}

/// The tag a vault's lock asks for in `assets/pool.ron`, written
/// `Locked("vault key")`: every locked vault's, until its floor is built
/// and [`crate::vaults::place_vault_fixtures`] gives the lock its own
/// floor's [`vault_key`] in its place.
pub const VAULT_KEY: &str = "vault key";

/// The tag the key to `floor`'s vault carries and that vault's lock asks
/// for, so a key opens its own vault and no other: one a floor, since a
/// floor has one locked vault at most, and the pool draws one on any
/// floor from the first to the ninth.
pub fn vault_key(floor: u32) -> String {
    format!("{VAULT_KEY} {floor}")
}

/// Every tag: the kinds of thing a chest's row and a floor's finds are
/// drawn by, and the lock and the key of each floor's vault.
pub fn tags() -> Registry<TagDef> {
    let kinds = ["weapon", "armor", "potion", "scroll", "staff", "wand", "ring", "gold", "arrow", "light", "thrown", "artifact", "waystone"].map(String::from);
    let keys = std::iter::once(VAULT_KEY.to_string()).chain((1..crate::dungeon::FLOORS).map(vault_key));
    Registry::from_defs(kinds.into_iter().chain(keys).map(TagDef::new).collect()).expect("every tag has a name of its own")
}

/// The effect kinds an Undermere app has, for reading the content where no
/// app is running, as a test does: [`add_game_effects`]' list, the `Ignite`
/// that `FirePlugin` declares for the fire breath, a staff of firebolt and
/// a potion of incineration, and the `Emit` that `GasPlugin` declares for
/// the potions that burst into a cloud. The throwaway `App` is there so the
/// list is the engine's own and never one kept beside it.
pub fn effect_kinds() -> EffectKinds {
    let mut app = App::new();
    app.add_effect::<rl_engine::rl_bevy::Ignite>().add_effect::<rl_engine::rl_bevy::Emit>();
    add_game_effects(&mut app);
    std::mem::take(&mut app.world_mut().resource_mut::<EffectKinds>())
}

/// Declares every effect Undermere's content names but the two its plugins
/// declare: the engine's `Harm`, `Mend`, `Inflict`, `Cleanse`, `Shove` and
/// `Pull`, and the ones Undermere writes itself, which reach what the
/// engine knows nothing of: a thing's level, what is known of it, a curse,
/// the adventurer's strength, and what [`effects`](crate::effects) holds,
/// where someone stands, who they are, whose side they are on, what they
/// see, what their things hold and a body made whole; a waystone's call
/// home, in [`waystone`](crate::waystone); and
/// [`tricks`](crate::tricks) holds a
/// landed blow that steals and a touch that saps strength no faster than
/// three stacks. The one list, for the app and for [`effect_kinds`] alike.
///
/// The engine's one by one rather than by `add_engine_effects`, whose
/// `Teleport` moves its user to where a use landed: Undermere's scroll of
/// teleportation is its own `Teleport`, which sends its reader far off,
/// and two effects registered under one name would be the second quietly
/// taking the first's place.
pub fn add_game_effects(app: &mut App) {
    use crate::effects::{bodies, charges, mending, minds, moving, senses};
    use rl_engine::rl_bevy::{Cleanse, Harm, Inflict, Mend, Pull, Shove};
    app.add_effect::<Harm>().add_effect::<Mend>().add_effect::<Inflict>().add_effect::<Cleanse>().add_effect::<Shove>().add_effect::<Pull>();
    app.add_effect::<crate::enchant::Enchanting>()
        .add_effect::<crate::enchant::RemoveCurse>()
        .add_effect::<crate::lore::Identify>()
        .add_effect::<crate::lore::NameWorn>()
        .add_effect::<crate::strength::Strengthen>();
    app.add_effect::<mending::Whole>().add_effect::<mending::CureLasting>();
    app.add_effect::<moving::Blink>()
        .add_effect::<moving::TeleportOther>()
        .add_effect::<moving::Beckon>()
        .add_effect::<moving::Descend>()
        .add_effect::<moving::Fall>();
    app.add_effect::<moving::Teleport>();
    app.add_effect::<bodies::Polymorph>().add_effect::<bodies::Plenty>().add_effect::<bodies::Conjure>().add_effect::<bodies::Life>();
    app.add_effect::<bodies::Summon>().add_effect::<bodies::RaiseDead>();
    app.add_effect::<minds::Dominate>()
        .add_effect::<minds::Discord>()
        .add_effect::<minds::Aggravate>()
        .add_effect::<minds::WakeFloor>()
        .add_effect::<minds::Rouse>();
    app.add_effect::<senses::Darkness>().add_effect::<senses::MagicMapping>();
    app.add_effect::<charges::Recharge>();
    app.add_effect::<crate::tricks::Steal>().add_effect::<crate::tricks::Wither>();
    app.add_effect::<bodies::Rally>().add_effect::<bodies::CallDrowned>().add_effect::<bodies::Hatch>().add_effect::<bodies::Rise>();
    app.add_effect::<bodies::RaiseAlly>().add_effect::<bodies::Whisper>().add_effect::<minds::AlertDrowned>();
    app.add_effect::<crate::waystone::CallGate>();
}

/// Reads `abilities.ron` against `registries`, its effects built through
/// `kinds`. Panics naming the file and every problem in it, since a
/// broken ability is a game that cannot start.
pub fn abilities(registries: &Registries, kinds: &EffectKinds) -> Abilities {
    Abilities::load(ABILITIES_RON, kinds, &registries.names()).unwrap_or_else(|e| panic!("assets/abilities.ron: {e}"))
}

/// Everything Undermere's content files hold, read in the order their
/// names need.
///
/// The one place that order is written, so whatever wants the content
/// never has to know it: traits read before the abilities they grant
/// would find every grant an unknown name.
pub struct Library {
    /// The registries every file names.
    pub registries: Registries,
    /// The abilities, their effects built.
    pub abilities: Abilities,
    /// The traits monsters are written as and items lend.
    pub traits: Registry<TraitDef>,
    /// The items, and what each floor holds.
    pub armory: Armory,
    /// The monsters, and where each turns up.
    pub bestiary: Bestiary,
    /// The faces the things of a kind wear until the kind is known.
    pub faces: crate::lore::Faces,
    /// What Mereholt's keepers may stock.
    pub wares: crate::shops::Wares,
    /// What a sword for hire may carry, and what it asks.
    pub mercenaries: crate::mercenaries::Mercenaries,
    /// The rites the temple's priest offers, their effects built.
    pub rites: crate::temple::Rites,
    /// Mereholt's people, a row a role.
    pub townsfolk: crate::townsfolk::Townsfolk,
    /// The sets every name the run gives is drawn from.
    pub names: crate::townsfolk::NameSets,
    /// What the innkeeper may tell of a run's dungeon.
    pub rumours: crate::rumours::Rumours,
}

impl Library {
    /// Reads every file, with `kinds` and `moments` the ones the app
    /// registered, so an effect or a moment nobody registered fails here.
    /// Panics naming the file at the first one that is broken.
    pub fn load(kinds: &EffectKinds, moments: &Moments) -> Self {
        let registries = registries();
        let abilities = abilities(&registries, kinds);
        let traits = crate::traits::load_defs(&registries, abilities.defs());
        let armory = Armory::load(&registries, &traits, kinds, moments);
        let bestiary = Bestiary::load(&registries, &traits, &armory.defs, kinds, moments);
        let faces = crate::lore::Faces::load(&registries, &armory.defs);
        let wares = crate::shops::Wares::load(&registries, &armory);
        let mercenaries = crate::mercenaries::Mercenaries::load(&registries, &armory);
        let rites = crate::temple::Rites::load(&registries, kinds);
        // After the monsters, which a line waiting on one slain names.
        let townsfolk = crate::townsfolk::Townsfolk::load(&bestiary);
        let names = crate::townsfolk::NameSets::load();
        // After the townsfolk, whose roles a rumour's `{role}` names.
        let rumours = crate::rumours::Rumours::load(&townsfolk);
        Self { registries, abilities, traits, armory, bestiary, faces, wares, mercenaries, rites, townsfolk, names, rumours }
    }
}

#[cfg(test)]
mod tests {
    use rl_engine::rl_rules::Relation;

    use super::*;

    /// Every file in `assets/`, read the way the game reads it: if any
    /// name in any of them were unknown, or any entry malformed, loading
    /// would panic naming the file and the entry.
    #[test]
    fn every_content_file_loads_and_every_name_in_it_resolves() {
        let library = Library::load(&effect_kinds(), &Moments::default());
        assert!(library.abilities.defs().len() >= 2, "fire breath and drowning grasp, and part 3's own");
        assert!(library.traits.len() >= 19, "the skeleton's nineteen traits, and part 3's own");
        assert!(library.armory.defs.len() >= 18, "about twenty items: {}", library.armory.defs.len());
        assert!(library.bestiary.defs.len() >= 28, "about thirty monsters: {}", library.bestiary.defs.len());
        assert!(!library.armory.table.rows().is_empty(), "item_spawns.ron has rows");
        assert!(!library.bestiary.spawns.entries().is_empty(), "monster_spawns.ron has rows");
    }

    /// The registries hold every side, kind, status, slot and tag the
    /// files write, so a file naming one of them reads.
    #[test]
    fn the_registries_hold_every_side_kind_status_slot_and_tag_the_content_names() {
        let r = registries();
        for side in ["adventurer", "beasts", "goblins", "undead", "dar", CAPTIVE, TOWNSFOLK] {
            assert!(r.factions.id(side).is_some(), "{side}");
        }
        for kind in ["slash", "pierce", "crush", "fire", "cold", "poison"] {
            assert!(r.damage_kinds.id(kind).is_some(), "{kind}");
        }
        for status in ["poisoned", "scorched", "mending", "regenerating", "chilled", WEBBED, WARDED, WEAKENED, ASLEEP] {
            assert!(r.statuses.id(status).is_some(), "{status}");
        }
        for slot in ["main hand", "off hand", "head", "body", "left finger", "right finger", "light"] {
            assert!(r.slots.id(slot).is_some(), "{slot}");
        }
        for tag in ["weapon", "armor", "potion", "scroll", "staff", "wand", "ring", "gold", "arrow", "light", "thrown", "artifact", "waystone"] {
            assert!(r.tags.id(tag).is_some(), "{tag}");
        }
        for (one, other) in PAIRS {
            assert!(r.slots.id(one).is_some() && r.slots.id(other).is_some(), "{one} and {other} are slots");
        }
    }

    /// Every stat starts where the content sheet puts it, the adventurer's
    /// strength at twelve and accuracy at a hundred, and speed at nought,
    /// since the turn loop adds it to an actor's own: at a hundred,
    /// everyone would move twice as fast. A step carries as far as the
    /// house's noise says one does, so nobody steps louder for having
    /// stats at all.
    #[test]
    fn every_stat_starts_where_the_sheet_says_and_speed_at_nought_so_nobody_moves_faster() {
        let r = registries();
        let base = |name: &str| r.stats.get(r.stats.expect(name)).base;
        let named = [STRENGTH, ACCURACY, EVASION, SPEED, STEALTH, MENDING, LIGHT, ARMOR, FOOTFALL, REGENERATION, WISDOM, REAPING, AWARENESS, QUIET, ATTACK];
        let bases: Vec<(&str, i32)> = named.map(|s| (s, base(s))).to_vec();
        let sheet = [
            (STRENGTH, 12),
            (ACCURACY, 100),
            (EVASION, 0),
            (SPEED, 0),
            (STEALTH, 0),
            (MENDING, 0),
            (LIGHT, 0),
            (ARMOR, 0),
            (FOOTFALL, 2),
            (REGENERATION, 0),
            (WISDOM, 0),
            (REAPING, 0),
            (AWARENESS, 0),
            (QUIET, 0),
            (ATTACK, 0),
        ];
        assert_eq!(bases, sheet, "the sheet's bases");
        assert_eq!(base(FOOTFALL), crate::plugin::NOISE.step, "a step as loud as the house's");
        assert_eq!(r.stats.len(), named.len(), "a stat this test does not name");
        let speed = r.stats.get(r.stats.expect(SPEED));
        assert_eq!((speed.min, speed.max), (Some(-90), Some(300)), "no chill stops a body dead");
    }

    /// The run's rules name the stats they read: accuracy against evasion
    /// for a blow, the armor stat beside what is worn, mending on a heal,
    /// the attack stat on every blow and shot's roll, and speed in the
    /// turn loop. A blow on the unaware lands at three times, the house's,
    /// which a dagger's own outranks.
    #[test]
    fn a_blow_a_heal_and_the_turn_loop_read_the_stats_that_change_them() {
        let r = registries();
        let stat = |name: &str| Some(r.stats.expect(name));
        let rules = combat_rules(&r);
        assert_eq!((rules.accuracy, rules.evasion, rules.armor, rules.mending), (stat(ACCURACY), stat(EVASION), stat(ARMOR), stat(MENDING)));
        assert_eq!(rules.attack, stat(ATTACK), "nothing bestows it, but the crown's aura reads it");
        assert_eq!(rules.unaware, Some(300), "three times on the unaware");
        assert_eq!(turn_rules(&r).speed, stat(SPEED));
    }

    /// A chill takes a quarter off its holder's speed, and the drowning
    /// grasp chills whatever it catches for four turns, as part 1's sheet
    /// first wanted before the turn loop read a speed from a stat.
    #[test]
    fn the_drowning_grasp_chills_what_it_catches_for_four_turns_and_a_chill_takes_a_quarter_off_its_speed() {
        let r = registries();
        let chilled = r.statuses.get(r.statuses.expect("chilled"));
        assert_eq!(chilled.modifiers.iter().map(|m| (m.stat, m.op)).collect::<Vec<_>>(), [(r.stats.expect(SPEED), Op::Add(-25))]);
        let library = Library::load(&effect_kinds(), &Moments::default());
        let grasp = library.abilities.expect("drowning grasp");
        let said = library.abilities.describe(grasp, &library.registries);
        assert!(said.iter().any(|s| s == "chilled for 4 turns"), "{said:?}");
    }

    /// Part 3's six new abilities, each landing what its trait's words
    /// promise: a goblin mystic's ward is three armor's status for eight
    /// turns, a dar priestess mends what a staff of healing would, a dar
    /// battlemage's firebolt and word of slowness are a bolt of harm and a
    /// chill by another name, a cave spider's web holds for two turns, and
    /// a blink moves whoever blinks four cells.
    #[test]
    fn part_3s_new_abilities_land_what_their_trait_says() {
        let library = Library::load(&effect_kinds(), &Moments::default());
        let said = |name: &str| library.abilities.describe(library.abilities.expect(name), &library.registries);
        assert_eq!(said("spit web"), vec!["webbed for 2 turns"]);
        assert_eq!(said("blink"), vec!["blinks you up to 4 cells"]);
        assert_eq!(said("ward ally"), vec!["warded for 8 turns"]);
        assert_eq!(said("mend ally"), vec!["mends 3d6"]);
        assert_eq!(said("firebolt"), vec!["2d6 fire"]);
        assert_eq!(said("slowness"), vec!["slowed for 5 turns"]);
        assert_eq!(said("war cry"), vec!["hastens every ally in sight for 4 turns"]);
        assert_eq!(said("spit"), vec!["poisoned for 4 turns"]);
        assert_eq!(said("call the drowned"), vec!["a drowned thrall rises from water within 6"]);
        assert_eq!(said("wyrm breath"), vec!["2d6+2 fire", "sets the ground alight for 3 turns"]);
    }

    /// A tripwire over what stands in the places, by name, so a prop is
    /// added or dropped on purpose. Furniture is scenery, which the nearby
    /// rail leaves out; what holds, sets off, hides or offers something is
    /// not, and what blocks the way is what the places are laid round.
    #[test]
    fn every_prop_of_the_town_and_the_floors_is_in_the_file_and_only_what_answers_for_itself_is_more_than_scenery() {
        let r = registries();
        let props = [
            ("lamp post", true),
            ("well", true),
            ("forge", true),
            // The furniture blocks, by hand in Mereholt and at a density
            // in the dressed rooms, which `rl_mapgen::Dress` keeps whole:
            // nothing it lays cuts a room or shuts in a slot laid before.
            ("anvil", true),
            ("shelf", true),
            ("barrel", true),
            ("table", true),
            ("counter", true),
            ("hearth", true),
            ("bed", true),
            ("altar", true),
            ("pew", true),
            ("candelabra", false),
            ("hanging lantern", false),
            ("witchlight", false),
            ("brazier", true),
            ("cold brazier", true),
            ("throne", true),
            ("chest", true),
            ("banner", false),
            ("bone organ", true),
            ("font", true),
            ("rack", true),
            ("egg sac", true),
            ("fire", true),
            ("shackles", false),
            // The flavor pool's thirteen fixed pieces: each a small, hand
            // laid piece with open margin round every blocking prop.
            ("cold firepit", false),
            ("bedroll", false),
            ("broken pot", false),
            ("remains", false),
            ("broken statue", true),
            ("shrine", true),
            ("offering bowl", false),
            ("coal heap", false),
            ("shovel", false),
            ("ore cart", true),
            ("rail", false),
            ("embalming slab", true),
            ("jar", false),
            ("orrery", true),
            ("cracked bell", true),
            ("fishing net", false),
            ("creel", false),
            // The flavor and reward dressings' own scenery: the furniture
            // among it blocks, as the town's does, and the clutter does not.
            ("chalk mark", false),
            ("bench", false),
            ("dice", false),
            ("roots", false),
            ("sack", false),
            ("footlocker", false),
            ("straw dummy", true),
            ("practice post", true),
            ("fungus stalk", false),
            ("cocooned husk", false),
            ("empty egg case", false),
            ("timber brace", false),
            ("fallen beam", false),
            ("ore vein", false),
            ("skull", false),
            ("sarcophagus", true),
            ("desk", true),
            ("reading lamp", false),
            ("page drift", false),
            ("ruined screen", false),
            ("shell heap", false),
            ("quench trough", true),
            ("slag heap", false),
            ("obsidian spire", false),
            ("midden", false),
            ("lectern", false),
            // The reward pieces'.
            ("coin chest", true),
            ("cache chest", true),
            ("overturned wagon", true),
            ("spilled crate", false),
            ("spring", true),
            ("dry spring", true),
            ("frost-rimed anvil", true),
            ("cold anvil", true),
            ("moonwell", true),
            ("darkened moonwell", true),
            ("reliquary", true),
            ("enchanter's altar", true),
            ("chronicle", true),
            ("scratched words", false),
            ("adventurer's remains", false),
            // `src/decor.rs`'s own ambient decor: a wall worth reading, laid
            // by the survey rather than any piece.
            ("wall inscription", false),
            // The risk-and-reward pieces', and what `src/vaults.rs` stands
            // down a spent one as.
            ("caged blade", true),
            ("caged staff", true),
            ("caged ring", true),
            ("shut cage", true),
            ("empty cage", true),
            ("gnawed plinth", true),
            ("rat hole", false),
            ("drowned altar", true),
            ("iron grate", false),
            ("teetering plinth", true),
            ("keystone plinth", true),
            ("cracked lintel", false),
            ("bare plinth", true),
            ("bare altar", true),
            ("palisade", true),
            ("loot pile", true),
            ("cocooned body", true),
            ("circle bones", false),
            ("grave sigil", false),
            ("warding glyph", false),
            ("dart plate", false),
            ("caustic vent", false),
            ("paralysis vent", false),
            ("ember plate", false),
            ("alarm glyph", false),
            ("net trap", false),
            ("trapdoor", false),
            ("fire jet", false),
            ("loose flagstone", false),
            ("concealed lever", false),
            ("pulled lever", false),
            ("offering altar", true),
            ("guardian statue", true),
            // Where the Drowned Gate brings the adventurer back.
            ("gate mark", false),
        ];
        // Every prop that holds something, sets something off, hides or
        // offers to be worked answers for itself, so a room of them is
        // something to act on and not just furniture to walk round.
        const NOT_SCENERY: &[&str] = &[
            "chest",
            "egg sac",
            "coin chest",
            "cache chest",
            "overturned wagon",
            // The anvil and the moonwell are scenery as written: their
            // choosing offers are `src/vaults/wells.rs`'s, which takes
            // the marker off as it puts each offer on.
            "spring",
            "reliquary",
            "enchanter's altar",
            "chronicle",
            "scratched words",
            "adventurer's remains",
            "wall inscription",
            "caged blade",
            "caged staff",
            "caged ring",
            "gnawed plinth",
            "drowned altar",
            "teetering plinth",
            "keystone plinth",
            "loot pile",
            "cocooned body",
            "circle bones",
            "grave sigil",
            "warding glyph",
            "dart plate",
            "caustic vent",
            "paralysis vent",
            "ember plate",
            "alarm glyph",
            "net trap",
            "trapdoor",
            "fire jet",
            "loose flagstone",
            "concealed lever",
            "offering altar",
        ];
        assert_eq!(r.props.len(), props.len(), "a prop this list does not name");
        for (name, blocks) in props {
            let def = r.props.get(r.props.id(name).unwrap_or_else(|| panic!("no prop called {name:?}")));
            assert_eq!(def.blocks, blocks, "{name}: whether it stands in the way");
            assert_eq!(def.is_scenery(), !NOT_SCENERY.contains(&name), "{name}");
        }
        let chest = r.props.get(r.props.expect("chest"));
        assert!(chest.container.as_ref().is_some_and(|c| c.contents.len() == 3), "a weapon, armor and a potion, by tag");
    }

    /// Plate turns a blade and a club, and nothing else: a tick of poison
    /// or of standing in fire that armor could stop would never tick at
    /// all, so the elements are what a resistance answers and armor never
    /// does.
    #[test]
    fn armor_turns_blows_and_never_the_elements() {
        let r = registries();
        let armored = |name: &str| r.damage_kinds.get(r.damage_kinds.expect(name)).armored;
        for kind in ["slash", "pierce", "crush"] {
            assert!(armored(kind), "{kind} is a blow");
        }
        for kind in ["fire", "cold", "poison"] {
            assert!(!armored(kind), "{kind} goes through plate");
        }
    }

    /// The design's war: the adventurer against every side, beasts and
    /// goblins against each other, the dead against everything living, and
    /// the dar fighting nobody but the adventurer, so the dungeon fights
    /// itself and the dar's own quarrel stays their own.
    #[test]
    fn the_adventurer_fights_every_side_and_the_sides_fight_each_other_as_the_design_says() {
        let r = registries();
        let rules = combat_rules(&r);
        let at = |a: &str, b: &str| rules.factions.relation(r.factions.expect(a), r.factions.expect(b));
        for other in ["beasts", "goblins", "undead", "dar"] {
            assert_eq!(at("adventurer", other), Relation::Hostile, "the adventurer and the {other}");
            assert_eq!(at(other, "adventurer"), Relation::Hostile, "the {other} and the adventurer");
        }
        assert_eq!(at("beasts", "goblins"), Relation::Hostile);
        assert_eq!(at("undead", "beasts"), Relation::Hostile);
        assert_eq!(at("undead", "goblins"), Relation::Hostile);
        for own in ["beasts", "goblins", "undead", "dar"] {
            assert_eq!(at(own, own), Relation::Allied, "{own} stand together");
        }
        for other in ["beasts", "goblins", "undead"] {
            assert_eq!(at("dar", other), Relation::Neutral, "the dar keep to themselves: {other} is no quarrel of theirs");
        }
        for other in ["adventurer", "beasts", "goblins", "undead", "dar"] {
            assert_eq!(at(CAPTIVE, other), Relation::Neutral, "a shackled captive and {other}");
            assert_eq!(at(other, CAPTIVE), Relation::Neutral, "{other} and a shackled captive");
            assert_eq!(at(WILDLIFE, other), Relation::Neutral, "a moth or a cave rat and {other}");
            assert_eq!(at(other, WILDLIFE), Relation::Neutral, "{other} and a moth or a cave rat");
        }
        for other in ["adventurer", "beasts", "goblins", "undead", "dar", CAPTIVE, DISCORDANT] {
            assert_eq!(at(TOWNSFOLK, other), Relation::Neutral, "Mereholt's own and {other}: nobody is hostile to it");
            assert_eq!(at(other, TOWNSFOLK), Relation::Neutral, "{other} and Mereholt's own: nobody is hostile to it");
        }
        assert_eq!(at(TOWNSFOLK, TOWNSFOLK), Relation::Allied, "the keepers and the crowd stand together");
    }

    /// What the neutral faction buys, in the one place it is read: a bump
    /// into a townsperson is neither a blow nor a free refusal, the same
    /// as the engine's own bump reads any neutral offering one thing, so
    /// it comes to an `Interact` instead. Task 9 gives Mereholt's people
    /// their own actor; this stands in for one with nothing but the side
    /// and the offer the design asks of it.
    #[test]
    fn a_townsperson_bumped_is_talked_to_not_struck() {
        use crate::testing::{adventurer, at, headless, settle};

        #[derive(Resource, Default)]
        struct Heard(Vec<Interacted>);
        fn hear(mut done: MessageReader<Interacted>, mut heard: ResMut<Heard>) {
            heard.0.extend(done.read().copied());
        }

        let mut app = headless(RunSeed(0));
        app.add_verb("talk");
        let talk = app.world().resource::<Verbs>().get("talk").expect("just declared");
        app.init_resource::<Heard>().add_systems(Update, hear);
        let me = adventurer(&mut app);
        let townsfolk = app.world().resource::<Registries>().factions.expect(TOWNSFOLK);
        let beside = at(&app, me).offset(1, 0);
        let keeper =
            app.world_mut().spawn((Actor, Blocks, Position(beside), Health::full(20), Faction(townsfolk), Offers(vec![Offering::new(talk, 100)]))).id();
        app.update();
        app.world_mut().write_message(Intent::new(me, Bump(Direction::East)));
        settle(&mut app);
        assert_eq!(app.world().get::<Health>(keeper).map(|h| h.current), Some(20), "talked to, not struck");
        assert_eq!(app.world().resource::<Heard>().0, vec![Interacted { actor: me, target: keeper, verb: talk, thing: None }], "the walk key talked to them");
    }

    /// Every light Undermere's files write, on a tile, a prop, an item or
    /// a trait, is at the one brightness, and every one sheds light rather
    /// than taking it: what tells a lamp from a glowcap is how far it
    /// reaches and its colour.
    #[test]
    fn every_light_the_files_write_is_at_the_one_brightness() {
        let library = Library::load(&effect_kinds(), &Moments::default());
        let undermere = crate::dungeon::Undermere::new(RunSeed(0));
        let mut lights: Vec<(String, Glow)> = Vec::new();
        lights.extend(undermere.tiles().iter().filter_map(|(_, t)| t.light.map(|l| (t.name.clone(), l))));
        lights.extend(library.registries.props.iter().filter_map(|(_, p)| p.light.map(|l| (p.name.clone(), l))));
        lights.extend(library.armory.defs.iter().filter_map(|(_, d)| d.light.map(|l| (d.name.clone(), l))));
        lights.extend(library.traits.iter().filter_map(|(_, t)| t.light.map(|l| (t.name.clone(), l))));
        assert!(lights.len() >= 20, "tiles, props, the lights and the glowing traits: {}", lights.len());
        for (name, light) in &lights {
            assert_eq!((light.intensity, light.darkens), (LIGHT_INTENSITY, false), "{name}");
        }
    }

    /// Every prop that is not meant to blend, and every walkable floor, each
    /// as plain red-green-blue steps of 255 multiplied channel by channel by
    /// `tint`, which is how a light colours what it lands on, the floors
    /// `only` and the props `standing` when they are not empty. Those the prop
    /// is too close to, by thirty from a floor's glyph colour or forty-five
    /// from its fill, are returned by name.
    fn blenders(tint: [f32; 3], only: &[&str], standing: &[&str]) -> Vec<String> {
        use rl_engine::rl_core::RunSeed;
        let undermere = crate::dungeon::Undermere::new(RunSeed(7));
        let looks = undermere.appearance();
        let rgb = |c: Color| {
            let c = c.to_srgba();
            [c.red * 255.0 * tint[0], c.green * 255.0 * tint[1], c.blue * 255.0 * tint[2]]
        };
        let apart = |a: [f32; 3], b: [f32; 3]| a.iter().zip(b).map(|(x, y)| (x - y).powi(2)).sum::<f32>().sqrt();
        let floors: Vec<(String, [f32; 3], [f32; 3])> = undermere
            .tiles()
            .iter()
            .filter(|(_, tile)| tile.walkable && !["door", "gate", "bridge", "causeway", "pool"].iter().any(|w| tile.name.contains(w)))
            .filter(|(_, tile)| only.is_empty() || only.contains(&tile.name.as_str()))
            .map(|(id, tile)| (tile.name.clone(), rgb(looks.lit(id).fg), rgb(looks.lit(id).bg)))
            .collect();
        assert!(floors.len() > 20 || !only.is_empty(), "the floors are read: {}", floors.len());
        assert!(only.is_empty() || floors.len() == only.len(), "every floor asked for is read: {} of {only:?}", floors.len());
        let mut blenders = Vec::new();
        for (_, prop) in registries().props.iter() {
            if prop.name == "loose flagstone" || !(standing.is_empty() || standing.contains(&prop.name.as_str())) {
                continue;
            }
            let c = prop.look.color;
            let c = [f32::from(c.r) * tint[0], f32::from(c.g) * tint[1], f32::from(c.b) * tint[2]];
            for (floor, fg, bg) in &floors {
                if apart(c, *fg) < 30.0 || apart(c, *bg) < 45.0 {
                    blenders.push(format!("{} on {floor}", prop.name));
                }
            }
        }
        blenders
    }

    /// A prop is drawn in a colour clearly apart from the ground it stands
    /// on, and in a glyph a floor does not wear: in plain red-green-blue
    /// steps of 255 it is at least thirty from each walkable floor's glyph
    /// colour and forty-five from each one's fill, and its glyph is none of
    /// the floors' own `.`, `,`, `:` and `-`. The loose flagstone alone is
    /// meant to be found by looking closely, as a flagstone is, and is let
    /// blend with the cobbles it lies among.
    #[test]
    fn every_prop_is_drawn_clear_of_every_floor_in_colour_and_in_glyph() {
        for (_, prop) in registries().props.iter().filter(|(_, p)| p.name != "loose flagstone") {
            assert!(!['.', ',', ':', '-'].contains(&prop.look.glyph), "{} is drawn '{}', as a floor is", prop.name, prop.look.glyph);
        }
        let blenders = blenders([1.0; 3], &[], &[]);
        assert!(blenders.is_empty(), "props that blend into a floor: {blenders:#?}");
    }

    /// The same under the tavern hearth's light on the floors of the town,
    /// which takes the blue out of everything: the barrels, counters and tables of the town stood brown
    /// on a floor that is brown in firelight, and a counter all but
    /// vanished. Held to the same margin once both are multiplied by the
    /// hearth's colour.
    #[test]
    fn every_prop_is_drawn_clear_of_every_floor_under_the_hearths_light_too() {
        let hearth = registries().props.iter().find_map(|(_, p)| (p.name == "hearth").then_some(p.light)).flatten().expect("the hearth is lit");
        let unit = |byte: u8| byte as f32 / 255.0;
        let (r, g, b) = (unit(hearth.color.r), unit(hearth.color.g), unit(hearth.color.b));
        // Mereholt's pieces, `assets/prefabs/mereholt.ron`, on the town's floors.
        let town = ["well", "lamp post", "forge", "anvil", "shelf", "barrel", "table", "counter", "hearth", "bed", "candelabra", "altar", "pew"];
        let blenders = blenders([r, g, b], &["cobbles", "road", "planks", "garden", "grass"], &town);
        assert!(blenders.is_empty(), "props that blend into a floor under firelight: {blenders:#?}");
    }
}
