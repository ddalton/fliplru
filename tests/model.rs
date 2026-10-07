//! The crate against a plain model of the algorithm: two maps written the obvious way. Random
//! gets, puts and read-throughs at several capacities must give the same values, lengths and
//! flip counts, with the default hasher and a custom one. Any optimization of the crate has to
//! keep this test passing.

use fliplru::LruCache;
use std::collections::HashMap;
use std::hash::BuildHasherDefault;
use std::num::NonZeroUsize;

/// fliplru's algorithm, as plainly as it can be written.
struct Model {
    current: HashMap<u64, u64>,
    previous: HashMap<u64, u64>,
    cap: usize,
    flips: usize,
}

impl Model {
    fn new(cap: usize) -> Model {
        Model { current: HashMap::new(), previous: HashMap::new(), cap, flips: 0 }
    }
    fn flip_if_full(&mut self) {
        if self.current.len() == self.cap {
            self.previous = std::mem::take(&mut self.current);
            self.flips += 1;
        }
    }
    fn get(&mut self, k: u64) -> Option<u64> {
        if let Some(&v) = self.current.get(&k) {
            return Some(v);
        }
        let v = self.previous.remove(&k)?;
        self.flip_if_full();
        self.current.insert(k, v);
        Some(v)
    }
    fn put(&mut self, k: u64, v: u64) -> Option<u64> {
        self.flip_if_full();
        let old = self.previous.remove(&k);
        self.current.insert(k, v).or(old)
    }
    fn read(&mut self, k: u64, v: u64) -> u64 {
        match self.get(k) {
            Some(x) => x,
            None => {
                self.put(k, v);
                v
            }
        }
    }
    fn len(&self) -> usize {
        std::cmp::min(self.current.len() + self.previous.len(), self.cap)
    }
}

/// A deliberately different hasher, to exercise `with_hasher`.
#[derive(Default)]
struct Mul(u64);
impl std::hash::Hasher for Mul {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0 ^ b as u64).wrapping_mul(0x100000001b3);
        }
    }
}

fn run<S: std::hash::BuildHasher>(mut cache: LruCache<u64, u64, S>, cap: usize, seed: u64) {
    let mut model = Model::new(cap);
    let mut x = seed;
    let mut rnd = move || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    };
    let keys = cap as u64 * 3 + 1;
    for n in 0..100_000 {
        let k = rnd() % keys;
        let v = rnd();
        match rnd() % 5 {
            0 => assert_eq!(cache.put(k, v), model.put(k, v), "put, cap {cap}, op {n}"),
            1 => assert_eq!(cache.get(&k).copied(), model.get(k), "get, cap {cap}, op {n}"),
            2 => assert_eq!(cache.get_mut(&k).map(|r| *r), model.get(k), "get_mut, cap {cap}, op {n}"),
            3 => assert_eq!(*cache.get_or_insert_with(k, || v), model.read(k, v), "get_or_insert_with, cap {cap}, op {n}"),
            _ => assert_eq!(*cache.get_or_insert_with_ref(&k, || v), model.read(k, v), "get_or_insert_with_ref, cap {cap}, op {n}"),
        }
        assert_eq!(cache.get_flips(), model.flips, "flips, cap {cap}, op {n}");
        assert_eq!(cache.len(), model.len(), "len, cap {cap}, op {n}");
    }
}

#[test]
fn the_cache_behaves_as_the_plain_model() {
    for (i, &cap) in [1usize, 2, 3, 7, 64, 1000].iter().enumerate() {
        let nz = NonZeroUsize::new(cap).unwrap();
        run(LruCache::new(nz), cap, 0x9e3779b97f4a7c15 ^ i as u64);
        run(LruCache::with_hasher(nz, BuildHasherDefault::<Mul>::default()), cap, 0x2545f4914f6cdd1d ^ i as u64);
    }
}
