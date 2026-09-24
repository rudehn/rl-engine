# Props

Status: designed 2026-09-21, built 2026-09-21 and 2026-09-22, whole.
The reasoning is here; `docs/OVERVIEW.md` lists what exists.

## 0. Summary

The engine has no word for a thing on the map that is not an actor and not an item.
Two games have now built one by hand, and built it differently.

Foundry's reactor console is a marker component, an action, a resolver, a key and a refusal message.
The heist's wall lamps are a marker component, two actions, two resolvers, a key, a `Sense` so the watch can see a dark lamp, and a `Tactic` to go and relight it.
Neither can reuse a line of the other, and a third game wanting a chest would write the same shape a third time.

That is the pattern the plan was written against: the engine left the behaviour out, so the games built it, and each built a different one.

Props are the answer, kept deliberately small.
A prop is a cell, a name and a look.
Everything else, blocking, containing, triggering, hiding, breaking, offering, is a component on top, and what any of it means is the game's.

## 1. What a prop is

`Prop` is a marker.
Beside it an entity carries `Position`, optionally `OnMap`, a `Name` and a `Glyph`, which is already what the nearby rail and the look cursor read: `InSight` lists anything with a glyph that is not `Dead`, so props need nothing new to be seen.

This is a path items already walk: a coin on the floor is listed under "On the ground" today, by a collector that asks only whether the entity is an `Actor`, so a prop needs no new drawing and no new collector.

`PropsPlugin` reports, once and loudly, a prop spawned without a name, the way `MindsPlugin` reports a mind spawned without it.
It says nothing about the glyph, because glyphs live above this crate: a prop carries its `PropKind`, and `rl-render` dresses it from the same definition, so a prop spawned from content always has one.
A nameless prop is invisible to every panel, which is a spawn bug every time and never a choice.

`Blocks` decides whether it stands in the way, and it is the ordinary component, so the occupancy index, the flow fields and the move resolver all already handle it.
Sight is not blocked in this slice; §10 says why.

## 2. Offers, and the one action

The engine owns the offer, not the payload.

What a prop offers is read from its definition through `PropKind`, not held as a second copy on the entity: one place to write it, nothing to keep in step, and nothing extra to save.
An offer is:

```ron
offers: [(verb: "open", time: 200, needs: "cutter", effects: [...])]
```

A lock is written on the container rather than on the verb, and the gate folds it in: opening a locked thing wants its key as surely as an offer that asked for one itself.

The engine collects the offers in reach of the actor holding the turn, the way the ability gate collects what it could use, and answers three questions the player asks: what can I do here, how long does it take, and why not.

`Interact { prop, verb }` is the one action.
Its resolver claims the turn, spends the offer's `time`, lands any effects the offer carried, and writes `Interacted { actor, prop, verb }`.

**That message is the whole extension mechanism.**
Foundry's console is not a trait impl and not a registered interaction: it is a system in `TurnSet::React` reading `Interacted`, keeping the ones whose verb is `charge`, and doing what it already does.
The engine needs no registry of behaviours because it already has messages, and a game already knows how to answer one.

Verbs are interned, `open` and `search` by the engine and the rest through `add_verb`, for the same reason sounds are: so two games spell `open` the same way, and so the engine can say "there is nothing here to open" in its own words.

### 2.1 Reaching an offer

One key, and bump.

A bump into a blocking prop with exactly one offer resolves to `Interact`, in `ResolveSet::Redirect`, beside the bump that resolves to `Open` on a door.
That is the shape `bump.rs` already documents for an alternate action, and it means walking into a crate opens it, which is what a player expects.

The interact key covers what bump cannot reach: a prop underfoot, which you are standing on rather than walking into, and a choice between several offers.
One offer runs at once; several open the offers screen.

A bump into a prop offering two things spends no turn and reports itself as `Bumped`, which is what that message has always meant: the bump came to nothing.
`OffersPanel` reads it after the pass and asks the question the turn loop could not, so walking into a workbench that offers two things asks which, and a game with no such screen simply gets nothing rather than a guess.
A row reads as the verb's registered name and the prop's own name, and a row that cannot be taken up is listed greyed with the reason in words, the way the ability menu lists what it cannot use.

