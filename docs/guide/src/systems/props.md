<!-- documents:
     plugins: PropsPlugin
     files: crates/rl-rules/src/prop.rs
            crates/rl-rules/src/ai/snapshot.rs
            crates/rl-bevy/src/props.rs
            crates/rl-bevy/src/plugin.rs
            crates/rl-bevy/src/bump.rs
            crates/rl-bevy/src/turn.rs
            crates/rl-bevy/src/items.rs
            crates/rl-bevy/src/combat.rs
            crates/rl-bevy/src/ability.rs
            crates/rl-bevy/src/effects/mod.rs
            crates/rl-bevy/src/effects/triggers.rs
            crates/rl-render/src/map_view.rs
            crates/rl-ui/src/interact.rs
            crates/rl-save/src/run.rs
     fingerprint: 80f48349 -->

# Props

A prop is what stands on a map that is neither an actor nor an item: a crate, a plate, a console, a barrel, the wreck a machine leaves.
What one *is* comes to a cell, a name and a look.
Everything else, standing in the way, holding things, offering things, going off underfoot, hiding, breaking, is a component the definition asks for and a game may leave out.
What any of it means is the game's, and the whole of how a game says so is one message.

## Turning it on

`PropsPlugin` declares `needs::<Registries>` with `props` loaded from a `props.ron`, since a prop is a definition in a registry the way a monster is, and `depends_on::<CorePlugin>`.
It adds `offer_here` in `DecideSet::Offer`, beside the ability gate; `report_entered` at the end of `ResolveSet::Travel`, after every step, swap and warp; `resolve_interactions` and `resolve_takes` in `ResolveSet::Act`; `spot_hidden_props` in `DecideSet::Notice`, beside the stealth roll it resembles; `perceive_props` in `PerceiveSet::Annotate`; `arm_props` in `TurnSet::Schedule`; and `close_emptied_containers` and `report_destroyed` in `TurnSet::React`.
Outside the turn loop it configures `PropSet::Stock` then `PropSet::Fill` inside `EngineSet::Stream`, because a prop is put down while a place is built: the engine builds every prop's effects, arms each new prop and asks for what each container holds in the first, and a game answers in the second, so what goes into a crate lands in the frame the crate was put down rather than in that frame or the next depending on which way the executor ran two systems.
`report_bare_props` runs in `PostUpdate` and in no set at all, since `Added` matches for one frame and a report gated on play having begun is a report that never happens.
It takes `PropRng` for what a container holds and what is spotted, and adds `EffectsPlugin` if the game has not, which lands a trigger's effects from `EffectRng`, because a trap lands the same effects an ability does and wants the same dice.
It declares `reads` for every message its work may touch that belongs to a plugin a game may have left out, so a game may have props with no items, no combat and no statuses, and those queues stay empty.
`CorePlugin` registers `Intent<Interact>` whether or not props are on, because the bump redirect writes one and a writer for an unregistered message panics; the resolver and the sweep stay here, which is what decides whether props run.

## The model

