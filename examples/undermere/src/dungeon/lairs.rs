//! The bosses' halls: floor ten's throne, drawn from three, the way in
//! laid farthest from it, and the lair a mini-boss's floor stamps.

use super::*;

/// Where the throne's mouth lets out, emitted by [`ThroneHall`] for the
/// way in to be measured from.
struct Mouth(Point);

/// Lays the throne at the north end of floor ten, after the floor is grown
/// and before the lakes and the way in: a hall a cell larger than the
/// piece all round, carved over whatever grew there, the piece in the
/// middle of it facing south down the floor, and a corridor from under its
/// mouth to the nearest cell of an area clear of the hall. A floor with no
/// area clear of the hall is refused, since nothing would reach the
/// throne.
///
/// The hall's ring of floor is what keeps the floor whole: an area or a
/// passage the hall cut through meets the ring, and the ring goes round
/// the piece. Carved rather than grown, so the areas it cuts keep their
/// cells as growth recorded them; laying the throne as a piece growth
/// joins through its own entries replaces this.
pub(super) struct ThroneHall {
    /// The throne, as drawn: its mouth in the middle of its bottom row.
    pub(super) piece: Prefab,
    /// What the hall is floored with.
    pub(super) floor: TileId,
}

impl Pass<BaseContext> for ThroneHall {
    fn name(&self) -> &'static str {
        "throne_hall"
    }

    fn phase(&self) -> Phase {
        Phase::Structures
    }

    fn apply(&self, ctx: &mut BaseContext) -> Result<(), BuildError> {
        let (w, h) = (self.piece.width(), self.piece.height());
        let bounds = ctx.terrain().bounds();
        // The hall, and a row of wall all round it at the map's edge.
        if w + 4 > bounds.width || h + 5 > bounds.height {
            return Err(BuildError::new("throne_hall", format!("a {w}x{h} throne does not fit a {}x{} floor", bounds.width, bounds.height)));
        }
        let x = ctx.rng().random_range(1..=bounds.width - w - 3);
        let hall = Rect::new(x, 1, w + 2, h + 2);
        for p in hall.cells() {
            ctx.terrain_mut().set(p, self.floor);
        }
        StampPrefab { name: "throne", prefab: self.piece.clone(), at: Placement::At(Point::new(hall.x + 1, hall.y + 1)), orient: Orient::Fixed }.apply(ctx)?;
        let mouth = Point::new(hall.x + 1 + w / 2, hall.y + 1 + h);
        // Refused rather than laid shut away: a hall with nothing to join
        // its mouth to is a floor the walk to the throne cannot cross, and
        // a change to the floor's size or the throne's that brings it about
        // should fail where it happens.
        if !join_mouth(ctx, hall, mouth, self.floor) {
            return Err(BuildError::new("throne_hall", format!("no area clear of the {w}x{h} throne's hall to join its mouth to")));
        }
        ctx.emit(Mouth(mouth));
        Ok(())
    }
}

/// Carves `tile` from `mouth`, under the bottom row of `hall`, to the
/// nearest cell of a grown area outside it: straight down out of the
/// hall, along the row under it, then up or down into the area, never
/// back into the hall, so a cell straight above it is never the one
/// joined. `false`, carving nothing, when no such cell lies outside the
/// hall.
fn join_mouth(ctx: &mut BaseContext, hall: Rect, mouth: Point, tile: TileId) -> bool {
    let under = hall.bottom();
    let above = |p: Point| p.y < under && (hall.x..hall.x + hall.width).contains(&p.x);
    let nearest = ctx
        .outputs()
        .iter::<Area>()
        .flat_map(|area| area.cells.iter().copied())
        .filter(|p| !hall.contains(*p) && !above(*p))
        .min_by_key(|p| (geometry::manhattan(*p, mouth), *p));
    let Some(to) = nearest else {
        return false;
    };
    let corridor = (mouth.y..=under)
        .map(|y| Point::new(mouth.x, y))
        .chain((mouth.x.min(to.x)..=mouth.x.max(to.x)).map(|x| Point::new(x, under)))
        .chain((under.min(to.y)..=under.max(to.y)).map(|y| Point::new(to.x, y)))
        .collect::<Vec<_>>();
    for p in corridor {
        ctx.terrain_mut().set(p, tile);
    }
    true
}

/// Emits as the way in the cell farthest by walking from the throne's
/// mouth, so the throne is where the floor ends and the walk to it is the
/// whole of floor ten; fails, for the floor to be grown again, when that
/// cell is nearer a piece than the population keeps from the way in.
pub(super) struct FarthestFromTheThrone;

impl Pass<BaseContext> for FarthestFromTheThrone {
    fn name(&self) -> &'static str {
        "farthest_from_the_throne"
    }

    fn phase(&self) -> Phase {
        Phase::Exits
    }

    fn apply(&self, ctx: &mut BaseContext) -> Result<(), BuildError> {
        let mouth = ctx.outputs().first::<Mouth>().ok_or_else(|| BuildError::new("farthest_from_the_throne", "no throne laid"))?.0;
        let pieces = pieces(ctx);
        let far = farthest(ctx, mouth, |p| pieces.iter().any(|r| r.contains(p)))
            .ok_or_else(|| BuildError::new("farthest_from_the_throne", "nowhere to walk to from the throne"))?;
        // A floor that winds back on itself can leave its farthest cell
        // beside the throne's wall; grown again rather than arriving
        // within the population's distance of the boss.
        let clear = pieces.iter().map(|r| geometry::chebyshev(far, geometry::clamp_to(far, *r))).min().unwrap_or(i32::MAX);
        if clear < MIN_DISTANCE_FROM_ENTRY {
            return Err(BuildError::new("farthest_from_the_throne", format!("the farthest cell from the throne is {clear} from a piece")));
        }
        ctx.emit(StartPoint(far));
        Ok(())
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
    /// What floor ten is filled with before anything is carved, when this
    /// one is drawn.
    pub(super) wall: &'static str,
    /// What floor ten's rooms and its own hall are carved as, when this
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
}

