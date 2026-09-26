# Accuracy and light bands

Status: built 2026-09-26 on the `accuracy-and-light` branch, from `docs/superpowers/specs/2026-09-24-accuracy-light-targeting-design.md`.

## 0. Summary

Every blow, shot and throw used to land.
The combat reference said so on purpose, and the only plan for a miss was a damage stage that returned zero.
That left range meaning nothing but a hard edge, and light meaning nothing to a fight at all.

Accuracy is a roll made once, before the damage, from odds a game's model computes.
Light gets three bands, dark, dim and lit, and the odds and stealth both read them.
The player sees the odds and every reason behind them in the targeting box and the inspect panel, from the same call the roll uses.

## 1. What a miss buys

A long shot into a dim ring is a gamble, and a short one in lamplight is not.
Range stops being a wall at the weapon's edge and becomes a slope a player can read.
Light becomes something to use: a lamp shows you everything and shows everything you, and the edge of its pool is where a shot goes wide and a sneak goes unseen.

## 2. The model

A `HitModel` turns a `Shot` into `Odds`.

`Shot` is plain facts gathered by the engine: how the attack travels (`Delivery::Melee`, `Shot` or `Thrown`), the distance, the weapon's effective range and reach, the `LightBand` at the target, and the attacker's accuracy and the target's evasion as the stats the game named give them, `None` where it named none.
`Odds` is `hits` chances in `out_of`, and the labelled `Line`s that produced it.

Every model reduces to that.
A percent roll is 67 in 100.
A d20 against a target number is 13 in 20.
Two dice against each other are an exact count of outcomes.
So a panel prints a chance and its reasons whatever the model is, and the resolver draws once whatever the model is; `rl-rules` proves the claim with a d20 model written in its tests.

`Certain` is the default and answers `None`: nothing is rolled and nothing is drawn from `CombatRng`, so a game that never chose accuracy plays and replays exactly as it did.
`Percent` is the model the engine ships: accuracy (100 without a stat) less evasion (0 without one), less a fixed number of points per tile past effective range, less a penalty for dim or dark light, clamped to 0..=100.
A blow reads neither range nor light: it lands on the cell beside the attacker, which the adjacency floor always shows.

## 3. Where the facts come from

`Marksmanship` is the one place the facts are gathered, and the attack resolver, the throw resolver, the targeting cursor and the inspect panel all ask it.
The chance a panel prints is therefore the chance the roll uses, the discipline `shot` and `flight` already keep for where a projectile goes.

Accuracy and evasion are stats, named on `CombatRules` with `accuracy_stat` and `evasion_stat` the way `armor_stat` is.
A weapon that should be clumsier is a worn item that moves the accuracy stat, which the engine already supports.
The one number on the weapon is `effective`, how far it reaches with no penalty, a third of `range` when left out.

## 4. Light bands

`LightBand::of(intensity, threshold, bright)` is the one rule, in `rl-grid`, where `rl-rules` can read the same type.
Below `threshold` is `Dark` and unseen, from there to `bright` is `Dim`, and at or above `bright` is `Lit`.
`bright` defaults to 64, which leaves a ring of one or two tiles of dim light at the edge of the lamps the example games carry.

Sight does not change: seen is anything not dark, plus dark sight, plus the adjacent tile.
Stealth's light bonus reads the lit band, so a subject in a lamp's dim ring is seen and gives a watcher no bonus.
The bands are symmetric: a monster shooting at a player in a dim ring pays the dim penalty the player would.

## 5. A miss

The roll is made after the weapon has fired and before the damage.
So a miss still writes `Struck` and the weapon's `fire` moment, and still spends heat, ammunition or a charge, since the weapon did fire.
It writes no `DamageEvent` and fires no `hit` moment; it writes `Missed`, at the moment a hit would have landed, for the narrator.

A shot's flight still plays and ends at the target.
A thrown item rests where a hit would have left it, and `ItemEvent::Thrown` reports that it struck nobody.
A throw that meets a body before the one aimed at is rolled against the body it meets, at that body's distance.
A throw with no strike, a grenade, rolls nothing, since nothing it does depends on striking.

A forecast counts the misses: `Combatant::hitting` scales what one blow is expected to deal, so a fight that is deadly on paper and a coin toss at the trigger reads as the coin toss.

## 6. Rejected

- **A miss as a `DamageStage` that returns zero.**
  The weapon's `hit` moment and on-hit riders would still fire, the narrator would say "no effect" rather than "miss", and a stage cannot tell a panel the chance before the roll or list what went into it.
- **A base accuracy on each weapon.**
  Accuracy belongs to whoever holds the weapon, and a weapon that should be clumsier already has a way to say so through a stat it moves while worn.
- **A list of boxed modifiers beside a fixed percent roll**, the `DamageStages` shape.
  A d20 game would have to fight the percent arithmetic, and two extension points for one question is one too many.
- **Distance as a flat per-tile penalty for every weapon**, or **a slope per weapon.**
  Flat makes a long gun as bad at range as a pistol; a slope on every weapon is one more number to balance for a crossover that the weapon's reach and the attacker's accuracy already produce.
- **Raising the seen cutoff instead of adding a band.**
  Darkness would hide monsters rather than make them hard to hit, and the fade on screen would still disagree with the rules.
- **A floor and ceiling of 5 and 95.**
  A genre convention rather than an engine need, and it would make an adjacent blow at accuracy 100 miss one time in twenty.

## 7. Not built

- Abilities still land unconditionally; rolling one is its own slice.
- Minds roll but do not weigh the odds when choosing between tactics.
- `Odds` reports the chance, not the draw, so a game cannot yet print the face of the die.
