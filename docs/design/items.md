# Items: what a thing does

Status: built 2026-09-22, against `main` at `0797370`.
The reasoning is here; `docs/OVERVIEW.md` lists what exists.

## 0. Summary

The engine owns no item definition and never will: what an item *is* belongs to a game's own registry, and the engine reads components off whatever the game spawned.
This page is about the other half, which the engine does own: what a thing *does*, and which of the three carriers of an effect list it should use.

Three carriers exist, and the reason there are three rather than one is that they differ in when they land and on whom, not in what they can do:

| Carrier | Written in | Lands | Aimed | Spends |
| --- | --- | --- | --- | --- |
| An ability, through `Grants` | the ability file | when the ability resolves | yes: any mode the ability names | a pool, a charge, health, a tagged item |
| A prop's trigger or offer | the prop file | when the prop is entered, broken or its offer taken up | no: on the prop's own cell | nothing, or the game's own answer |
| `OnUse` on a carried thing | the game's item file | when the thing is used | no: on the user, where they stand | one use, when it is `Consumable` |

All three build the same `(kind, chance, args)` out of content, into the same `Effects`, landed by the same code.

## 1. The dividing line, and the day it was wrong

Foundry's stim and medkit shipped as abilities first: each item named an ability in `grants`, and each of those abilities was written in `abilities.ron` with `costs: [Charge(1)]`, which the engine spends off the item's stack.
It worked, it was tested, and it was wrong in three ways that are worth writing down, because all three are the kind of wrong that reads fine in a diff.

1. **One file held two concepts.** `stims`, granted by an upgrade, is something the commando was taught. `med gel` is what a thing in the pack does when it is torn open. Nothing in the schema said which was which.
2. **The abilities screen listed the pack.** Granted abilities are folded into `Known` by `refresh_known`, which is right for a wand and wrong for a ration: carrying two stims put two entries on the screen beside the one thing the commando actually knew.
3. **The item's economy was spelled as the ability's cost.** `Charge(1)` says "using me destroys one of whatever lent me", which is a fact about the item, written on the ability, and readable only from the ability's side.

So the line is drawn by what the use *needs*, not by what the thing is:

- **It needs aiming, a cooldown or a pool → it is an ability**, and the item lends it with `Grants`. A wand, an implant, an arc capacitor. Everything abilities know about targeting, gating and refusal reasons applies, and `Known` is the right place for it, because the carrier really has learned to point the thing.
- **It just happens, to whoever used it, where they stand → it is `OnUse`.** A stim, a ration, a gel. It never enters the ability registry, never appears in `Known`, and what it costs the thing is on the thing.

The line is a judgement and will be argued with. The test of it is whether `Known` reads as "what this run has learned"; if a carrier's entry makes that sentence false, it belongs on the item.

## 2. `Effects`, and why it exists at all

Before this slice, `ability.rs` had a private `Built { chance, effect }` with a loop that rolled the chance and applied each, and `props.rs` had a second copy of both, written months later, already differing in how it reported a name nobody registered.
A third carrier would have been a third copy.

`Effects` is that list, once: built from `&[EffectSpec]` against the registered kinds, reporting every spec that would not build rather than the first, and landed either onto a full `Landing` (an ability, which knows its own footprint and flight) or onto one cell through `land_on` (a prop or a used thing, which has neither).
`describe` moved with it, so a screen can say what a list does without knowing what carries it.

Two consequences worth stating:

- **Every effect in the engine rolls from one stream, `AbilityRng`.** A trap, a medkit and a spell are dealt from the same deck. That is deliberate: a subsystem that rolled from its own stream would shift another's dice by the order the passes happen to run in, which is the bug `docs/design/lighting.md` section 4 and the seeding rules in `CLAUDE.md` both exist to prevent.
- **`EffectSpec` deserializes itself now.** Each content file used to mirror the authored form privately, so a game's own file could not read an effect list at all without writing that mirror a third time. It is one public shape: `(kind: "Harm", chance: 50, args: (..))`, chance certain unless given, arguments left as text for the layer that registered the effect. The two engine loaders keep their own error paths, because they report every failure in a file at once and name the ability or prop it belongs to.

## 3. What a use is, and is not

`UseItem` is an action like any other: the items resolver charges a turn for it, checks the thing is really in the bag, and reports `ItemEvent::Used`.
What lands the effects is a separate system, `consumable::land_uses`, in the same `ResolveSet::Act` and ordered after the resolver.

Two things follow from where it sits:

- **The mend lands in the pass that spent the turn**, not a pass later, so a stim at one hit point beats the blow that is already queued behind it. That is why it is in `Act` rather than in `TurnSet::React`, which is where a reaction to what a turn *caused* belongs.
- **A use is one action a pass**, so `land_uses` can never run beside another resolver doing work, which is what lets Foundry's ambiguity test allow those pairs with a reason instead of inventing an order between them.

The user is the only target and their own cell the only cell. A thing that should land somewhere else is not a use: a thrown thing is `Throwable`, and an aimed thing is an ability.

## 4. `Consumable`, and the ladder it walks

`Consumable` is a marker, not a count, because the count already exists in three shapes and the engine already had the rule: one off `Charges` when the thing counts its own, else one off its `Stack`, else the thing itself, despawned.
That is the same ladder `Cost::Charge` walks, and it is what makes a potion a potion and a wand a wand without either having to say which it is.
A thing with `OnUse` and no `Consumable` survives being used, which is what a tool is.

A bag that held the last of a stack keeps a dangling entity for the rest of the frame; `forget_removed_items` clears it, which is where everything that stops being an item is forgotten.

## 5. What is deliberately not here

- **`on_equip` effects.** Effects are one-shot; wearing is a standing state. What a worn thing does is already declarative and needs no list: `Armor`, `Resists`, an attack, `Bestows` for the registered stats, `Grants` for an ability it lends. A one-shot when something goes on, a cursed plate that bites, is a real case and a small one; it waits for a game that wants it.
- **"Hold a status while worn."** The insulated suit that reads as arc-resistant. Today that is a game's own system, as Foundry's `WornDarkSight` and `sync_dark_sight` are. By the rule in `CLAUDE.md`, the second game to write it is the signal it belongs in the engine.
- **An engine item schema.** `uses`, `on_use`, `stack` and the rest are Foundry's own field names in Foundry's own file; Corsair's are different and should be. What is shared is the components and the effect vocabulary, never the file.
- **Items as offers.** A crate you open and a medkit you use are nearly the same shape, and `props`' offers already carry a verb, a time cost and a refusal reason in words. Unifying them would give the bag refusals like "needs a free hand" for free. It is the most interesting road not taken here: `OfferedHere` is spatial throughout, and carried offers would need their own collector and their own screen. Worth revisiting when something wants a refusal reason on a use.

## 6. Where the pieces are

- `crates/rl-bevy/src/effects.rs`: `Effects`, the engine's seven effects, and `AddEngineEffects`.
- `crates/rl-bevy/src/consumable.rs`: `OnUse`, `Consumable`, `land_uses`, `ConsumablesPlugin`.
- `crates/rl-bevy/src/items.rs`: the actions, `ItemEvent`, `Bestows` and the gear fold.
- `crates/rl-bevy/src/ability.rs`: `Grants`, `Charges`, `Known`, and the ability side of a lent use.
- `crates/rl-rules/src/ability.rs`: `EffectSpec`, the authored form any content file can read.
- `examples/foundry/src/gear.rs` and `examples/foundry/assets/items.ron`: a game's own item file, with `on_use` and `uses` mapped onto the components above.