`Intent<Interact>` is registered by `CorePlugin` even though props are opt-in, because bump writes it and a writer for an unregistered message is a panic.
The sweeper and the resolver stay with `PropsPlugin`, which is what decides whether props run at all.
The minds register `Intent<Attack>` the same way and for the same reason.

A refusal is free.
The offers are computed before the player acts, so the engine knows an interaction is impossible before a turn is spent on finding out.
Foundry charges a turn for setting a charge where there is no console, which was a workaround for not knowing.

## 3. Containers

A container is a prop with `Container`, which is `Inventory` on something that is not an actor, and the `open` verb.
The engine answers `open` on a container itself: contents and taking are the engine's, since the panel rules and the item actions all exist.

What it holds is written as item names rather than ids, and stays names: items are each game's own registry, so only the game can spawn one.
The engine rolls the counts from `PropRng` and asks the game to fill the container, which is the same seam a game's own spawner already sits behind.

The modal is take-only, one or all.
Putting things back is a stash mechanic, and no game here wants one.

The screen binds one key of its own, take-all, defaulting to `a` for all, and takes the rest from the shared cursor and direction bindings, so a game that rebinds those rebinds this screen too.
A game that wants another key inserts its own `ContainerKeys`, as it would for the bag or the ability menu.

`a` is free to be `a` although the ability menu opens on it, because a modal's keys are read only while it is the top screen.
That is the line between the two kinds of binding, and writing this screen is what drew it: the controls registry is for what works while the world is in front of the player, including the key that opens each screen, and a screen's own keys live in the screen and are written along its bottom border.
The engine had been doing both: the bag registered wear, drop, use and throw, and the log registered scrolling and filtering, each of them already printed in that screen's own footer.
Those are gone now, so the controls screen lists root-level keys only.

Taking does not close the screen, unlike every action the bag takes.
Opening already cost what the definition said, each take costs a turn, and a crate emptied a piece at a time would otherwise cost a screen a time.
It closes itself when the container goes out of reach, because a screen onto a crate across the room is a screen showing a lie.

`locked: (needs: "cutter")` refuses the offer, in words, until the actor carries something with that tag.
What a cutter is stays the game's.

An emptied container changes: `opened:` names the look it takes, so a crate you have done reads as done at a glance, and it offers nothing further.
A container whose definition gives no opened look keeps its offer and the modal says it is empty.

## 4. Triggers

Since 2026-09-23 a prop's triggers are the `Triggers` an item carries, owned by `EffectsPlugin`: `on` is a moment by name from an open registry, `entered` or `destroyed` for a prop, each with an `Area`, the prop reports the moment as `Fired`, and `land_triggers` lands it; `docs/design/effects.md` has the timing and why. What follows is the design as it was first built.

`Trigger { on, fires, effects }`, where `on` is `Entered` or `Destroyed` and `fires` is how many times it may go off, once for a pressure plate and more for a leaking line.

`Entered` is the pressure plate, read from `Stepped`, which the move resolver already writes for every step it lets through.
`Destroyed` is the exploding barrel, read from `DeathEvent`, which costs nothing now that a prop can be killed.

The payload is effects, so the trap in §11 is data with no Rust anywhere: `Emit` the fuel vapour, `Ignite` it.
A trigger that wants to do something no effect can say writes `Interacted`'s sibling, `Triggered`, and the game answers that.

How often it has gone off is `Fired`, on the prop, because a definition is shared by every plate of its kind and this is one plate's history.

Landing effects took two changes to what was already there.
`Landing::ability` is now an `Option`, because a trap lands the same effects an ability does and has no ability: the cues that draw an ability's look are skipped when there is none, and Delve's own `Drain` effect learned the same.
`EffectKinds::build` is public, so anything holding effects as content can build them, which a prop's triggers and offers do on the first frame, after the last game-registered effect is in.
A definition naming an effect nobody registered is reported loudly at that point, because a trap that silently does nothing is the worst kind of trap.

`Adjacent` and `Opened` are left out until something wants them.

## 5. Hidden

