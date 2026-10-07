---
title: "Is your cache the right size? fliplru 0.3 can tell you"
published: false
description: "A two-generation LRU cache for Rust that counts its own flips and promotions, and turns them into a verdict on its capacity."
tags: rust, performance, caching, opensource
---
Every cache has a capacity, and almost every capacity is a guess. Too small and the cache
thrashes: entries are evicted just before they are needed again. Too large and you pay
memory for nothing. Most LRU caches give you no way to tell which one you have.

[fliplru](https://crates.io/crates/fliplru) is a small, fast LRU cache for Rust, `no_std`
and safe, that measures its own fit. Version 0.3 turns that measurement into a verdict:

```rust
use fliplru::{LruCache, Sizing};
use std::num::NonZeroUsize;

let mut cache = LruCache::new(NonZeroUsize::new(1000).unwrap());
// ... run your workload ...
let stats = cache.stats();
println!("hit ratio {:.1}%", stats.hit_ratio() * 100.0);
match stats.sizing() {
    Sizing::Oversized { needed } => println!("only {needed} entries were ever needed"),
    Sizing::TooSmall => println!("up to 2x the capacity would turn lucky hits into reliable ones"),
    Sizing::MuchTooSmall => println!("several times the capacity would help a lot"),
    Sizing::Thrashing => println!("almost nothing is reused before it is evicted"),
    Sizing::Fits => println!("a bigger cache would gain little"),
    _ => println!("run longer"),
}
```

This post covers how that works, how the verdicts were checked, how fliplru compares with
the popular LRU crates, and the optimizations that did and did not pay off.

## Two generations and a flip

fliplru keeps two hash maps, a *current* generation and a *previous* one. New and recently
used keys go into the current one. When it holds `cap` entries the cache **flips**: the
current generation becomes the previous one, the old previous generation is dropped, and an
empty current generation begins. A lookup that finds its key in the previous generation
moves it back into the current one, so anything still in use survives the next flip.

![How a flip works: the full current generation becomes the previous one, the old previous one is dropped, and a key found in the previous generation moves back](https://raw.githubusercontent.com/ddalton/fliplru/main/docs/diagrams/flip.png)

The design is old (the JavaScript `hashlru` package works the same way), and it has three
useful properties:

- A `get` of a recently used key is a single hash-table lookup, with no linked list to
  update.
- The last `cap` distinct keys you used are always present. Up to `2 * cap` may be, since
  the previous generation holds on to more, so you plan memory for `2 * cap`.
- **Flips measure turnover.** No flips means everything you use fits. Flips approaching
  `accesses / cap` means almost nothing is used twice before it is dropped.

## From a flip count to a verdict

The flip count alone already tells you a lot, but 0.3 adds the numbers around it:

| counter | what it is |
|---|---|
| `hits` | lookups found in the current generation |
| `promotions` | lookups found in the previous generation: hits, but lucky ones, since that generation was about to be dropped |
| `misses` | lookups that found nothing |
| `inserts`, `updates`, `flips` | |
| `peak_entries` | the most entries ever held: the memory the workload actually needed |

**Promotions are the interesting one.** A promotion means the key was reused at a distance
between `cap` and `2 * cap`. Many promotions mean the keys in use slightly outnumber the
capacity, and a modest increase would turn those lucky hits into reliable ones. No
conventional LRU can report this, because it has no second generation to catch the near
misses.

![One lookup: a hit in the current generation, a promotion from the previous one, or a miss, and the counter each one bumps](https://raw.githubusercontent.com/ddalton/fliplru/main/docs/diagrams/lookup.png)

`stats().sizing()` combines these into one of six verdicts. The rules are simple:

- **`NotEnoughData`**: fewer than 10 x cap lookups so far.
- **`Oversized { needed }`**: there were no flips. `needed` is the peak number of entries.
- **`Thrashing`**: the hit ratio is under 10%.
- **`TooSmall`**: more than a quarter of hits were promotions, and either the hit ratio is
  at least 50% or nearly all hits were promotions.
- **`MuchTooSmall`**: more than a quarter of hits were promotions, with a low hit ratio.
- **`Fits`**: anything else.

### Checking the verdicts against the truth

Rules like these are easy to write and easy to get wrong, so I checked them against ground
truth. I ran each test workload at the chosen capacity, then again at half, double and four
times that capacity. If halving costs nothing, the cache was oversized. If doubling gains a
lot, it was too small. If doubling gains little, it fits.

![Hit ratio at half, one, two and four times the capacity for one workload per verdict](https://raw.githubusercontent.com/ddalton/fliplru/main/docs/diagrams/verdicts.png)

The test set was 21 scenarios:

- fixed working sets of 0.3 to 20 times the capacity, in rotation and at random;
- Zipf traffic (a few very popular keys and a long tail) at skews 0.7 to 1.2;
- Zipf traffic interrupted by one-off scans.

Each ran at capacities of 1,000, 10,000 and 100,000. A few of the results:

| workload (cap 10,000) | hit ratio | at 2x cap | at 4x cap | verdict |
|---|---:|---:|---:|---|
| 8,000 keys in rotation | 99.2% | 99.2% | 99.2% | `Oversized { needed: 8000 }` |
| 12,000 keys in rotation | 79.2% | 98.8% | 98.8% | `TooSmall` |
| 50,000 keys at random | 28.0% | 52.1% | 86.7% | `MuchTooSmall` |
| 200,000 keys at random | 7.4% | 14.4% | 27.5% | `Thrashing` |
| Zipf 0.99 over 100,000 keys | 75.4% | 82.6% | 89.0% | `Fits` |
| the same, with scans | 18.5% | 21.7% | 23.7% | `Fits` (scans cannot be cached) |

The first version of the rules called the 50,000-key case `TooSmall` ("raise it a
little"), when it really needed about four times the capacity. That is why there are two
"too small" verdicts. With the split, all 21 scenarios get the right verdict at all three
capacities. One case per verdict is now a test in the crate.

The counters are plain integers updated as the cache runs. Measured against 0.2, they cost
nothing measurable on lookups and about 0.4 ns per `put`.

## How fast is it?

Speed got a lot of attention in 0.2. Here is fliplru against the most-used LRU crates on an
Apple M1, with a capacity of 100,000 and times in nanoseconds per operation:

| cache | get (all hits) | put | Zipf, integer keys | loop, String keys |
|---|---:|---:|---:|---:|
| fliplru | 7.1 | 16.3 | 15.4 | 84.7 |
| fliplru + Fx hasher | 5.3 | 6.0 | 14.2 | 85.2 |
| lru 0.18 | 8.7 | 36.5 | 16.7 | 106.4 (every request misses) |
| schnellru 0.2 | 7.3 | 25.5 | 14.8 | 100.8 (every request misses) |
| quick_cache 0.7 | 7.7 | 31.2 | 19.9 | 74.9 |

Read-throughs use each crate's own best "get or insert" method. These numbers were
measured on 0.2; the counters added in 0.3 cost about 0.4 ns per `put`. fliplru is clearly fastest
on puts, and on gets with a faster hasher. On realistic read traffic it is level with the
best; with String keys, the top three trade places from run to run.

That is also where I will be careful. **Speed is not the main reason to choose fliplru.**
When a miss costs a database query, a few points of hit ratio matter far more than a few
nanoseconds per hit. And at equal memory, fliplru's hit ratio is lower than a classic LRU's
(78.7% against 82.0% on Zipf traffic), because a flip drops a whole generation at once
where an LRU drops one key at a time. If hit ratio is what you need,
[quick_cache](https://crates.io/crates/quick_cache) (S3-FIFO) is better, especially on
traffic with one-off scans.

So fliplru fits best where operations are very frequent and misses are cheap (memoizing
small computations, interning, write-heavy caches), or wherever you want the cache to tell
you whether it is the right size.

## What made 0.2 faster, and what didn't work

The optimizations that paid off:

- **Hashing once.** The two generations are now hashbrown `HashTable`s sharing one hasher.
  A key is hashed once per operation, and that hash is reused for every probe and insert.
- **Fewer table operations on a promotion.** Moving a key from the previous generation used
  to take four lookups; now it takes two.
- **Flips that reuse memory.** A flip clears the retired table rather than allocating a new
  one.
- **Read-through methods.** `get_or_insert_with` does a lookup and a fill-on-miss with one
  hash, and `get_or_insert_with_ref` accepts a `&str` for `String` keys, so a hit builds no
  owned key.

The fast path has one Rust-specific wrinkle. "Return the value if it is in the current
generation, otherwise look in the previous one" is a known limitation of today's borrow
checker: returning a borrow from one branch, then using the map again in the other.
[polonius-the-crab](https://crates.io/crates/polonius-the-crab) makes that single-lookup
version expressible in safe Rust, with no runtime cost.

The experiments that did not pay off:

- **One table instead of two,** with each entry tagged by its generation. Promotions became
  a single in-place write, and gets got 15% faster. But dropping a generation from a shared
  table leaves deleted markers that lookups have to skip, and puts became **2.2x slower**.
  The second table's probe that this was meant to save turned out to be cheap: hashbrown
  checks a whole group of slots in one SIMD comparison.
- **A "probation" generation,** the admission idea behind S3-FIFO: new keys must prove
  themselves before entering the main cache. It nearly matched S3-FIFO on scans, but it
  gained little on ordinary traffic. It also broke fliplru's guarantee that the last `cap`
  keys are always present, which is the property the sizing signal relies on.

## Benchmarking lessons

- **Run order matters.** On a laptop, the order in which benchmarks run moved results by
  10-20%. The harness in the repo runs every cache in every round and rotates the order
  between rounds.
- **Report hit ratio next to time.** A cache that misses more is doing different work. On a
  loop slightly larger than the capacity, a classic LRU misses every request, and its time
  there measures misses, not hits.
- **Use each crate's best API.** Comparing your `get_or_insert` against someone else's
  `get` followed by `put` is not a fair comparison.

## Try it

```sh
cargo add fliplru
```

The docs are on [docs.rs](https://docs.rs/fliplru), and the benchmark harness is in the
[repository](https://github.com/ddalton/fliplru): `cd bench && cargo run --release`.
If `stats().sizing()` tells you something surprising about one of your caches, I would like
to hear about it.
