//! Benchmarks on the paths that actually cost something: FOV, A*, Dijkstra
//! maps and region labelling, on maps shaped like the ones a game produces.

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use rand::{Rng, SeedableRng, rngs::StdRng};
use rl_core::{Grid2D, Point, Rect, Steps};
use rl_grid::{AStar, BitGrid, DijkstraMap, PathRules, Terrain, TileRegistry, fov, region};

/// A cave-like map: 35% walls, then two smoothing passes so it has rooms
/// and corridors rather than static.
fn cave(width: i32, height: i32, seed: u64) -> (Terrain, TileRegistry) {
    let registry = TileRegistry::standard();
    let (wall, floor) = (registry.expect("wall"), registry.expect("floor"));
    let mut rng = StdRng::seed_from_u64(seed);
    let mut t = Terrain::from_fn(width, height, |p| {
        let edge = p.x == 0 || p.y == 0 || p.x == width - 1 || p.y == height - 1;
        if edge || rng.random_range(0..100) < 35 { wall } else { floor }
    });
    for _ in 0..2 {
        let prev = t.clone();
        for (p, _) in prev.iter() {
            let walls = Steps::Eight.directions().iter().filter(|d| prev.get(p + d.offset()).is_none_or(|id| id == wall)).count();
            t.set(p, if walls >= 5 { wall } else { floor });
        }
    }
    (t, registry)
}

fn open_cell(t: &Terrain, registry: &TileRegistry, rng: &mut StdRng) -> Point {
    let floor = registry.expect("floor");
    loop {
        let p = Point::new(rng.random_range(0..t.width()), rng.random_range(0..t.height()));
        if t.get(p) == Some(floor) {
            return p;
        }
    }
}

fn bench_fov(c: &mut Criterion) {
    let mut group = c.benchmark_group("fov");
    for range in [8, 12, 20] {
        let (t, r) = cave(96, 64, 1);
        let view = t.view(&r);
        let mut rng = StdRng::seed_from_u64(2);
        let origins: Vec<Point> = (0..64).map(|_| open_cell(&t, &r, &mut rng)).collect();
        let mut out = BitGrid::new(96, 64);
        group.bench_with_input(BenchmarkId::new("64_actors_range", range), &range, |b, &range| {
            b.iter(|| {
                for o in &origins {
                    fov::compute(&view, *o, range, &mut out);
                }
                black_box(out.count())
            })
        });
    }
    group.finish();
}

fn bench_astar(c: &mut Criterion) {
    let mut group = c.benchmark_group("astar");
    for (w, h) in [(64, 48), (128, 96), (256, 192)] {
        let (t, r) = cave(w, h, 3);
        let view = t.view(&r);
        let mut rng = StdRng::seed_from_u64(4);
        let pairs: Vec<(Point, Point)> = (0..16).map(|_| (open_cell(&t, &r, &mut rng), open_cell(&t, &r, &mut rng))).collect();
        let mut astar = AStar::new();
        group.bench_with_input(BenchmarkId::new("16_searches", format!("{w}x{h}")), &(w, h), |b, _| {
            b.iter(|| {
                let mut found = 0;
                for (s, g) in &pairs {
                    if astar.find(&view, *s, *g, PathRules::default()).is_some() {
                        found += 1;
                    }
                }
                black_box(found)
            })
        });
    }
    group.finish();
}

fn bench_dijkstra(c: &mut Criterion) {
    let mut group = c.benchmark_group("dijkstra");
    let (t, r) = cave(512, 512, 5);
    let view = t.view(&r);
    let mut rng = StdRng::seed_from_u64(6);
    for side in [64, 128, 256] {
        let goal = open_cell(&t, &r, &mut rng);
        let region = Rect::new(goal.x - side / 2, goal.y - side / 2, side, side);
        let mut map = DijkstraMap::new(region);
        group.bench_with_input(BenchmarkId::new("build_region", side), &side, |b, _| {
            b.iter(|| {
                map.build(&view, [goal], PathRules::default());
                black_box(map.value(goal))
            })
        });
        group.bench_with_input(BenchmarkId::new("safety_rescan", side), &side, |b, _| {
            b.iter(|| {
                map.build(&view, [goal], PathRules::default());
                map.scale(-12, 10);
                map.rescan(&view, PathRules::default());
                black_box(map.minimum())
            })
        });
    }
    group.finish();
}

fn bench_regions(c: &mut Criterion) {
    let mut group = c.benchmark_group("regions");
    for (w, h) in [(96, 64), (256, 256)] {
        let (t, r) = cave(w, h, 7);
        let view = t.view(&r);
        group.bench_with_input(BenchmarkId::new("label", format!("{w}x{h}")), &(w, h), |b, _| {
            b.iter(|| {
                let regions = region::label_regions(&view, |i| view.is_walkable_idx(i), Steps::Eight);
                black_box(regions.count())
            })
        });
    }
    group.finish();
}

criterion_group!(benches, bench_fov, bench_astar, bench_dijkstra, bench_regions);
criterion_main!(benches);