`Hidden { spot }` keeps a prop out of sight until it is found: the map view and `InSight` both filter it out, which is a query filter and not a second drawing path, since `rl-render` already depends on `rl-bevy`.

A prop in the player's sight is rolled for each turn, at `spot` percent, in `DecideSet::Notice` beside the stealth roll it resembles.
There is no search action: spending turns pressing a key at every wall is a chore the genre has spent twenty years removing.

The roll comes from `PropRng`, the plugin's own stream, so spotting never nudges a later blow.
The effects a trap lands roll from the effect stream instead, the one abilities use, because it is the same dice; `add_stream` now refuses to add a second deriving system for a stream two plugins both want.

A hidden trap still fires.
Not seeing it is the point.

Minds do not roll in this slice: they do not interact yet, and a trap catches whoever steps on it regardless.

## 6. Breaking

A prop with `Health` can already be attacked: `resolve_attacks` needs only a `Position` on its target, and a death writes `DeathEvent` like any other.
So a barrel that bursts and a lamp you shoot out are a prop with health and a `Destroyed` trigger, and the engine gains nothing new for them.

What breaking means, a fire, a spill, a room gone dark, is the game's, through effects or through the death it answers.

## 7. Remains become props

`RemainsPlugin` adds `Prop` when it converts an actor, and `Snapshot::remains` folds into props in the snapshot.

Remains is four days old with no consumers yet, so this is the cheapest this change will ever be, and it removes a category: a body is a thing on the map that can be interacted with, which is what a prop is.
`Remains { since, credit }` stays as the provenance.

Searching a wreck is then an offer with the `search` verb, not a bespoke action.

### 7.1 Naming

An actor's name is its own, so a wreck would otherwise read as "line droid" in the nearby rail, sitting in the things list beside loose slugs.

`RemainsPlugin::naming("{what} remains")` is the template, applied at conversion, and a game passes its own: `"{what} corpse"` in a dungeon, `"wreck of a {what}"` in a foundry.
One place, and every view and the log get it for nothing.

It survives a save by construction: the game's record still says "line droid", and the plugin re-applies the template when the save lays the body back down.
A game that wants a different name per kind sets `Name` itself, reacting to `RemainsLeft`, and then owns re-applying it on a continued run.

## 8. What a mind sees

Props go into the snapshot so a tactic can read them, and the offers with them, so a repair drone asks "is there an offer of `repair` near me" rather than a game pushing a `Sense` for it.

No engine tactic ships, and no mind interacts in this slice.
What an actor does on reaching a prop is the game's, and the player's loop is the part that can be tested end to end today.

## 9. Saving

`PropsPlugin` registers the save kind for props itself, which is a first: every save kind so far has been a game's.

It is right here because the engine spawns props from its own registry, so the engine is what knows how to spawn one again.
The alternative is every game writing `save_kind::<Prop>()` by hand and one of them forgetting, which is a bug that only shows up on a reload.

What is saved: which definition it is, where it lies, whether it has been opened, spotted, or spent its triggers, and what a container still holds.
An emptied crate stays emptied, a sprung trap stays sprung, a spotted plate stays spotted.

## 10. What is deliberately not here

- **Sight-blocking props.** Field of view reads the map's tiles and the veil that gas writes into. A prop that blocks sight has to write into that veil and bump the map's `opacity_epoch` so viewsheds and light recast. That is a second mechanism, nothing designed so far needs it, and the veil is where it goes when something does.
- **Doors as props.** Doors stay tiles. A door has no per-instance state, and tiles carry opacity and cost properly. Props are for things that remember something.
- **A banded spawn table.** Games place props at prefab marks and `Spot`s, as they already place consoles and stairs. Scattering by depth can come later without changing what a prop is.
- **Two-way containers, `Adjacent` and `Opened` triggers, an engine tactic, minds interacting.** All waiting for a use case.

## 11. The RON

