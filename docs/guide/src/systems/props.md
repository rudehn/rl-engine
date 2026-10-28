<!-- documents:
     plugins: PropsPlugin
     files: crates/atglyph-rules/src/prop.rs
            crates/atglyph-rules/src/ai/snapshot.rs
            crates/atglyph-bevy/src/props.rs
            crates/atglyph-bevy/src/plugin.rs
            crates/atglyph-bevy/src/bump.rs
            crates/atglyph-bevy/src/turn.rs
            crates/atglyph-bevy/src/items.rs
            crates/atglyph-bevy/src/combat.rs
            crates/atglyph-bevy/src/ability.rs
            crates/atglyph-bevy/src/effects/mod.rs
            crates/atglyph-bevy/src/effects/triggers.rs
            crates/atglyph-bevy/src/work.rs
            crates/atglyph-render/src/map_view.rs
            crates/atglyph-ui/src/interact.rs
            crates/atglyph-ui/src/focus.rs
            crates/atglyph-save/src/run.rs
            crates/atglyph-save/src/engine.rs
            crates/atglyph-bevy/src/lighting.rs
            crates/atglyph-bevy/src/app.rs
     fingerprint: 0007db88 -->

# Props

A prop is what stands on a map that is neither an actor nor an item: a crate, a plate, a console, a barrel, the wreck a machine leaves.
What one *is* comes to a cell, a name and a look.
Everything else, standing in the way, holding things, offering things, going off underfoot, hiding, breaking, is a component the definition asks for and a game may leave out.
What any of it means is the game's, and the whole of how a game says so is one message.

## Turning it on

`PropsPlugin` declares `needs::<Registries>` with `props` loaded from a `props.ron`, since a prop is a definition in a registry the way a monster is, and `depends_on::<CorePlugin>`.
It works out the offers in `DecideSet::Offer`, beside the ability gate; reports what was stepped on at the end of `ResolveSet::Travel`, after every step, swap and warp; resolves interactions and takes in `ResolveSet::Act`; rolls to spot what is hidden in `DecideSet::Notice`, beside the stealth roll it resembles; puts props into a mind's snapshot in `PerceiveSet::Annotate`; arms new props in `TurnSet::Schedule`; closes emptied containers in `ReactSet::Settle`; and reports what was broken in `TurnSet::React`.
Outside the turn loop it configures `PropSet::Stock` then `PropSet::Fill` inside `EngineSet::Stream`, because a prop is put down while a place is built: the engine builds every prop's effects, arms each new prop and asks for what each container holds in the first, and a game answers in the second, so what goes into a crate lands in the frame the crate was put down rather than in that frame or the next depending on which way the executor ran two systems.
The report of a prop with no name runs in `PostUpdate` and in no set at all, since `Added` matches for one frame and a report gated on play having begun is a report that never happens.
It takes `PropRng` for what a container holds and what is spotted, and adds `EffectsPlugin` unless something already has, which lands a trigger's effects from `EffectRng`, because a trap lands the same effects an ability does and wants the same dice.
It declares `reads` for every message its work may touch that belongs to a plugin a game may have left out, so a game may have props with no items, no combat and no statuses, and those queues stay empty.
`PropsPlugin::spotting(rules)` weighs the spotting roll by a `SpotRules`, how near a hidden thing must be and what a turn spent waiting is worth; plain `PropsPlugin` keeps the default, whole at any distance in sight.
`CorePlugin` registers `Intent<Interact>` whether or not props are on, because the bump redirect writes one and a writer for an unregistered message panics; the resolver and the sweep stay here, which is what decides whether props run.

## The model

