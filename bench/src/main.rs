//! fliplru against other LRU caches: speed and hit ratio. Stable Rust:
//!
//!     cd bench && cargo run --release
//!
//! Every cache runs every workload in every round, in an order that rotates each round, and
//! the median of the rounds is reported: on a laptop, running one cache after another lets
//! warm-up and heat skew the results by 10-20%. A read-through ("get, compute on a miss")
//! uses each crate's own best method, taking the key by reference where it can. Hit ratios
//! are printed next to the times: a cache that misses more does different work.

use std::hash::Hash;
use std::hint::black_box;
use std::num::NonZeroUsize;
use std::time::Instant;

const CAP: usize = 100_000;
const OPS: usize = 4_000_000;
const ROUNDS: usize = 9;

trait Key: Hash + Eq + Clone + 'static {
    fn schnellru_read(m: &mut schnellru::LruMap<Self, u64, schnellru::ByLength>, k: &Self) -> bool;
}
impl Key for u64 {
    fn schnellru_read(m: &mut schnellru::LruMap<u64, u64, schnellru::ByLength>, k: &u64) -> bool {
        let mut hit = true;
        black_box(m.get_or_insert(*k, || {
            hit = false;
            1
        }));
        hit
    }
}
impl Key for String {
    fn schnellru_read(m: &mut schnellru::LruMap<String, u64, schnellru::ByLength>, k: &String) -> bool {
        let mut hit = true;
        black_box(m.get_or_insert(k.as_str(), || {
            hit = false;
            1
        }));
        hit
    }
}

/// One interface over every crate: a read-through (`read`, returning whether it hit), a
/// plain `get`, and `put`.
trait Cache<K> {
    fn read(&mut self, k: &K) -> bool;
    fn get(&mut self, k: &K) -> bool;
    fn put(&mut self, k: K);
}

struct Flip<K: Key>(fliplru::LruCache<K, u64>);
struct FlipFx<K: Key>(fliplru::LruCache<K, u64, rustc_hash::FxBuildHasher>);
struct Flip016<K: Key>(fliplru_016::LruCache<K, u64>);
struct Lru<K: Key>(lru::LruCache<K, u64>);
struct Hashlink<K: Key>(hashlink::LruCache<K, u64>);
struct Schnell<K: Key>(schnellru::LruMap<K, u64, schnellru::ByLength>);
struct Quick<K: Key>(quick_cache::unsync::Cache<K, u64>);

macro_rules! read_with {
    ($self:ident, $call:expr) => {{
        let mut hit = true;
        let _ = black_box($call(&mut hit));
        hit
    }};
}

impl<K: Key> Cache<K> for Flip<K> {
    fn read(&mut self, k: &K) -> bool {
        read_with!(self, |hit: &mut bool| *self.0.get_or_insert_with_ref(k, || { *hit = false; 1 }))
    }
    fn get(&mut self, k: &K) -> bool { black_box(self.0.get(k)).is_some() }
    fn put(&mut self, k: K) { self.0.put(k, 1); }
}
impl<K: Key> Cache<K> for FlipFx<K> {
    fn read(&mut self, k: &K) -> bool {
        read_with!(self, |hit: &mut bool| *self.0.get_or_insert_with_ref(k, || { *hit = false; 1 }))
    }
    fn get(&mut self, k: &K) -> bool { black_box(self.0.get(k)).is_some() }
    fn put(&mut self, k: K) { self.0.put(k, 1); }
}
impl<K: Key> Cache<K> for Flip016<K> {
    // 0.1.6 has no read-through: get, then put on a miss
    fn read(&mut self, k: &K) -> bool {
        if self.get(k) { true } else { self.put(k.clone()); false }
    }
    fn get(&mut self, k: &K) -> bool { black_box(self.0.get(k)).is_some() }
    fn put(&mut self, k: K) { self.0.put(k, 1); }
}
impl<K: Key> Cache<K> for Lru<K> {
    fn read(&mut self, k: &K) -> bool {
        read_with!(self, |hit: &mut bool| *self.0.get_or_insert_ref(k, || { *hit = false; 1 }))
    }
    fn get(&mut self, k: &K) -> bool { black_box(self.0.get(k)).is_some() }
    fn put(&mut self, k: K) { self.0.put(k, 1); }
}
impl<K: Key> Cache<K> for Hashlink<K> {
    // no read-through method: get, then insert on a miss
    fn read(&mut self, k: &K) -> bool {
        if self.get(k) { true } else { self.put(k.clone()); false }
    }
    fn get(&mut self, k: &K) -> bool { black_box(self.0.get(k)).is_some() }
    fn put(&mut self, k: K) { self.0.insert(k, 1); }
}
impl<K: Key> Cache<K> for Schnell<K> {
    fn read(&mut self, k: &K) -> bool { K::schnellru_read(&mut self.0, k) }
    fn get(&mut self, k: &K) -> bool { black_box(self.0.get(k)).is_some() }
    fn put(&mut self, k: K) { self.0.insert(k, 1); }
}
impl<K: Key> Cache<K> for Quick<K> {
    fn read(&mut self, k: &K) -> bool {
        let mut hit = true;
        let _ = black_box(self.0.get_or_insert_with(k, || {
            hit = false;
            Ok::<u64, ()>(1)
        }));
        hit
    }
    fn get(&mut self, k: &K) -> bool { black_box(self.0.get(k)).is_some() }
    fn put(&mut self, k: K) { self.0.insert(k, 1); }
}

