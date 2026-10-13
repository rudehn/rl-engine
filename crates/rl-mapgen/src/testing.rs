//! Test support: a seed range played on every core at once.

use std::any::Any;
use std::ops::Range;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

/// How many seeds of a range an everyday run plays.
pub(crate) const QUICK: u64 = 30;

/// The seeds a generation test plays: the first [`QUICK`] of `full`, or all
/// of `full` when the environment sets `RL_FULL_SEEDS`, as CI does.
///
/// Every seed is a whole run generated from scratch, and a property that
/// holds on thirty of them is as good a tripwire for a change as one that
/// holds on five hundred. The wide range is for the release and for CI,
/// where it catches the rare seed. A test about something rare, such as an
/// entry being drawn at least once, keeps its own range and says why.
pub(crate) fn seeds(full: Range<u64>) -> Range<u64> {
    if std::env::var_os("RL_FULL_SEEDS").is_some() { full } else { full.start..full.end.min(full.start + QUICK) }
}

/// Runs `check` on every seed of `seeds`, spread over as many threads as
/// the machine has cores.
///
/// The seeds are independent runs, so the order they finish in is no
/// part of what a test says. A failure stops the rest from starting and is
/// raised again here, from the lowest seed that failed and naming it, so
/// the message is the one a plain loop would have met first.
pub(crate) fn par_seeds(seeds: Range<u64>, check: impl Fn(u64) + Sync) {
    let next = AtomicU64::new(seeds.start);
    let failed: Mutex<Option<(u64, String)>> = Mutex::new(None);
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                loop {
                    let s = next.fetch_add(1, Ordering::Relaxed);
                    if s >= seeds.end || failed.lock().unwrap().is_some() {
                        return;
                    }
                    if let Err(payload) = catch_unwind(AssertUnwindSafe(|| check(s))) {
                        let mut slot = failed.lock().unwrap();
                        if slot.as_ref().is_none_or(|(first, _)| s < *first) {
                            *slot = Some((s, message(payload.as_ref())));
                        }
                    }
                }
            });
        }
    });
    if let Some((s, text)) = failed.into_inner().unwrap() {
        panic!("seed {s} failed: {text}");
    }
}

/// The text a panic carried.
fn message(payload: &(dyn Any + Send)) -> String {
    payload.downcast_ref::<String>().cloned().or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_owned())).unwrap_or_else(|| "a panic".to_owned())
}
