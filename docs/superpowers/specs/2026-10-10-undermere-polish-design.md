# Undermere, part 7: polish

Status: written 2026-10-10, while parts 4 (dungeon generation) and 6 (mercenaries) were being built by other sessions.
The game design is `examples/undermere/DESIGN.md`; part 7 there is "a pass over everything for the look, the feel and the balance".
The user asked for the design calls to be made without further questions, so every call below is the author's, made to be reviewed afterwards.

## Goal

Undermere looks as good as Brogue or better, every line it says reads right, and no build is always the right one.
The work is what parts 1 to 3 left for this part, what play found, and a last look over everything once parts 4 to 6 are in.

## The engine's half (wave A: can start now)

These are engine gaps the game found, each written for any game.

1. **A light that warms, and a light that darkens.**
   `Shading::light` multiplies a tile's colour by the light on it, so a fire's orange on green moss looks like the lantern (`docs/TODO.md` item 27).
   A light lends a share of its own colour as well, scaled by its intensity, as Brogue's does, so a coloured light can give a tile a colour it lacks.
   And a light may take light away: a light of negative intensity that darkens what it reaches, so a pool of true darkness is a light source like any other (part 2 wanted it for the potion of darkness).
2. **The player's long action.**
   The engine's multi-turn work skips the player (`docs/TODO.md` item 28), so Undermere ruled a captive's freeing uninterruptible.
   The player may take up a long action: its turns pass on their own, a key stops it, and it is broken off by a blow, by a foe coming into sight, or by anything a game adds.
3. **Narration that reads right.**
   An actor the player cannot see (concealed in the dark) is told as something unseen, never by name.
   An ability's name is said once: "Your bolt hits the rat for 3.", not "You use bolt on the rat." and then "Your bolt hits the rat for 3.".
   A proper name is said as written: "the Widow's Fang", not "your Widow's Fang".
   A multi-word proper name through a connective pluralises the right word.
4. **Uses and flights.**
   An aimed use ends its user's unseen status, as an attack does (`docs/TODO.md`).
   A reflected flight is drawn to the body that turned it and back to the one it lands on, so the picture agrees with the log.

## The game's half (wave B: after parts 4 and 6 are on main)

5. **Adopt the engine's half.** The potion of darkness pools true darkness; freeing a captive is the player's long action, broken off by a blow; artifacts are said by their names.
6. **Weapons with a manner.** The mace staggers (its blow knocks the struck back a cell or dazes it), the rapier lunges (a step toward a foe two cells away and a strike in one turn), the two part 2 left.
7. **Charms.** The rechargeable kind parts 2 and 3 left: a charm is used and recharges over turns, enchanting shortens its recharge and strengthens it; a handful (healing, protection, haste, shattering, teleportation, recharging), found deep and rare.
8. **Balance with everything in.** The soak at 150 delves a plan with parts 4 to 6's content: no plan below about half the best plan's win rate (part 3 ended with armor at a third of weapon's), the floor ten boss winnable under each of the three, and a run won roughly a fifth of the time; the Abbess's Chalice costs something on floors without the drowned.
9. **The look.** Real-window photographs of every scene the parts built (town, each biome, camps, lairs, captives, thrones, the look panel with a "?", the bag, the end screen and records), each read and fixed for what reads wrong; a lair's tiles follow its floor's biome.

## Not in this part

New systems beyond the above; the release's documentation pass, which comes with the engine's next release.
