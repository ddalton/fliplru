//! An LRU cache with a fast `get` and a built-in signal for sizing it.
//!
//! fliplru keeps two hash maps, the *current* and the *previous* generation. New and
//! recently used keys live in the current one; when it reaches `cap` entries it **flips**:
//! it becomes the previous generation, the old previous generation is dropped, and an empty
//! current generation starts. A key found in the previous generation is moved back into the
//! current one, so anything still in use survives the next flip.
//!
//! - **`get` is one hash lookup** when the key is in the current generation, and every
//!   operation hashes the key once.
//! - **The last `cap` distinct keys used are always in the cache.** Up to `2 * cap` may be
//!   (the previous generation too), so plan memory for `2 * cap` entries.
//! - **Flips tell you whether `cap` fits your workload** ([`LruCache::get_flips`]):
//!   - no flips: everything you use fits — the cache may be larger than it needs to be;
//!   - flips close to `accesses / cap`: almost nothing is reused before it is dropped —
//!     the cache is too small to help;
//!   - in between: the cache is working; fewer flips per access means more reuse.
//!
//! # Example
//!
//! ```
//! use fliplru::LruCache;
//! use std::num::NonZeroUsize;
//!
//! let mut cache = LruCache::new(NonZeroUsize::new(2).unwrap());
//! cache.put("apple", 3);
//! cache.put("pear", 5);
//! assert_eq!(cache.get(&"apple"), Some(&3));
//!
//! // read-through: compute the value only on a miss, with one hash
//! let n = *cache.get_or_insert_with("plum", || 7);
//! assert_eq!(n, 7);
//!
//! // with String keys, look up by &str: no String is built on a hit
//! let mut by_name: LruCache<String, u32> = LruCache::new(NonZeroUsize::new(100).unwrap());
//! by_name.get_or_insert_with_ref("alice", || 1);
//! assert_eq!(by_name.get("alice"), Some(&1));
//! ```
//!
//! # Choosing the capacity
//!
//! ```
//! use fliplru::LruCache;
//! use std::num::NonZeroUsize;
//!
//! // 5 keys used round-robin, cache of 2: every access misses, the flips say so
//! let mut small = LruCache::new(NonZeroUsize::new(2).unwrap());
//! for i in 0..20 { small.get_or_insert_with(i % 5, || i); }
//! assert_eq!(small.get_flips(), 9);
//!
//! // a cache of 5: everything fits, no flips
//! let mut big = LruCache::new(NonZeroUsize::new(5).unwrap());
//! for i in 0..20 { big.get_or_insert_with(i % 5, || i); }
//! assert_eq!(big.get_flips(), 0);
//! ```
//!
//! # When to use something else
//!
//! fliplru evicts a whole generation at a time, so at equal memory it keeps somewhat fewer
//! useful keys than a classic LRU, and it is not scan resistant. If hit ratio matters more
//! than speed and the sizing signal, see `quick_cache` (S3-FIFO) or `moka` (W-TinyLFU).
//! fliplru is single-threaded (`&mut self`); wrap it in a lock to share it.

#![no_std]

extern crate alloc;

use alloc::borrow::ToOwned;
use core::borrow::Borrow;
use core::hash::{BuildHasher, Hash};
use core::num::NonZeroUsize;
use core::{cmp, mem};
use hashbrown::{hash_table::Entry, DefaultHashBuilder, HashTable};
use polonius_the_crab::{polonius, polonius_return};

/// An LRU cache of `K` to `V`, hashing keys with `S`
/// (hashbrown's default hasher unless chosen with [`LruCache::with_hasher`]).
///
/// See the [crate documentation](crate) for how it works and how to size it.
pub struct LruCache<K, V, S = DefaultHashBuilder> {
    l1_map: HashTable<(K, V)>,
    l2_map: HashTable<(K, V)>,
    hasher: S,
    cap: NonZeroUsize,
    flips: usize,
}

impl<K: Hash + Eq, V> LruCache<K, V> {
    /// Creates a cache that keeps at least the last `cap` keys used (and up to `2 * cap`).
    ///
    /// Memory for both generations is allocated up front, so no allocation happens when the
    /// cache flips.
    ///
    /// # Example
    ///
    /// ```
    /// use fliplru::LruCache;
    /// use std::num::NonZeroUsize;
    /// let mut cache: LruCache<isize, &str> = LruCache::new(NonZeroUsize::new(10).unwrap());
    /// ```
    pub fn new(cap: NonZeroUsize) -> LruCache<K, V> {
        LruCache::with_hasher(cap, DefaultHashBuilder::default())
    }
}