`PropDef` is what a `props.ron` says: a `name`, a `Look` of glyph, colour and layer, `blocks`, an optional `health`, a list of `OfferDef`, a list of `triggers`, and optional `container`, `hidden` and `light`, the `Glow` the prop sheds wherever it stands and takes with it when it goes.
`load` resolves every name in the file and reports every problem in it at once rather than the first.
`spawn_prop(commands, registries, id, at, map)` puts one down and returns the entity, so a game can hang its own components on it; the definition says what, the game says where.
It inserts `Prop`, `PropKind(id)`, a `Name`, a `Position` and an `OnMap`, and then `Blocks`, `Health`, `Container` with an `Inventory`, and `Hidden` as the definition asks.
A definition that offers nothing, holds nothing, answers no trigger kind, has no `health` and is not hidden is scenery, as `PropDef::is_scenery` reads it, and the prop gets `Scenery` as well.
No glyph: a look travels as data and whoever draws dresses the prop from the same definition, exactly as a tile is described once as a tile and once as a look.
`OfferDef` is a `verb`, a `time` in hundredths of a step, an optional `needs` tag and a list of effects.
The gate works out, for the actor holding the turn and nobody else, what it is offered by what it stands on and what stands beside it, as an `Offer { target, verb, time, price, asks, refused }` in `OfferedHere`, `target` whatever offered it, a prop or an actor; `OfferedHere::for_actor` answers only for the actor it was worked out for.
A refused offer is still listed, with `Refused::Needs(tag)` saying which tag it wants, so a screen greys the row and says why rather than hiding it.
A container's lock is folded in here, since opening a locked thing wants its key as surely as an offer that asked for one itself, and an `Emptied` container stops offering `open`.
`Interact { target, verb }` is the one action whatever the verb: its resolver spends the offer's `time`, lands whatever effects the offer carried, and writes `Interacted { actor, target, verb }`.
An actor offers the same way a prop does, through its own `Offers`, a list of `Offering` built with `Offering::new(verb, time)` rather than read from a definition, since what an actor offers is its own rather than its kind's, a captive to free or someone to talk to.
`.needing(tag)` refuses it to anyone not carrying the tag, `.priced(n)` charges `n` of the trade slice's `Currency`, paid into the offerer's pack, `.asking(question)` asks yes or no first, and `.choosing(prompt, among)` first chooses one of the taker's things from what the game's `OfferFilter` system names, so backing out of either costs nothing.
The gate reads whichever of the two an actor carries, and refuses an actor's own offers to its own holder and to anyone the gate's `CombatRules` call hostile, so a foe is struck rather than talked past and a bump into one takes nothing up; an ally's offers stand, as a prop's do.
`Offers` is read where it sits and never saved by the engine, so a game puts it back on with whatever it saves the actor as, a prop included.
Verbs are interned by `Verbs`, `OPEN` and `SEARCH` first and the rest through `app.add_verb`, so two games spell `open` the same way.
`Container` is `Inventory` on something that is not an actor, so everything that reads a bag reads this one.
A container's `contents` are `ContentRoll`s, each a `Stock::Item` by name in a count or a `Stock::Tag` in a number of draws, with a `band` offset for a tag.
Stocking rolls each row's count from `PropRng` and asks with `FillContainer { prop, what, count, band }`, keyed on a `Stocked` marker rather than on `Added` because a stream derived from the run's seed may not exist on the frame a place is built.
With `LootPlugin` the engine answers it, drawing a tag from the game's loot table at the container's band plus the offset, so one locker holds better things deeper; without one, a game answers a named item itself, and a container asking by tag is refused as play begins, since there is nothing to draw it from.
`Take { from, item }` moves one or all into the taker's bag, filling its stacks to their `StackLimit` and writing `ItemEvent::PickedUp` the way the ground does, and costs one turn either way.
A container is marked `Emptied` only when its definition gives an opened look, so a crate that cannot show it is done goes on offering and the screen says it is empty.
Each trigger is the `TriggerSpec` an item's are, an `on` trigger kind by name, an `Area`, an optional `fires` count and effects, a word it does not read failing the load, and a prop answers two of the engine's trigger kinds, `entered` and `destroyed`.
Every trigger and every offer is built once on the first frame, after the last game-registered effect and trigger kind are in, with the game's sounds in scope when it has any, so an alarm plate names its sound as it names a status, and reports every spec that would not build with the prop's name and which list it was in, because a trap that silently does nothing is the worst kind of trap.
Arming puts its kind's `Triggers` and `LandsAsItself` on each new prop, so a trap strikes as itself and not as whoever stepped on it, the lists shared and the count of fires left the prop's own, since a definition is shared by every plate of its kind; `PendingFires` carries a restored prop's count until it is armed.
The `entered` report reads `Stepped`, which the move resolver writes for every step it lets through, and `Swapped` for both who changed places, so anything that walks sets off a plate, and it lands in the same pass, before damage.
The `destroyed` report reads `DeathEvent`, which a prop with `Health` raises like anything else, and since the prop is gone by the end of the frame it leaves a `Remnant` carrying its triggers and its name, reported and landed a pass later; a prop with nothing to do when broken leaves none.
Both only report `Fired { on, moment, by, at }`, `moment` a `TriggerKindId`, the second through the remnant; [Effects](effects.md) lands the list, rolling each chance and reading each argument exactly as a spell does, over the one cell or the burst the trigger names, and a game reads the same message for what no effect can say.
`Hidden { spot }` is a prop nobody has spotted: the map view, what it remembers and a mind's contributor all skip one, which is a query filter rather than a second drawing path.
The spotting roll is made once a turn for the player alone, for every hidden prop and every group of the place's secrets in sight, at `SpotRules::chance` of the thing's `spot` percent plus the player's `SpotBonus`, weighed by distance and by whether its last turn was a wait.
A find writes one `Spotted { what, by, at }`, `what` a `Finding` of a prop or a secret's group, and takes `Hidden` off or lays the secret as the tile it is.
The props the mind holding the turn can see go into `Snapshot::props` as a `PropView` of which entity, where, and whose side, so a mind that walks to wrecks and a mind that walks to crates read one list.

