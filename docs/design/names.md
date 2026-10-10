# Names

Status: built 2026-10-08, on branch `namegen` from `main`, for Undermere's townsfolk and mercenaries.
The reasoning is here; `docs/OVERVIEW.md` lists what exists, and the module docs of `rl-rules::namegen` are the reference, with the file's full option space.

## 0. Summary

A game with people in it names them, and a roguelike names them anew every run.
Undermere's fifth part names everyone in its town each run, and its sixth names the mercenaries at the tavern; a scroll's unknown title is the same problem in capitals.
Every game that wants this writes the same thing: pools of syllables, a few shapes a name can take, and a check that throws away the ones that read badly.
So the engine owns it, as `rl_rules::namegen`: a `NameBook` of named `NameSet`s read from one RON file, and a draw from the caller's stream that never loops.

## 1. The lineage: libtcod

libtcod's `namegen` is the reference, and the engine keeps its whole idea.
A config file holds named sets; each has syllable pools by position (`syllablesStart`, `syllablesMiddle`, `syllablesEnd`, `syllablesPre`, `syllablesPost`), single letters (`phonemesVocals`, `phonemesConsonants`), an `illegal` list, and `rules` in a string language: `$s$v$35m$e` is a start syllable, a vowel, a middle syllable 35 times in a hundred and an end, `%50` in front of a rule is its chance of being chosen, and `_` is a space.
A drawn name is thrown away and drawn again when it holds an illegal substring, a letter three times running, a pair repeated back to back, or a run of three letters seen again later.

Those rejections are the engine's too, unchanged, plus a stray or doubled space, a word given twice, and a length the set gives.
The word rule is the engine's own: `Lo Lo` repeats no run of three letters, so libtcod's rules let it through.
They are what make syllable names usable: without them `Kakak` and `Mermer` come up often enough to be noticed.

## 2. What differs, and why

**A pattern is RON, not a string language.**
A part is `Pool(p)`, `Maybe(pct, p)` or `Text(t)`, read by serde and resolved at load, so a pattern naming a pool the set does not have fails at startup naming both.
In libtcod an unknown `$x` is copied through, and a misspelt pool is a name with a stray character found in play.
A string language is shorter to write, but the files are a few lines each, and the engine's rule is that content is checked once at load and never in play.

**Pools are named by the set, not fixed by position.**
Start, middle and end are one kind of name's convention.
A byname, `the Elder`, `Halfhand`, is a pool like any other, and a pattern of given name, a space and a byname is three parts rather than a `syllablesPost` that means whatever the set uses it for.
Single letters are a pool of one-letter syllables, so libtcod's vowels and consonants need no field of their own.

**Patterns are drawn once, by weight.**
libtcod picks a rule, rolls its `%`, and picks again on a miss, so how often a rule is chosen depends on every other rule's chance.
A weight is read the way the loot table's is: only the ratio matters, and adding a pattern never changes how the others share.

**Case and length are the set's.**
libtcod capitalises the first letter.
A set here says `Title`, `Upper`, `Lower` or `AsWritten`, so the same generator writes `Osk Halfhand` and an unknown scroll's `XOMBA ULTH`, and its `length` is a rejection like the others rather than a pattern's accident.
The case is applied to syllables and never to a pattern's `Text`, so `Text(" the ")` between two title-cased syllables stays lower, `Osk the Younger`, without the set giving up its case and writing every syllable's capital by hand.
Length is counted in characters of the cased name, not bytes, and case is applied a character at a time, so a syllable outside ASCII is a syllable like any other; load allows for the few letters whose count changes with their case.

**A draw is bounded and may avoid names already given.**
`draw` tries `ATTEMPTS` times and returns `None` past them, and `draw_distinct` also refuses what the caller says is taken.
A run naming twenty people must never name two alike, and a set strict enough to refuse almost everything must cost a bounded number of tries rather than hang the game.

## 3. What load checks, and what it cannot

Load checks the shape: every pool named exists and holds no empty syllable, every pool is named by some pattern (an unused pool is a typo elsewhere), every weight and chance makes sense and the weights fit the roll, no forbidden substring is empty (it would refuse every name), the length is a range, and every pattern can make a name inside it, counting a `Maybe` as both absent and at its longest.
Every mistake is reported together, each naming its set.

It cannot check the luck: whether a set's rejections leave enough names for a run.
Sampling at load would need a generator, and engine code never makes one, so a set's yield is a property the game that ships it tests over a range of its seeds, as Undermere's are to draw a hundred distinct names from each of its sets.

## 4. Rejected

- **A Markov chain over a word list.** It makes names that sound like a source, but it needs a corpus per culture, a game has to find and license one, and it reproduces the source's own names often enough to be noticed. Syllable pools are a few lines a designer writes and reads.
- **libtcod's file format as is.** Familiar to some authors, but the string language is the part that hides mistakes, and the fixed positions are the part a byname does not fit.
- **Names as a registry of content.** A name is not content a rule looks up; it is a string a game shows. A registry would intern thousands of strings nothing refers to by id.
- **Uniqueness kept by the engine.** Which names are taken is the run's, saved by the game with the run, and differs by what is being named, so the engine asks through a closure rather than keeping a set of its own.
