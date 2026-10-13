//! A seed range played on every core at once.

use std::any::Any;
use std::ops::Range;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

/// Runs `check` on every seed of `seeds`, spread over as many threads as
/// the machine has cores.
///
/// The seeds are independent runs, so the order they finish in is no
/// part of what a test says. A failure stops the rest from starting and is
/// raised again here, from the lowest seed that failed and naming it, so
/// the message is the one a plain loop would have met first.
pub fn par_seeds(seeds: Range<u64>, check: impl Fn(u64) + Sync) {
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
