//! The bosses' halls: floor ten's throne, drawn from three, and the lair a
//! mini-boss's floor grows. Both are prefabs grown as areas of their own,
//! through the one entry each file's bottom row marks, joined by
//! [`Join::Arch`]: plain floor, nothing to open, as the corridor out of a
//! hall always was. The throne's own way in is [`ClearOfTheThrone`]'s to
//! hold far enough off.

use super::*;
use rl_engine::rl_mapgen::Join;

/// Fails the floor when the way in is nearer the grown piece with
/// [`Area::entries`] than half the floor's longer side, walking: the
/// throne is grown now, so its mouth is wherever its one entry fell
/// rather than always south, and [`RandomStart`] draws the way in with no
/// care for which piece it ends up facing. A floor whose way in already
/// clears it passes untouched, for [`Chain::run_retrying`] to grow
/// another where it does not.
pub(super) struct ClearOfTheThrone;

impl Pass<BaseContext> for ClearOfTheThrone {
    fn name(&self) -> &'static str {
        "clear_of_the_throne"
    }

    fn phase(&self) -> Phase {
        Phase::Exits
    }

    fn apply(&self, ctx: &mut BaseContext) -> Result<(), BuildError> {
        let err = |reason: String| BuildError::new("clear_of_the_throne", reason);
        let start = ctx.outputs().first::<StartPoint>().ok_or_else(|| err("no start point emitted before it".into()))?.0;
        let throne = ctx.outputs().iter::<Area>().find(|a| a.entries.is_some()).ok_or_else(|| err("no throne grown".into()))?;
        let bounds = ctx.terrain().bounds();
        let need = bounds.width.max(bounds.height) / 2;
        let tables = ctx.tiles().tables();
        let opened = Opened { terrain: ctx.terrain(), tables: &tables };
        let mut walk = DijkstraMap::covering(&opened);
        walk.build(&opened, [start], PathRules::default());
        match throne.cells.iter().filter_map(|c| walk.value(*c)).min() {
            Some(d) if d >= need => Ok(()),
            Some(d) => Err(err(format!("the way in is {d} steps from the throne, nearer than half the {} side", bounds.width.max(bounds.height)))),
            None => Err(err("the throne cannot be walked to from the way in".into())),
        }
    }
}

/// One of the three floor ten bosses a run draws between.
/// `piece` is its own `assets/prefabs/` file's `name:`, the Drowned
/// King's own kept as `"the throne"` from part 1 rather than renamed, and
/// the other two matching `monsters.ron`'s as a mini-boss's lair already
/// does; `monsters.ron`'s own name, what the score and the records use,
/// is read off whichever monster the piece's own slot stands, never kept
/// twice here. `wall` and `floor` are what the whole of floor ten is
/// built of when this one is drawn, its own hall and the ordinary rooms
/// leading to it alike, so the walk to the throne reads as one place
/// rather than the drawn boss's own colours leading to somebody else's
/// door.
pub(crate) struct FinalBoss {
    /// The name its own `assets/prefabs/` piece is loaded by.
    pub(crate) piece: &'static str,
    /// What floor ten is filled with before anything is grown, when this
    /// one is drawn.
    pub(super) wall: &'static str,
    /// What floor ten's rooms and its own hall are grown as, when this
    /// one is drawn.
    pub(super) floor: &'static str,
    /// The `assets/biomes.ron` entry that stands with this boss: `biomes`
    /// reads this to draw floor ten's biome, rather than by weight, so
    /// the two draws can never disagree.
    pub(super) biome: &'static str,
}

/// The three, the content sheet's own order: the Drowned King's own
/// flooded hall kept exactly as part 1 built it, the Mother Below's cave
/// reusing the Fungal Deeps' own wall and floor, and the Ember Wyrm's hall
/// reusing the Forge of Embers' own, so no new tile is spent on a look the
/// floors above already have.
pub(crate) const FINAL_BOSSES: [FinalBoss; 3] = [
    FinalBoss { piece: "the throne", wall: "throne wall", floor: "throne floor", biome: "Drowned Throne" },
    FinalBoss { piece: "the Mother Below", wall: "cave wall", floor: "moss", biome: "Brood Hollow" },
    FinalBoss { piece: "the Ember Wyrm", wall: "forge wall", floor: "ash", biome: "Wyrm's Hall" },
];

