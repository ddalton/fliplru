#![feature(test)]
extern crate caches;
extern crate fast_lru;
extern crate lru;
extern crate lru_cache;
extern crate test;

#[cfg(test)]
mod tests {
    use super::*;
    use caches::lru::LRUCache;
    use caches::Cache as CachesCache;
    use hashlru::{Cache, SyncCache};
    use lru::LruCache;
    use std::collections::HashMap;
    use std::num::NonZeroUsize;
    use test::Bencher;

    const CAPACITY: usize = 100_000;

    #[bench]
    fn bench_read_usize_hashlru_cache(b: &mut Bencher) {
        let mut cache = Cache::new(CAPACITY);
        for i in 0..CAPACITY {
            cache.insert(i, i);
        }
        let mut i: usize = 0;
        b.iter(|| {
            cache.get(&i);
            i = (i + 7) % CAPACITY;
        });
    }

    #[bench]
    fn bench_read_usize_hashlru_sync_cache(b: &mut Bencher) {
        let cache = SyncCache::new(CAPACITY);
        for i in 0..CAPACITY {
            cache.insert(i, i);
        }
        let mut i: usize = 0;
        b.iter(|| {
            cache.get(&i);
            i = (i + 7) % CAPACITY;
        });
    }

    #[bench]
    fn bench_read_usize_builtin_hashmap(b: &mut Bencher) {
        let mut cache = HashMap::with_capacity(CAPACITY);
        for i in 0..CAPACITY {
            cache.insert(i, i);
        }
        let mut i: usize = 0;
        b.iter(|| {
            cache.get(&i);
            i = (i + 7) % CAPACITY;
        });
    }

    #[bench]
    fn bench_read_usize_extern_lru(b: &mut Bencher) {
        let mut cache = LruCache::new(NonZeroUsize::new(CAPACITY).unwrap());
        for i in 0..CAPACITY {
            cache.push(i, i);
        }
        let mut i: usize = 0;
        b.iter(|| {
            cache.get(&i);
            i = (i + 7) % CAPACITY;
        });
    }

    #[bench]
    fn bench_read_usize_extern_caches(b: &mut Bencher) {
        let mut cache = LRUCache::new(CAPACITY).unwrap();
        for i in 0..CAPACITY {
            cache.put(i, i);
        }
        let mut i: usize = 0;
        b.iter(|| {
            cache.get(&i);
            i = (i + 7) % CAPACITY;
        });
    }

    #[bench]
    fn bench_read_usize_extern_fliplru(b: &mut Bencher) {
        let mut cache = fliplru::LruCache::new(NonZeroUsize::new(CAPACITY).unwrap());
        for i in 0..CAPACITY {
            cache.put(i, i);
        }
        let mut i: usize = 0;
        b.iter(|| {
            cache.get(&i);
            i = (i + 7) % CAPACITY;
        });
    }

    #[bench]
    fn bench_read_usize_extern_fastlru(b: &mut Bencher) {
        let mut cache: fast_lru::LruCache<_, _, CAPACITY> = fast_lru::LruCache::new();
        for i in 0..CAPACITY {
            cache.put(i, i);
        }
        let mut i: usize = 0;
        b.iter(|| {
            cache.get(&i);
            i = (i + 7) % CAPACITY;
        });
    }

    #[bench]
    fn bench_read_usize_extern_lru_cache(b: &mut Bencher) {
        let mut cache = lru_cache::LruCache::new(CAPACITY);
        for i in 0..CAPACITY {
            cache.insert(i, i);
        }
        let mut i: usize = 0;
        b.iter(|| {
            cache.get_mut(&i);
            i = (i + 7) % CAPACITY;
        });
    }
}