`PropDef` is what a `props.ron` says: a `name`, a `Look` of glyph, colour and layer, `blocks`, an optional `health`, a list of `OfferDef`, a list of `triggers`, and optional `container` and `hidden`.
`load` resolves every name in the file and reports every problem in it at once rather than the first.
`spawn_prop(commands, registries, id, at, map)` puts one down and returns the entity, so a game can hang its own components on it; the definition says what, the game says where.
It inserts `Prop`, `PropKind(id)`, a `Name`, a `Position` and an `OnMap`, and then `Blocks`, `Health`, `Container` with an `Inventory`, and `Hidden` as the definition asks.
No glyph: a look travels as data and whoever draws dresses the prop from the same definition, exactly as a tile is described once as a tile and once as a look.
`OfferDef` is a `verb`, a `time` in hundredths of a step, an optional `needs` tag and a list of effects.
`offer_here` works out, for the actor holding the turn and nobody else, what it is offered by what it stands on and what stands beside it, as an `Offer { prop, verb, time, refused }`.
A refused offer is still listed, with `Refused::Needs(tag)` saying which tag it wants, so a screen greys the row and says why rather than hiding it.
A container's lock is folded in here, since opening a locked thing wants its key as surely as an offer that asked for one itself, and an `Emptied` container stops offering `open`.
`Interact { prop, verb }` is the one action whatever the verb: its resolver spends the offer's `time`, lands whatever effects the offer carried, and writes `Interacted { actor, prop, verb }`.
Verbs are interned by `Verbs`, `OPEN` and `SEARCH` first and the rest through `app.add_verb`, so two games spell `open` the same way.
`Container` is `Inventory` on something that is not an actor, so everything that reads a bag reads this one.
A container's `contents` are `ContentRoll`s, each a `Stock::Item` by name in a count or a `Stock::Tag` in a number of draws, with a `band` offset for a tag.
`stock_containers` rolls each row's count from `PropRng` and asks with `FillContainer { prop, what, count, band }`, keyed on a `Stocked` marker rather than on `Added` because a stream derived from the run's seed may not exist on the frame a place is built.
With `LootPlugin` the engine answers it, drawing a tag from the game's loot table at the container's band plus the offset, so one locker holds better things deeper; without one, a game answers a named item itself, and a container asking by tag is refused as play begins, since there is nothing to draw it from.
`Take { from, item }` moves one or all into the taker's bag, merging stacks and writing `ItemEvent::PickedUp` the way the ground does, and costs one turn either way.
`close_emptied_containers` marks a container `Emptied` only when its definition gives an opened look, so a crate that cannot show it is done goes on offering and the screen says it is empty.
Each trigger is the `TriggerSpec` an item's are, an `on` moment by name, an `Area`, an optional `fires` count and effects, and a prop answers two of the engine's moments, `entered` and `destroyed`.
`build_prop_effects` builds every trigger and every offer once on the first frame, after the last game-registered effect and moment are in, and reports every spec that would not build with the prop's name and which list it was in, because a trap that silently does nothing is the worst kind of trap.
`arm_props` puts its kind's `Triggers` and `LandsAsItself` on each new prop, so a trap strikes as itself and not as whoever stepped on it, the lists shared and the count of fires left the prop's own, since a definition is shared by every plate of its kind; `PendingFires` carries a restored prop's count until it is armed.
`report_entered` reads `Stepped`, which the move resolver writes for every step it lets through, so anything that walks sets off a plate, and it lands in the same pass, before damage.
`report_destroyed` reads `DeathEvent`, which a prop with `Health` raises like anything else, and since the prop is gone by the end of the frame it leaves a `Remnant` carrying its triggers and its name, reported and landed a pass later; a prop with nothing to do when broken leaves none.
Both only report `Fired { on, moment, by, at }`, the second through the remnant; [Effects](effects.md) lands the list, rolling each chance and reading each argument exactly as a spell does, over the one cell or the burst the trigger names, and a game reads the same message for what no effect can say.
`Hidden { spot }` is a prop nobody has spotted: the map view and a mind's contributor both skip one, which is a query filter rather than a second drawing path, and `spot_hidden_props` rolls `spot` percent a turn for the player alone while it is in sight, writing `Spotted` and taking the component off.
`perceive_props` puts what the mind holding the turn can see into `Snapshot::props` as a `PropView` of which entity, where, and whose side, so a mind that walks to wrecks and a mind that walks to crates read one list.

## Using it

The engine owns the interaction and the game owns what it meant, which is a system reading `Interacted` and keeping the verb it registered.