```ron
// props.ron - what stands on a map that is not an actor and not an item.
//
// Every field (the ones marked "optional" may be left out):
//   name:      unique
//   glyph:     one character
//   color:     (r, g, b) in 0..=1
//   layer:     optional; draw order among entities on one cell, default 1
//   blocks:    optional; whether it stands in the way, default false
//   health:    optional; how much it takes to break it; absent, it cannot be
//   offers:    optional; [(verb:, time:, needs:, effects: [...])]
//              verb is registered ("open" and "search" are the engine's);
//              time is hundredths of a step, default 100;
//              needs is a tag the actor must carry, or the offer is refused;
//              effects are the engine's, landed when the interaction resolves
//   container: optional; (contents: [(item, min, max)], locked: (needs:),
//              opened: (glyph:, color:)) - the look it takes once emptied
//   trigger:   optional; (on: Entered | Destroyed, fires:, effects: [...])
//              fires is how many times it may go off, default 1
//   hidden:    optional; (spot:) percent a turn to spot it while in sight
#![enable(implicit_some)]
[
    (name: "supply crate", glyph: '&', color: (0.75, 0.65, 0.45), blocks: true, health: 6,
     container: (contents: [("slug", 8, 12), ("medkit", 0, 1)],
                 opened: (glyph: '"', color: (0.5, 0.45, 0.35))),
     offers: [(verb: "open", time: 200)]),

    (name: "locked cache", glyph: '&', color: (0.8, 0.8, 0.85), blocks: true,
     container: (contents: [("composite plate", 1, 1)], locked: (needs: "cutter")),
     offers: [(verb: "open", time: 300)]),

    (name: "fuel-line plate", glyph: '^', color: (0.9, 0.55, 0.2),
     hidden: (spot: 40),
     trigger: (on: Entered, fires: 1, effects: [
         (kind: "Emit",   args: (gas: "fuel vapour", amount: 90)),
         (kind: "Ignite", args: (turns: 3)),
     ])),

    (name: "reactor console", glyph: '%', color: (0.35, 0.85, 0.9), blocks: true,
     // No payload: Foundry answers `Interacted` with this verb itself.
     offers: [(verb: "charge", time: 300)]),
]
```

## 12. The slices

1. **`rl-rules`:** `PropDef`, the loader beside the monster and item loaders, and the verb names. Pure, tested without an `App`. **Done.**
2. **`rl-bevy`, the prop:** `Prop`, `PropsPlugin`, `Registries::props`, spawning one by id, the assert on a nameless prop, and `Blocks` working through the existing index. Tests: a prop stands in the way, a nameless one is reported, a prop is in sight and in the nearby rail. **Done**, with the look applied by `rl-render`'s `dress_props`: glyphs live above `rl-bevy`, so a prop is described once as data and once as a look, exactly as a tile is, and the engine reports only the missing name.
3. **Offers and `Interact`:** `Verbs`, the gate, the action, its resolver, `Interacted`, and the bump redirect. Tests: the offer's time is what the turn costs, an impossible interaction is free, bumping a blocking prop with one offer interacts, a prop underfoot is reachable only by the key. **Done.** Effects on an offer are parsed and carried but not yet landed; that arrives with the triggers in slice 5, which is where the effect machinery is wired in.
4. **Containers:** `Container`, locked, the take-only modal in `rl-ui`, and the opened look. Tests: taking moves items into the bag, a locked one refuses in words, an emptied one changes and offers nothing. **Done**, with `FillContainer`: the engine rolls the counts from `PropRng` and the game spawns what it named, since only the game has an item registry. Stocking is keyed on a `Stocked` marker rather than on `Added`, because a stream is derived from the seed and may not exist on the frame a place is built.
5. **Triggers, hidden and breaking:** `Trigger`, `fires`, `Hidden`, `PropRng`, the drawing and listing filters. Tests: a plate fires once and no more, a hidden plate fires unspotted, spotting is a roll from the plugin's own stream, a prop with health breaks and its `Destroyed` trigger fires. **Done**, and the proof that traps are data is a test with no `AbilitiesPlugin` at all: `PropsPlugin` registers what an effect may write, so a game may have traps without abilities, combat or statuses, and what nobody resolves, nobody answers.
6. **Saving:** the engine's own save kind. Test: an emptied crate, a sprung trap and a spotted plate all come back as they were. **Done.** It lives in `rl-save` rather than in `PropsPlugin`, because `Saveable` is `rl-save`'s and `rl-bevy` sits below it: `SavePlugin` registers the kind itself, so a game that saves gets its props saved without asking and a game with no props saves none. A prop is written down by its definition's name plus the part that is this prop's own history rather than its kind's: how often it has fired, whether it was stocked, whether it was emptied, and whether it is still unspotted. Where it stands and what it holds are `EntityState`'s, as for everything else.
7. **Remains as props:** `Prop` on conversion, the naming template, props in the snapshot in place of `Snapshot::remains`. **Done.** `RemainsView` became `PropView` and `Snapshot::remains` became `Snapshot::props`, filled by one contributor in `props`, so a mind that walks to wrecks and a mind that walks to crates read one list, and a prop nobody has spotted is in neither. `RemainsNaming` is the template, applied when an actor becomes a body and again when a save lays one back down, which is what keeps the wording in one place across a reload.
8. **Foundry:** supply crates on every deck, a hidden fuel-line plate per fabrication deck, wrecks named through the template, and the console moved onto `Interacted`. This is the proof, and the first thing in `examples/foundry/DESIGN.md` to get built. **Done**, with live cable in place of the fuel-line plate, since fabrication's decks do not exist yet and Foundry's damage kinds have no cryo. The console's bespoke action, resolver, key and refusal message are gone: what is left is a line of RON and a system that reads `Interacted` for one verb, because reporting a fact no effect can express is all that was ever Foundry's.