## Using it

The engine owns the interaction and the game owns what it meant, which is a system in `TurnSet::React` reading `Interacted`, passing over every message whose `verb` is not the one it registered, and doing what the verb meant to the `target`: a console that takes a charge writes the run's fact, says so with a `Tell`, and is given a spent `PropKind` with no offer, which is how it stops being something to charge twice.

## The line

The engine decides whether an interaction is possible, what it costs and when it resolves; what it meant is the game's, read off `Interacted` and filtered by verb.
There is no registry of behaviours and no trait to implement, because a game already knows how to answer a message, and a refusal is free: the offers are worked out before the player acts, so nothing is spent finding out that nothing was there.
A prop or an actor that blocks and offers one thing that can be taken up is walked into, because the bump redirect writes the interaction; one offering two, offering one it refuses, or one that is priced, asks or chooses first spends no turn and says so as `Bumped` with `offers` set, since a walk key can neither ask a question nor give a reason, nor spend currency.
`OffersPanel` opens on that flag rather than asking whether the thing bumped was a prop, which is what lets a captive bumped into with nothing free to say read the same way a workbench with two tools does.
What a bump cannot reach, a body underfoot or a plate already found, is `UiPlugin`'s interact key, its binding in `InteractKeys`, which takes the one offer in reach and takes none where two are offered.
`OffersPanel` and `ContainerPanel`, in `atglyph-ui`, are where that question and that rummaging are drawn, and a game with neither gets nothing rather than a guess.
A verb whose answer takes several turns is the game's to begin: an offer taken up is `Interacted` at once, and a game that wants the taking to cost more than its offer's `time` begins a `Works` on the target itself, which the engine's multi-turn work already carries to done or broken without asking the player again.
A prop that blocks sight is not here: field of view reads the map's tiles and the veil gas writes into, so a sight-blocking prop is a second mechanism and the veil is where it goes on the day something needs it.
Doors stay tiles, because a door has no state of its own to remember and props are for things that remember something.
Scenery is a prop rather than a tile because it stands in the way and can be looked at by name, and it stays out of what is listed in sight, the nearby rail and the Tab cycle, because a room of machinery would otherwise bury its one crate; the map draws it and the look cursor names it, and a game that hangs a behaviour of its own on one takes `Scenery` off.
A prop with no `Name` is reported once and loudly: nothing can list it or look at it, which is a spawn bug every time and never a choice.
Saving is the engine's, and the first save kind that is: `SavePlugin` registers `PropKind` itself, so a game that saves gets an emptied crate still empty, a sprung trap still sprung and a spotted plate still spotted, without writing a line for it.
A body a game dressed as a prop is not saved as a prop but as what it was, its prop kind kept with its remains, so the one entity is never written down twice.
Where a prop stands and what a container holds are saved as any entity's are; what is written down beyond that is which definition it is, by name, and the part that is this prop's own history rather than its kind's.

## Where it lives

`atglyph-rules` is tier 1 and has no Bevy in it: `prop.rs` is the content half and nothing in it runs, so a `props.ron` with three mistakes names three without an `App` anywhere.
Two things are left as names there rather than resolved to ids, and both for the same reason: a verb is a string until there is a run to intern it in, and a container's fixed contents are item names because items are a game's own registry, which the loot plugin resolves through the game's `ItemMaker`.
`atglyph-bevy` is tier 2 and owns everything that happens: `props.rs` is the plugin, the components, the gate, the two actions, the reports of its two trigger kinds and the spotting roll, and it is one file because a prop's parts are one idea seen from several sides.
What a trigger does is not in it: landing one is `effects`', shared with every item, so a prop's trap and a thrown grenade cannot drift apart.
What stays out of it says as much: the look is dressed in `atglyph-render` from the same definition, the offers screen and the take-only modal are in `atglyph-ui`, and the save kind is in `atglyph-save`, because `Saveable` is that crate's and `atglyph-bevy` sits below it.