<!-- include: ../../../../examples/foundry/src/mission.rs:charge -->
```rust,no_run
/// Answers the `charge` verb: reports the fact the tracker counts, says
/// so, and leaves the console reading as spent.
///
/// The engine has already decided the charge was possible, spent the
/// three turns the offer asked for, and landed whatever effects the offer
/// carried, which for this one is none. What is left is the part no
/// effect could express: a fact about this run. The console then becomes
/// a different kind of prop, one with no offer and a duller glyph, which
/// is how it stops being something to charge twice.
pub fn answer_charge(
    mut commands: Commands,
    mut done: MessageReader<Interacted>,
    verbs: Res<Verbs>,
    registries: Res<Registries>,
    mut report: ChargeReport,
    consoles: Query<(&PropKind, Option<&OnMap>)>,
) {
    let Some(charge) = verbs.get(CHARGE) else { return };
    let Some(spent) = registries.props.id("spent reactor console") else { return };
    for ev in done.read() {
        if ev.verb != charge {
            continue;
        }
        let Ok((_, on)) = consoles.get(ev.prop) else { continue };
        let deck = crate::decks::deck_of(on.map(|m| m.0).unwrap_or(MapId::SURFACE));
        // The glyph comes off with the kind, so the renderer dresses it
        // again from the definition of a console already used.
        commands.entity(ev.prop).remove::<Glyph>().insert(PropKind(spent));
        report.happened.write(Happened(Fact::new(report.facts.charge_set).about(u64::from(deck))));
        report.tell.write(Tell::new("You set the charge. The reactor stirs.", Tones::GOOD));
    }
}
```

## The line

The engine decides whether an interaction is possible, what it costs and when it resolves; what it meant is the game's, read off `Interacted` and filtered by verb.
There is no registry of behaviours and no trait to implement, because a game already knows how to answer a message, and a refusal is free: the offers are worked out before the player acts, so nothing is spent finding out that nothing was there.
A prop that blocks and offers one thing it can take up is walked into, because the bump redirect writes the interaction; one offering two, or offering one it refuses, spends no turn and says so as `Bumped`, since a walk key can neither ask a question nor give a reason.
What a bump cannot reach, a body underfoot or a plate already found, is `InteractKey`'s, which takes the one offer in reach and takes none where two are offered.
`OffersPanel` and `ContainerPanel`, in `rl-ui`, are where that question and that rummaging are drawn, and a game with neither gets nothing rather than a guess.
A prop that blocks sight is not here: field of view reads the map's tiles and the veil gas writes into, so a sight-blocking prop is a second mechanism and the veil is where it goes on the day something needs it.
Doors stay tiles, because a door has no state of its own to remember and props are for things that remember something.
A prop with no `Name` is reported once and loudly: nothing can list it or look at it, which is a spawn bug every time and never a choice.
Saving is the engine's, and the first save kind that is: `SavePlugin` registers `PropKind` itself, so a game that saves gets an emptied crate still empty, a sprung trap still sprung and a spotted plate still spotted, without writing a line for it.
A body a game dressed as a prop is not saved as a prop but as what it was, its prop kind kept with its remains, so the one entity is never written down twice.
Where a prop stands and what a container holds are saved as any entity's are; what is written down beyond that is which definition it is, by name, and the part that is this prop's own history rather than its kind's.

## Where it lives

`rl-rules` is tier 1 and has no Bevy in it: `prop.rs` is the content half and nothing in it runs, so a `props.ron` with three mistakes names three without an `App` anywhere.
Two things are left as names there rather than resolved to ids, and both for the same reason: a verb is a string until there is a run to intern it in, and a container's fixed contents are item names because items are a game's own registry, which the loot plugin resolves through the game's `ItemMaker`.
`rl-bevy` is tier 2 and owns everything that happens: `props.rs` is the plugin, the components, the gate, the two actions, the reports of its two moments and the spotting roll, and it is one file because a prop's parts are one idea seen from several sides.
What a trigger does is not in it: landing one is `effects`', shared with every item, so a prop's trap and a thrown grenade cannot drift apart.
What stays out of it says as much: the look is dressed in `rl-render` from the same definition, the offers screen and the take-only modal are in `rl-ui`, and the save kind is in `rl-save`, because `Saveable` is that crate's and `rl-bevy` sits below it.