What adopting it taught, each fixed rather than worked around:
- **The interact key was missing.** Slice 3 built `Interact` and the bump redirect but no key, and a wreck lies underfoot where a bump cannot reach. `rl-ui`'s `InteractKey` is that key: it takes the one offer in reach and, where two are offered, takes none, because a key is not a question.
- **A game answering `FillContainer` raced the engine's ask.** Answered this frame or the next depending on which way the executor ran two systems, which moved every entity id after it and made Foundry's fingerprint tripwire flicker one run in five. `PropSet::{Stock, Fill}` inside `EngineSet::Stream` is the fix: the engine asks in one stage and a game answers in the next, so a game orders itself against the engine's phases and never against its functions.
- **Three systems that all spawn, left unordered, is nondeterminism.** Foundry's droids, loot and props all fill a deck on arrival; nothing in the game plays differently for their order, but a tripwire that reads a run by spawn order flickers. They are one chain now.
- **`report_bare_props` cannot live in a play-gated phase**, because props are put down while a place is built and `Added` matches for one frame. It runs in `PostUpdate`, in no set.
- **`DecideSet::Sense` is never configured**, which Foundry's ambiguity test caught the moment a prop held a `Viewshed` in the same pass. Putting it in the chain breaks delve's hearing test, so it is recorded in `docs/TODO.md` with the evidence rather than fixed in passing, and the pairs it causes are allowed with that bug as their reason rather than a claim they are safe.

## 13. Risks

- **The offer list is a new modal in the player's loop.** If it opens often it will feel like a tax. The mitigation is that bump and the single-offer case skip it entirely, so it should appear only in a genuinely ambiguous cell. Worth watching in Foundry before it spreads to the other games.
- **A game that builds `Bindings` by hand breaks whenever the engine adds a screen.** Corsair does, in its save key, and adding the container screen's keys broke its build until the new field was filled in. Nothing is wrong with the field; the wart is that the struct is built by hand outside a system at all. If a third screen breaks it again, `Bindings` wants a constructor that reads the world.
- **`Interacted` is a broad message.** Every game's prop behaviour reacts to the same one, filtered by verb. If a game grows dozens of verbs this becomes a dispatch table written by hand in systems. That is the point at which a registry of interactions earns its place, and not before.
- **The ground list gets crowded.** Props and remains are not a new kind of thing to the panels: items already lie about and are already listed, under "On the ground", by a collector that sorts on `Actor` and nothing else, so props join a path items proved. What changes is how much is in that list. A room with four crates, two wrecks and the slugs they dropped is eight rows where there used to be two, and the heading's word covers all of it. Whether the ground list wants grouping, or a heading a game can change per panel as it already can, is a question for Foundry to answer once it has crates, not something to guess at here.
