# fliplru

[![crates.io](https://img.shields.io/crates/v/fliplru.svg)](https://crates.io/crates/fliplru)
[![docs.rs](https://docs.rs/fliplru/badge.svg)](https://docs.rs/fliplru)

A fast LRU cache for Rust with a built-in signal for sizing it. `no_std`, safe Rust, one
dependency (hashbrown).

```rust
use fliplru::LruCache;
use std::num::NonZeroUsize;

let mut cache = LruCache::new(NonZeroUsize::new(1000).unwrap());
cache.put("apple", 3);
assert_eq!(cache.get(&"apple"), Some(&3));

// read-through: compute the value only on a miss
let price = *cache.get_or_insert_with("pear", || 5);

// is 1000 the right size? see "Sizing the cache" below
println!("{:?}", cache.stats().sizing());
```

## How it works

fliplru keeps two hash maps: the **current** generation and the **previous** one. Keys you
put or use live in the current generation. When it reaches `cap` entries, the cache
**flips**: the current generation becomes the previous one, the old previous generation is
dropped, and a new, empty current generation starts. A key found in the previous
generation is moved back to the current one, so anything still in use survives the next
flip.

That gives:

- **A fast `get`**: one hash lookup when the key is in the current generation. Every
  operation hashes the key once.
- **A guarantee**: the last `cap` distinct keys used are always in the cache.
- **A memory cost**: up to `2 * cap` entries can be held (both generations), so plan for
  `2 * cap`.
- **A sizing signal**: the number of flips, and the statistics around it (`stats()`).

## Sizing the cache

Run your workload, then ask the cache:

```rust
use fliplru::{LruCache, Sizing};
use std::num::NonZeroUsize;

let mut cache = LruCache::new(NonZeroUsize::new(100).unwrap());
for i in 0..10_000 {
    cache.get_or_insert_with(i % 50, || i); // 50 keys in use
}
let stats = cache.stats();
println!("hit ratio {:.1}%, {} flips", stats.hit_ratio() * 100.0, stats.flips);
match stats.sizing() {
    Sizing::Oversized { needed } => println!("only {needed} entries were ever needed"),
    Sizing::TooSmall => println!("hits depend on luck: up to 2x the capacity would fix it"),
    Sizing::MuchTooSmall => println!("several times the capacity would help a lot"),
    Sizing::Thrashing => println!("almost nothing is reused: far too small, or no reuse to find"),
    Sizing::Fits => println!("a bigger cache would gain little"),
    Sizing::NotEnoughData => println!("run longer"),
    _ => {}
}
```

`stats()` returns hits, **promotions** (hits on keys found in the previous generation: they
survived only because it had not been dropped yet), misses, inserts, updates, flips, and the
most entries ever held. Keeping these counts costs nothing measurable on lookups and about
0.4 ns per `put`.

The verdicts were calibrated on fixed working sets from 0.3 to 20 times the capacity,
Zipf traffic and Zipf with scans, at capacities from 1,000 to 100,000, checking each verdict
against the hit ratio the same traffic gets at half, double and four times the capacity
(`tests/sizing.rs` keeps one case per verdict).

The flip count alone tells the same story. Compare it with the number of accesses:

| flips | what it means |
|---|---|
| 0 | everything you use fits: the cache may be bigger than it needs to be |
| close to `accesses / cap` | almost nothing is used twice before it is dropped: the cache is too small to help |
| in between | the cache is working; fewer flips per access means more reuse |

```rust
use fliplru::LruCache;
use std::num::NonZeroUsize;

// 5 keys used round-robin, cache of 2: every access misses, and the flips say so
let mut small = LruCache::new(NonZeroUsize::new(2).unwrap());
for i in 0..20 { small.get_or_insert_with(i % 5, || i); }
assert_eq!(small.get_flips(), 9);

// a cache of 5: everything fits, no flips
let mut big = LruCache::new(NonZeroUsize::new(5).unwrap());
for i in 0..20 { big.get_or_insert_with(i % 5, || i); }
assert_eq!(big.get_flips(), 0);
```

Use `reset()` to start counting again, for example once per reporting period.

## API

| method | |
|---|---|
| `new(cap)` | a cache keeping at least the last `cap` keys |
| `with_hasher(cap, hasher)` | the same, with your own hasher (see below) |
| `get(&k)`, `get_mut(&k)` | the value, if present (a key found in the previous generation moves to the current one, so these take `&mut self`) |
| `put(k, v)` | insert or update; returns the old value |
| `get_or_insert_with(k, f)` | read-through: the value, inserting `f()` first on a miss; one hash instead of a `get` plus a `put` |
| `get_or_insert_with_ref(&q, f)` | the same, looking up by reference: for `String` keys pass a `&str`, and no `String` is built on a hit |
| `stats()` | hits, promotions, misses, inserts, updates, flips, peak entries, and `sizing()`: a verdict on the capacity |
| `get_flips()`, `reset()` | the flip count, and zeroing every counter |
| `len()`, `is_empty()`, `cap()` | |

**Choosing a hasher.** The default is hashbrown's (foldhash), which is fast and resists
collision attacks. For integer keys you control, an Fx hasher (the `rustc-hash` crate) is
faster still:

```rust
use fliplru::LruCache;
use std::num::NonZeroUsize;

let mut cache: LruCache<u64, String, rustc_hash::FxBuildHasher> =
    LruCache::with_hasher(NonZeroUsize::new(1000).unwrap(), rustc_hash::FxBuildHasher);
```

fliplru is single-threaded (`&mut self` methods). To share one across threads, put it
behind a `Mutex`.

## Performance

From `bench/` (stable Rust: `cd bench && cargo run --release`), on an Apple M1, cap
100,000. Every cache runs each workload in every round, in an order that rotates between
rounds, and the median of 9 rounds is shown. Read-throughs use each crate's own best
method. Hit ratios are next to the times, since a cache that misses more does different
work.

**Speed, ns per operation (hit ratio)**

| cache | get (all hits) | put | Zipf, integer keys | Zipf, String keys | loop, String keys |
|---|---:|---:|---:|---:|---:|
| **fliplru** | 7.1 | **16.3** | 15.4 (79%) | 90.2 (78%) | 84.7 (49%) |
| **fliplru + Fx hasher** | **5.3** | **6.0** | **14.2** (79%) | 90.3 (78%) | 85.2 (49%) |
| fliplru 0.1.6 | 7.3 | 20.1 | 21.3 (79%) | 95.2 (78%) | 141.2 (49%) |
| lru 0.18 | 8.7 | 36.5 | 16.7 (76%) | 102.1 (76%) | 106.4 (0%) |
| hashlink 0.12 | 8.7 | 33.1 | 16.2 (76%) | 94.8 (76%) | 96.5 (0%) |
| schnellru 0.2 | 7.3 | 25.5 | 14.8 (76%) | 82.2 (76%) | 100.8 (0%) |
| quick_cache 0.7 (S3-FIFO) | 7.7 | 31.2 | 19.9 (79%) | 87.7 (79%) | **74.9** (48%) |

- **Zipf**: requests over 1M keys where a few are very popular, read-through on a cache of
  100,000. **Loop**: keys cycling through 1.5 x cap, where a classic LRU misses every
  request.
- fliplru is fastest on puts and, with the Fx hasher, on gets. On Zipf traffic fliplru,
  schnellru and hashlink are within noise of each other with integer keys, and the top
  three are within noise with String keys: that column changes order from run to run
  (fliplru has measured 74-92 ns there). Expect about ±10% between runs on a laptop.

**Hit ratio at equal memory** (each cache holds at most 200,000 entries; fliplru, which
holds up to `2 * cap`, gets cap 100,000)

| cache | Zipf 0.99 | Zipf 0.7 | Zipf + scans | loop |
|---|---:|---:|---:|---:|
| fliplru | 78.7% | 41.0% | 20.3% | 0.0% |
| lru 0.18 | 82.0% | 48.1% | 20.3% | 0.0% |
| schnellru 0.2 | 82.0% | 48.1% | 20.3% | 0.0% |
| quick_cache 0.7 (S3-FIFO) | **83.5%** | **53.8%** | **25.0%** | **67.6%** |

At equal memory fliplru keeps fewer useful keys than a classic LRU: a flip drops a whole
generation at once, where an LRU drops one key at a time. And like any LRU it is not scan
resistant. If hit ratio matters more than speed and the sizing signal, use
[quick_cache](https://crates.io/crates/quick_cache) (S3-FIFO) or
[moka](https://crates.io/crates/moka) (W-TinyLFU, concurrent).

## Minimum Rust version

1.85 (from hashbrown 0.17).

## License

MIT