/// The one of [`FINAL_BOSSES`] a run of `seed` holds on floor ten: pure
/// over `seed`, so every call agrees without the draw being kept
/// anywhere. A free function, rather than only a method, so
/// [`Undermere::new`] can draw it before `Undermere` itself exists, to
/// draw floor ten's biome by the same roll.
pub(super) fn draw_final_boss(seed: RunSeed) -> &'static FinalBoss {
    let mut rng = seed.rng(SeedDomain::new(b"undermere.final_boss"), 0);
    &FINAL_BOSSES[rng.random_range(0..FINAL_BOSSES.len())]
}

impl Undermere {
    /// The one of [`FINAL_BOSSES`] this run's floor ten holds: pure over
    /// `self.seed`, so every call during the run agrees without the draw
    /// being kept anywhere, the same reasoning [`Undermere::lairs`]
    /// already follows for the two mini-bosses. A continued run therefore
    /// keeps the boss it started with, since nothing but the seed ever
    /// decides it.
    pub(crate) fn final_boss(&self) -> &'static FinalBoss {
        draw_final_boss(self.seed)
    }

    /// The throne, grown as an area of its own through the one entry its
    /// own prefab file marks, joined by [`Join::Arch`]: a way through with
    /// nothing to open, as the corridor out of the hall always was.
    pub(super) fn throne_piece(&self) -> Result<Box<dyn Shape>, BuildError> {
        let boss = self.final_boss();
        Ok(Box::new(self.prefabs.grown(boss.piece, Join::Arch)?))
    }
}

/// One of the five mini-bosses a run may draw a lair for.
/// Its name is `monsters.ron`'s and its lair's own `assets/prefabs/`
/// file's. Its floors are the span it may stand on, the content sheet's
/// own "Mini-bosses" row.
pub(crate) struct MiniBoss {
    /// The name both `monsters.ron` and the prefab's `name:` field use.
    pub(crate) name: &'static str,
    /// The shallowest and the deepest floor it may stand on, both
    /// included.
    pub(crate) floors: (u32, u32),
}

/// The five, in the content sheet's own order.
pub(crate) const MINI_BOSSES: [MiniBoss; 5] = [
    MiniBoss { name: "Grubnak the goblin king", floors: (3, 5) },
    MiniBoss { name: "the Widow", floors: (4, 6) },
    MiniBoss { name: "the Choirmaster", floors: (5, 7) },
    MiniBoss { name: "the Drowned Abbess", floors: (6, 8) },
    MiniBoss { name: "the Forgemaster", floors: (7, 9) },
];

impl Undermere {
    /// The two mini-bosses this run draws, and which floor each stands
    /// on, by index into [`MINI_BOSSES`]: never the same boss twice, each
    /// on a floor of its own within its own range. Pure over `self.seed`,
    /// so every call during one run agrees without keeping the draw
    /// anywhere, the same reasoning `generate`'s own per-place stream
    /// follows.
    pub(crate) fn lairs(&self) -> [(usize, u32); 2] {
        let mut rng = self.seed.rng(SeedDomain::new(b"undermere.lairs"), 0);
        let mut order: Vec<usize> = (0..MINI_BOSSES.len()).collect();
        for i in (1..order.len()).rev() {
            order.swap(i, rng.random_range(0..=i));
        }
        let mut chosen: Vec<(usize, u32)> = Vec::new();
        for &boss in order.iter().take(2) {
            let (lo, hi) = MINI_BOSSES[boss].floors;
            let mut floor = rng.random_range(lo..=hi);
            // Its range is always at least three floors wide and only one
            // other boss was drawn before it, so there is always a floor
            // in its own range the other did not take.
            if chosen.iter().any(|(_, f)| *f == floor) {
                floor = (lo..=hi).find(|f| chosen.iter().all(|(_, g)| g != f)).unwrap_or(floor);
            }
            chosen.push((boss, floor));
        }
        [chosen[0], chosen[1]]
    }

    /// The mini-boss this run stands on `floor`, if any does.
    fn lair_on(&self, floor: u32) -> Option<&'static MiniBoss> {
        self.lairs().iter().find(|(_, f)| *f == floor).map(|(boss, _)| &MINI_BOSSES[*boss])
    }

    /// The piece `floor`'s own chain grows if this run draws a lair
    /// there, through the one entry its own prefab file marks, joined by
    /// [`Join::Arch`]: the piece named exactly as its boss is in
    /// `monsters.ron`, so the one name serves both.
    pub(super) fn lair_piece(&self, floor: u32) -> Result<Option<Box<dyn Shape>>, BuildError> {
        match self.lair_on(floor) {
            Some(boss) => Ok(Some(Box::new(self.prefabs.grown(boss.name, Join::Arch)?))),
            None => Ok(None),
        }
    }
}