/// One of the five mini-bosses a run may draw a lair for.
/// Its name is `monsters.ron`'s and its lair's own `assets/prefabs/`
/// file's, its floors the span it may stand on, and its margin the tile
/// its lair's hall is floored with outside the piece's own footprint,
/// matching its own piece's look rather than whatever biome its floor
/// draws: a lair is the one place in a family that always looks like
/// itself, cave moss or crypt bone, however the floor round it reads;
/// only Grubnak's is new, since no band his own floors fall in is a hall.
pub(crate) struct MiniBoss {
    /// The name both `monsters.ron` and the prefab's `name:` field use.
    pub(crate) name: &'static str,
    /// The shallowest and the deepest floor it may stand on, both
    /// included: `content-sheet-monsters.md`'s own "Mini-bosses" row.
    pub(crate) floors: (u32, u32),
    /// What floors the margin round its lair's own piece.
    margin: &'static str,
}

/// The five, in the content sheet's own order. `pub(crate)` so
/// `population.rs`'s own tests can read a boss's name and floors without
/// a second list of them to keep in step.
pub(crate) const MINI_BOSSES: [MiniBoss; 5] = [
    MiniBoss { name: "Grubnak the goblin king", floors: (3, 5), margin: "goblin hall floor" },
    MiniBoss { name: "the Widow", floors: (4, 6), margin: "moss" },
    MiniBoss { name: "the Choirmaster", floors: (5, 7), margin: "crypt floor" },
    MiniBoss { name: "the Drowned Abbess", floors: (6, 8), margin: "flooded floor" },
    MiniBoss { name: "the Forgemaster", floors: (7, 9), margin: "ash" },
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

    /// The mini-boss and its lair's own margin tile this run stands on
    /// `floor`, if any does.
    fn lair_on(&self, floor: u32) -> Option<(&'static MiniBoss, TileId)> {
        self.lairs().iter().find(|(_, f)| *f == floor).map(|(boss, _)| (&MINI_BOSSES[*boss], self.tile(MINI_BOSSES[*boss].margin)))
    }

    /// The [`Lair`] pass `floor`'s own chain stamps, if this run draws one
    /// there: the piece named exactly as its boss is in `monsters.ron`,
    /// so the one name serves both.
    pub(super) fn lair_pass(&self, floor: u32) -> Result<Option<Lair>, BuildError> {
        match self.lair_on(floor) {
            Some((boss, margin)) => Ok(Some(Lair { name: boss.name, piece: self.prefabs.piece(boss.name)?, margin })),
            None => Ok(None),
        }
    }
}

/// Lays a mini-boss's lair over the grown floor, before its lakes: a hall
/// a cell larger than the piece all round, carved over whatever stood
/// there, the piece stamped fixed in its centre, and a corridor from its
/// mouth to the nearest area clear of it, as [`ThroneHall`] lays the
/// throne. Laying the lair as a piece growth joins through its own entries
/// replaces this.
pub(super) struct Lair {
    /// A stable name, so two lairs, or the throne, draw different streams.
    name: &'static str,
    /// The piece.
    piece: Prefab,
    /// What the hall is floored with outside the piece's own footprint.
    margin: TileId,
}

impl Pass<BaseContext> for Lair {
    fn name(&self) -> &'static str {
        self.name
    }

    fn phase(&self) -> Phase {
        Phase::Structures
    }

    fn apply(&self, ctx: &mut BaseContext) -> Result<(), BuildError> {
        let (w, h) = (self.piece.width(), self.piece.height());
        let bounds = ctx.terrain().bounds();
        // The hall, a row of wall all round it at the map's edge, and one
        // more row below for the corridor out of its mouth.
        if w + 4 > bounds.width || h + 5 > bounds.height {
            return Err(BuildError::new(self.name, format!("a {w}x{h} lair does not fit a {}x{} floor", bounds.width, bounds.height)));
        }
        let x = ctx.rng().random_range(1..=bounds.width - w - 3);
        let y = ctx.rng().random_range(1..=bounds.height - h - 4);
        let hall = Rect::new(x, y, w + 2, h + 2);
        for p in hall.cells() {
            ctx.terrain_mut().set(p, self.margin);
        }
        StampPrefab { name: self.name, prefab: self.piece.clone(), at: Placement::At(Point::new(hall.x + 1, hall.y + 1)), orient: Orient::Fixed }.apply(ctx)?;
        let mouth = Point::new(hall.x + 1 + w / 2, hall.y + 1 + h);
        if !join_mouth(ctx, hall, mouth, self.margin) {
            return Err(BuildError::new(self.name, format!("no area clear of the {w}x{h} lair's hall to join its mouth to")));
        }
        Ok(())
    }
}