const NAMES: [&str; 7] = [
    "fliplru (this version)",
    "fliplru + Fx hasher",
    "fliplru 0.1.6",
    "lru 0.18",
    "hashlink 0.12",
    "schnellru 0.2",
    "quick_cache 0.7 (S3-FIFO)",
];

fn make<K: Key>(i: usize, cap: usize) -> Box<dyn Cache<K>> {
    let nz = NonZeroUsize::new(cap).unwrap();
    match i {
        0 => Box::new(Flip(fliplru::LruCache::new(nz))),
        1 => Box::new(FlipFx(fliplru::LruCache::with_hasher(nz, rustc_hash::FxBuildHasher))),
        2 => Box::new(Flip016(fliplru_016::LruCache::new(nz))),
        3 => Box::new(Lru(lru::LruCache::new(nz))),
        4 => Box::new(Hashlink(hashlink::LruCache::new(cap))),
        5 => Box::new(Schnell(schnellru::LruMap::new(schnellru::ByLength::new(cap as u32)))),
        _ => Box::new(Quick(quick_cache::unsync::Cache::new(cap))),
    }
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

/// `len` draws from a Zipf(s) distribution over `n` keys (a few keys very popular, a long
/// tail), with key ids shuffled so popularity does not follow the id.
fn zipf(n: usize, s: f64, len: usize, seed: u64) -> Vec<u64> {
    let mut cdf = Vec::with_capacity(n);
    let mut acc = 0.0;
    for r in 1..=n {
        acc += 1.0 / (r as f64).powf(s);
        cdf.push(acc);
    }
    let mut rng = Rng(seed);
    let mut ids: Vec<u64> = (0..n as u64).collect();
    for i in (1..n).rev() {
        ids.swap(i, (rng.next() % (i as u64 + 1)) as usize);
    }
    (0..len)
        .map(|_| {
            let u = (rng.next() >> 11) as f64 / (1u64 << 53) as f64 * acc;
            ids[cdf.partition_point(|&c| c < u)]
        })
        .collect()
}

fn names(n: usize) -> Vec<String> {
    (0..n as u64).map(|i| format!("user:{:016x}:session", i.wrapping_mul(0x9E3779B97F4A7C15))).collect()
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

/// ns per operation and hit ratio of one run
fn time(ops: usize, f: impl FnOnce() -> usize) -> (f64, f64) {
    let t = Instant::now();
    let hits = f();
    (t.elapsed().as_nanos() as f64 / ops as f64, hits as f64 / ops as f64)
}

fn speed() {
    let ints = zipf(1_000_000, 0.99, OPS, 42);
    let strs = names(1_000_000);
    let str_trace = zipf(1_000_000, 0.99, OPS, 99);
    let loop_keys = names(CAP * 3 / 2);
    const W: usize = 5;
    let mut res = vec![vec![Vec::new(); W]; NAMES.len()];
    let mut hit = vec![[0.0; W]; NAMES.len()];
    for round in 0..ROUNDS {
        for j in 0..NAMES.len() {
            let i = (j + round) % NAMES.len();
            // 1. get, every key present
            let mut c = make::<u64>(i, CAP);
            for k in 0..CAP as u64 { c.put(k); }
            let r = time(OPS, || { let mut h = 0; let mut k = 0u64; for _ in 0..OPS { if c.get(black_box(&k)) { h += 1 } k = (k + 7) % CAP as u64; } h });
            res[i][0].push(r.0); hit[i][0] = r.1;
            // 2. put new keys (a flip every CAP puts for fliplru)
            let mut c = make::<u64>(i, CAP);
            let r = time(OPS, || { for k in 0..OPS as u64 { c.put(black_box(k)); } 0 });
            res[i][1].push(r.0);
            // 3. read-through, Zipf over 1M integer keys
            let mut c = make::<u64>(i, CAP);
            let r = time(OPS, || { let mut h = 0; for k in &ints { if c.read(black_box(k)) { h += 1 } } h });
            res[i][2].push(r.0); hit[i][2] = r.1;
            // 4. read-through, Zipf over 1M String keys
            let mut c = make::<String>(i, CAP);
            let r = time(OPS, || { let mut h = 0; for &x in &str_trace { if c.read(black_box(&strs[x as usize])) { h += 1 } } h });
            res[i][3].push(r.0); hit[i][3] = r.1;
            // 5. read-through, String keys cycling through 1.5 x CAP (classic LRU's worst case)
            let mut c = make::<String>(i, CAP);
            let r = time(OPS, || { let mut h = 0; let mut k = 0; for _ in 0..OPS { if c.read(black_box(&loop_keys[k])) { h += 1 } k = (k + 7) % loop_keys.len(); } h });
            res[i][4].push(r.0); hit[i][4] = r.1;
        }
    }
    println!("Speed, ns per operation (hit ratio), cap {CAP}, median of {ROUNDS} rounds\n");
    println!("| cache | get (all hits) | put | Zipf, integer keys | Zipf, String keys | loop, String keys |");
    println!("|---|---:|---:|---:|---:|---:|");
    for i in 0..NAMES.len() {
        let m: Vec<f64> = res[i].iter().map(|v| median(v.clone())).collect();
        println!(
            "| {} | {:.1} | {:.1} | {:.1} ({:.0}%) | {:.1} ({:.0}%) | {:.1} ({:.0}%) |",
            NAMES[i], m[0], m[1], m[2], hit[i][2] * 100.0, m[3], hit[i][3] * 100.0, m[4], hit[i][4] * 100.0
        );
    }
}

/// Hit ratio when every cache may hold the same number of entries. fliplru holds up to
/// 2 x cap, so it gets half the cap of the others.
fn hit_ratio() {
    let mem = 2 * CAP;
    let z99 = zipf(1_000_000, 0.99, 5_000_000, 7);
    let z70 = zipf(1_000_000, 0.7, 5_000_000, 11);
    let mut scan = Vec::with_capacity(15_000_000);
    let mut fresh = 1u64 << 40;
    for (n, &k) in z99.iter().enumerate() {
        scan.push(k);
        if n % 100_000 == 99_999 {
            for _ in 0..200_000 { scan.push(fresh); fresh += 1; }
        }
    }
    let lp: Vec<u64> = (0..5_000_000u64).map(|n| n % (mem as u64 * 6 / 5)).collect();
    println!("\nHit ratio at equal memory (each cache holds at most {mem} entries)\n");
    println!("| cache | Zipf 0.99 | Zipf 0.7 | Zipf 0.99 + scans | loop |");
    println!("|---|---:|---:|---:|---:|");
    for i in [0usize, 3, 5, 6] {
        let cap = if i == 0 { mem / 2 } else { mem };
        let mut r = [0.0; 4];
        for (t, trace) in [&z99, &z70, &scan, &lp].iter().enumerate() {
            let mut c = make::<u64>(i, cap);
            let mut h = 0usize;
            for k in trace.iter() { if c.read(k) { h += 1 } }
            r[t] = h as f64 / trace.len() as f64 * 100.0;
        }
        println!("| {} | {:.1}% | {:.1}% | {:.1}% | {:.1}% |", NAMES[i], r[0], r[1], r[2], r[3]);
    }
    println!("\nZipf + scans: after every 100,000 Zipf requests, 200,000 keys seen once (so at most ~33%).");
    println!("loop: keys 0..1.2 x memory in order, round and round.");
}

fn main() {
    speed();
    hit_ratio();
}