impl<K: Hash + Eq, V, S: BuildHasher> LruCache<K, V, S> {
    /// Creates a cache that hashes keys with `hasher`.
    ///
    /// The hasher runs once per operation, so a cheaper one speeds up everything. For integer
    /// keys from a trusted source, an Fx-style hasher (the `rustc-hash` crate) is much faster
    /// than the default; keep the default when keys can come from an attacker, as it resists
    /// collision attacks.
    ///
    /// # Example
    ///
    /// ```
    /// use fliplru::LruCache;
    /// use hashbrown::DefaultHashBuilder;
    /// use std::num::NonZeroUsize;
    /// let mut cache: LruCache<u64, &str> =
    ///     LruCache::with_hasher(NonZeroUsize::new(10).unwrap(), DefaultHashBuilder::default());
    /// cache.put(1, "a");
    /// assert_eq!(cache.get(&1), Some(&"a"));
    /// ```
    pub fn with_hasher(cap: NonZeroUsize, hasher: S) -> LruCache<K, V, S> {
        LruCache {
            l1_map: HashTable::with_capacity(cap.into()),
            l2_map: HashTable::with_capacity(cap.into()),
            hasher,
            cap,
            flips: 0,
        }
    }

    /// Finds `k` (hashed once, as `hash`): in the cache, or in the backup map, from which it is
    /// moved into the cache. The reference is to the entry where it now is.
    fn find_promote<Q>(&mut self, hash: u64, k: &Q) -> Option<&mut (K, V)>
    where
        K: Borrow<Q>,
        Q: Eq + ?Sized,
    {
        let mut this = self;
        polonius!(|this| -> Option<&'polonius mut (K, V)> {
            if let Some(e) = this.l1_map.find_mut(hash, |e| e.0.borrow() == k) {
                polonius_return!(Some(e));
            }
        });
        // In the backup map: after the removal the key is in neither map, so it is inserted
        // once, with the hash already computed.
        let ((rk, rv), _) = this.l2_map.find_entry(hash, |e| e.0.borrow() == k).ok()?.remove();
        this.flip_if_full();
        let hasher = &this.hasher;
        Some(
            this.l1_map
                .insert_unique(hash, (rk, rv), |e| hasher.hash_one(&e.0))
                .into_mut(),
        )
    }

    /// Returns the value of `k`, or `None` if it is not in the cache.
    ///
    /// A key found in the previous generation is moved into the current one (it is in use), which
    /// may flip the cache. Takes `&mut self` for that reason.
    ///
    /// # Example
    ///
    /// ```
    /// use fliplru::LruCache;
    /// use std::num::NonZeroUsize;
    /// let mut cache = LruCache::new(NonZeroUsize::new(2).unwrap());
    ///
    /// cache.put(1, "a");
    /// cache.put(2, "b");
    /// cache.put(2, "c");
    /// cache.put(3, "d");
    ///
    /// assert_eq!(cache.get(&2), Some(&"c"));
    /// assert_eq!(cache.get(&3), Some(&"d"));
    /// ```
    pub fn get<'a, Q>(&'a mut self, k: &Q) -> Option<&'a V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let hash = self.hasher.hash_one(k);
        self.find_promote(hash, k).map(|e| &e.1)
    }

    /// Returns a mutable reference to the value of `k`, or `None` if it is not in the cache.
    ///
    /// Like [`LruCache::get`], a key found in the previous generation moves into the current one.
    ///
    /// # Example
    ///
    /// ```
    /// use fliplru::LruCache;
    /// use std::num::NonZeroUsize;
    /// let mut cache = LruCache::new(NonZeroUsize::new(2).unwrap());
    ///
    /// cache.put("apple", 8);
    /// cache.put("banana", 4);
    /// cache.put("banana", 6);
    /// cache.put("pear", 2);
    ///
    /// assert_eq!(cache.get_mut(&"apple"), Some(&mut 8));
    /// assert_eq!(cache.get_mut(&"banana"), Some(&mut 6));
    /// assert_eq!(cache.get_mut(&"pear"), Some(&mut 2));
    /// ```
    pub fn get_mut<'a, Q>(&'a mut self, k: &Q) -> Option<&'a mut V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let hash = self.hasher.hash_one(k);
        self.find_promote(hash, k).map(|e| &mut e.1)
    }

    /// Returns the value of `k`, first inserting `f()` if `k` is not in the cache.
    ///
    /// This is a read-through: the same as [`LruCache::get`] and, on a miss, [`LruCache::put`],
    /// but with one hash instead of up to four. `f` runs only on a miss. Takes the key by value;
    /// use [`LruCache::get_or_insert_with_ref`] to avoid building a key (a `String`, say) on hits.
    ///
    /// # Example
    ///
    /// ```
    /// use fliplru::LruCache;
    /// use std::num::NonZeroUsize;
    /// let mut cache = LruCache::new(NonZeroUsize::new(2).unwrap());
    ///
    /// assert_eq!(*cache.get_or_insert_with(1, || "a"), "a");
    /// assert_eq!(*cache.get_or_insert_with(1, || "b"), "a"); // a hit: `f` is not called
    /// assert_eq!(cache.get(&1), Some(&"a"));
    /// ```
    pub fn get_or_insert_with<F: FnOnce() -> V>(&mut self, k: K, f: F) -> &mut V {
        let hash = self.hasher.hash_one(&k);
        let mut this = self;
        polonius!(|this| -> &'polonius mut V {
            if let Some(e) = this.find_promote(hash, &k) {
                polonius_return!(&mut e.1);
            }
        });
        // A miss: the key is in neither map (also after a flip, whose backup is the cache
        // that did not have it).
        this.flip_if_full();
        let hasher = &this.hasher;
        &mut this
            .l1_map
            .insert_unique(hash, (k, f()), |e| hasher.hash_one(&e.0))
            .into_mut()
            .1
    }

    /// Like [`LruCache::get_or_insert_with`], but looks up by reference: an owned key is made
    /// (`to_owned`) only on a miss, so hits allocate nothing. For `String` keys, pass a `&str`.
    ///
    /// # Example
    ///
    /// ```
    /// use fliplru::LruCache;
    /// use std::num::NonZeroUsize;
    /// let mut cache: LruCache<String, usize> = LruCache::new(NonZeroUsize::new(2).unwrap());
    ///
    /// assert_eq!(*cache.get_or_insert_with_ref("apple", || 8), 8);
    /// assert_eq!(*cache.get_or_insert_with_ref("apple", || 9), 8); // a hit: no String is made
    /// assert_eq!(cache.get("apple"), Some(&8));
    /// ```
    pub fn get_or_insert_with_ref<Q, F>(&mut self, k: &Q, f: F) -> &mut V
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ToOwned<Owned = K> + ?Sized,
        F: FnOnce() -> V,
    {
        let hash = self.hasher.hash_one(k);
        let mut this = self;
        polonius!(|this| -> &'polonius mut V {
            if let Some(e) = this.find_promote(hash, k) {
                polonius_return!(&mut e.1);
            }
        });
        this.flip_if_full();
        let hasher = &this.hasher;
        &mut this
            .l1_map
            .insert_unique(hash, (k.to_owned(), f()), |e| hasher.hash_one(&e.0))
            .into_mut()
            .1
    }

    /// Inserts `v` for `k`, returning the value `k` had, if any.
    ///
    /// The entry goes into the current generation, so it is among the last `cap` keys used. If
    /// the current generation is full, the cache flips first.
    ///
    /// # Example
    ///
    /// ```
    /// use fliplru::LruCache;
    /// use std::num::NonZeroUsize;
    /// let mut cache = LruCache::new(NonZeroUsize::new(2).unwrap());
    ///
    /// assert_eq!(None, cache.put(1, "a"));
    /// assert_eq!(None, cache.put(2, "b"));
    /// assert_eq!(Some("b"), cache.put(2, "beta"));
    ///
    /// assert_eq!(cache.get(&1), Some(&"a"));
    /// assert_eq!(cache.get(&2), Some(&"beta"));
    /// ```
    pub fn put(&mut self, k: K, v: V) -> Option<V> {
        let hash = self.hasher.hash_one(&k);
        self.flip_if_full();
        // invalidate any existing entry in L2 cache
        let ov = match self.l2_map.find_entry(hash, |e| e.0 == k) {
            Ok(o) => Some(o.remove().0 .1),
            Err(_) => None,
        };
        let hasher = &self.hasher;
        match self.l1_map.entry(hash, |e| e.0 == k, |e| hasher.hash_one(&e.0)) {
            Entry::Occupied(mut o) => Some(mem::replace(&mut o.get_mut().1, v)),
            Entry::Vacant(e) => {
                e.insert((k, v));
                ov
            }
        }
    }

    /// When the cache is full, flip: the full map becomes the backup and the old backup,
    /// emptied, becomes the cache. Clearing keeps its table, so a flip allocates nothing.
    #[inline]
    fn flip_if_full(&mut self) {
        if self.l1_map.len() == self.cap.get() {
            mem::swap(&mut self.l2_map, &mut self.l1_map);
            self.l1_map.clear();
            self.flips += 1;
        }
    }

    /// Returns the maximum number of key-value pairs the cache can hold.
    ///
    /// # Example
    ///
    /// ```
    /// use fliplru::LruCache;
    /// use std::num::NonZeroUsize;
    /// let mut cache: LruCache<isize, &str> = LruCache::new(NonZeroUsize::new(2).unwrap());
    /// assert_eq!(cache.cap().get(), 2);
    /// ```
    pub fn cap(&self) -> NonZeroUsize {
        self.cap
    }

    /// Returns the number of key-value pairs that are currently in the the cache.
    ///
    /// # Example
    ///
    /// ```
    /// use fliplru::LruCache;
    /// use std::num::NonZeroUsize;
    /// let mut cache = LruCache::new(NonZeroUsize::new(2).unwrap());
    /// assert_eq!(cache.len(), 0);
    ///
    /// cache.put(1, "a");
    /// assert_eq!(cache.len(), 1);
    ///
    /// cache.put(2, "b");
    /// assert_eq!(cache.len(), 2);
    ///
    /// cache.put(3, "c");
    /// assert_eq!(cache.len(), 2);
    /// ```
    pub fn len(&self) -> usize {
        cmp::min(self.l1_map.len() + self.l2_map.len(), self.cap().into())
    }

    /// Returns a bool indicating whether the cache is empty or not.
    ///
    /// # Example
    ///
    /// ```
    /// use fliplru::LruCache;
    /// use std::num::NonZeroUsize;
    /// let mut cache = LruCache::new(NonZeroUsize::new(2).unwrap());
    /// assert!(cache.is_empty());
    ///
    /// cache.put(1, "a");
    /// assert!(!cache.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.l1_map.len() == 0 && self.l2_map.len() == 0
    }

    /// Returns how many times the cache has flipped (its current generation filled up) since it
    /// was created or [`LruCache::reset`] was called.
    ///
    /// Compare it with the number of accesses to judge the capacity: no flips means everything
    /// fits; close to `accesses / cap` flips means almost nothing is reused before it is dropped.
    /// See the [crate documentation](crate#choosing-the-capacity).
    ///
    /// # Example
    ///
    /// ```
    /// use fliplru::LruCache;
    /// use std::num::NonZeroUsize;
    /// let mut cache = LruCache::new(NonZeroUsize::new(2).unwrap());
    ///
    /// for i in 0..5 {
    ///     cache.put(i, i);
    /// }
    /// for i in 0..20 {
    ///     cache.get(&(i % 5));
    /// }
    /// assert_eq!(cache.get_flips(), 8);
    /// ```
    pub fn get_flips(&self) -> usize {
        self.flips
    }

    /// Resets the flip count to zero (the cache's contents are unchanged), to measure a new
    /// period.
    ///
    /// # Example
    ///
    /// ```
    /// use fliplru::LruCache;
    /// use std::num::NonZeroUsize;
    /// let mut cache = LruCache::new(NonZeroUsize::new(2).unwrap());
    ///
    /// for i in 0..5 {
    ///     cache.put(i, i);
    /// }
    /// for i in 0..20 {
    ///     cache.get(&(i % 5));
    /// }
    /// assert_eq!(cache.get_flips(), 8);
    /// cache.reset();
    /// assert_eq!(cache.get_flips(), 0);
    /// ```
    pub fn reset(&mut self) {
        self.flips = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::LruCache;
    use core::{fmt::Debug, num::NonZeroUsize};

    fn assert_opt_eq<V: PartialEq + Debug>(opt: Option<&V>, v: V) {
        assert!(opt.is_some());
        assert_eq!(opt.unwrap(), &v);
    }

    #[test]
    fn test_put_and_get() {
        let mut cache = LruCache::new(NonZeroUsize::new(2).unwrap());
        assert!(cache.is_empty());
        assert_eq!(cache.get_flips(), 0);

        assert_eq!(cache.put("apple", "red"), None);
        assert_eq!(cache.put("banana", "yellow"), None);

        assert_eq!(cache.cap().get(), 2);
        assert_eq!(cache.len(), 2);
        assert_eq!(cache.get_flips(), 0);
        assert!(!cache.is_empty());
        assert_opt_eq(cache.get(&"apple"), "red");
        assert_opt_eq(cache.get(&"banana"), "yellow");
    }

    #[test]
    fn test_put_update() {
        let mut cache = LruCache::new(NonZeroUsize::new(2).unwrap());

        assert_eq!(cache.put("apple", "red"), None);
        assert_eq!(cache.put("apple", "green"), Some("red"));

        assert_eq!(cache.len(), 1);
        assert_opt_eq(cache.get(&"apple"), "green");
    }

    #[test]
    fn test_l2() {
        let mut cache = LruCache::new(NonZeroUsize::new(2).unwrap());

        assert_eq!(cache.get_flips(), 0);
        assert_eq!(cache.put("apple", "red"), None);
        assert_eq!(cache.put("banana", "yellow"), None);
        assert_eq!(cache.put("pear", "green"), None);
        assert_eq!(cache.get_flips(), 1);

        // This is retrieved from the overflow (L2 cache)
        assert_opt_eq(cache.get(&"apple"), "red");
        assert_opt_eq(cache.get(&"banana"), "yellow");
        assert_opt_eq(cache.get(&"pear"), "green");
        assert_eq!(cache.get_flips(), 2);

        // apple is no longer in both the caches
        assert_eq!(cache.put("apple", "green"), None);
        assert_eq!(cache.put("tomato", "red"), None);
        assert_eq!(cache.get_flips(), 3);

        assert_opt_eq(cache.get(&"pear"), "green");
        assert_opt_eq(cache.get(&"apple"), "green");
        assert_opt_eq(cache.get(&"tomato"), "red");
        assert_eq!(cache.get_flips(), 5);
    }

    #[test]
    fn test_max_cache_len() {
        let mut cache = LruCache::new(NonZeroUsize::new(2).unwrap());

        assert_eq!(cache.put("apple", "red"), None);
        assert_eq!(cache.put("banana", "yellow"), None);
        assert_eq!(cache.put("pear", "green"), None);
        assert_eq!(cache.put("tomato", "red"), None);
        assert_eq!(cache.get_flips(), 1);

        // Could retrieve `cap*2` oldest item, i.e., the 4th oldest item.
        assert_opt_eq(cache.get(&"apple"), "red");
        assert_eq!(cache.get_flips(), 2);

        // Could not retrieve `cap+1` oldest item, i.e., the 3rd oldest item, showing that only the
        // first `cap` items is guaranteed to be in the cache.
        assert_eq!(cache.get(&"banana"), None);
        assert_eq!(cache.get_flips(), 2);

        cache.reset();
        assert_eq!(cache.get_flips(), 0);
    }

    #[test]
    fn test_cache_under_capacity() {
        let mut cache = LruCache::new(NonZeroUsize::new(2).unwrap());
        for i in 0..5 {
            cache.put(i, i);
        }
        for i in 0..20 {
            cache.get(&(i % 5));
        }

        assert_eq!(cache.get_flips(), 8);
    }

    #[test]
    fn test_cache_over_capacity() {
        let mut cache = LruCache::new(NonZeroUsize::new(5).unwrap());
        for i in 0..5 {
            cache.put(i, i);
        }
        for i in 0..20 {
            cache.get(&(i % 5));
        }

        assert_eq!(cache.get_flips(), 0);
    }
}
