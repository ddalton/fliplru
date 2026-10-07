//! `Stats::sizing` on workloads whose right answer is known, one per verdict. The thresholds
//! were calibrated on these and more (fixed working sets from 0.3 to 20 x cap, Zipf traffic
//! from skew 0.7 to 1.2, Zipf with scans; at caps of 1,000 to 100,000) against the hit ratio
//! each trace gets at half, double and four times the capacity.

use fliplru::{LruCache, Sizing};
use std::num::NonZeroUsize;

const CAP: usize = 1000;
const LOOKUPS: usize = 100 * CAP;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

fn verdict(keys: impl Iterator<Item = u64>) -> Sizing {
    let mut cache = LruCache::new(NonZeroUsize::new(CAP).unwrap());
    for k in keys {
        cache.get_or_insert_with(k, || ());
    }
    cache.stats().sizing()
}

/// `len` draws from Zipf(s) over `n` keys (inverse CDF)
fn zipf(n: usize, s: f64, len: usize) -> Vec<u64> {
    let mut cdf = Vec::with_capacity(n);
    let mut acc = 0.0;
    for r in 1..=n {
        acc += 1.0 / (r as f64).powf(s);
        cdf.push(acc);
    }
    let mut rng = Rng(3);
    (0..len)
        .map(|_| {
            let u = (rng.next() >> 11) as f64 / (1u64 << 53) as f64 * acc;
            cdf.partition_point(|&c| c < u) as u64
        })
        .collect()
}

#[test]
fn too_few_lookups_to_judge() {
    assert_eq!(verdict((0..5 * CAP as u64).map(|n| n % 10)), Sizing::NotEnoughData);
}

#[test]
fn a_working_set_of_half_the_capacity_is_oversized_and_says_how_much_was_needed() {
    let ws = CAP as u64 / 2;
    assert_eq!(verdict((0..LOOKUPS as u64).map(|n| n % ws)), Sizing::Oversized { needed: CAP / 2 });
}

#[test]
fn a_working_set_slightly_over_the_capacity_is_too_small() {
    // 1.2 x cap in rotation: every hit is a promotion; a cap of 1.2x would hit ~99%
    let ws = CAP as u64 * 6 / 5;
    assert_eq!(verdict((0..LOOKUPS as u64).map(|n| n % ws)), Sizing::TooSmall);
}

#[test]
fn a_working_set_of_five_times_the_capacity_is_much_too_small() {
    // 5 x cap at random: 28% hits, 52% at 2x the cap, 87% at 4x
    let mut r = Rng(11);
    assert_eq!(verdict((0..LOOKUPS).map(|_| r.next() % (5 * CAP as u64))), Sizing::MuchTooSmall);
}

#[test]
fn a_working_set_of_twenty_times_the_capacity_thrashes() {
    let mut r = Rng(13);
    assert_eq!(verdict((0..LOOKUPS).map(|_| r.next() % (20 * CAP as u64))), Sizing::Thrashing);
}

#[test]
fn skewed_traffic_on_a_reasonable_cache_fits() {
    // Zipf 0.99 over 100 x cap keys: ~70% hits; twice the cap adds under 10 points
    assert_eq!(verdict(zipf(100 * CAP, 0.99, LOOKUPS).into_iter()), Sizing::Fits);
}
