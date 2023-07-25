#![no_std]

use core::borrow::Borrow;
use core::hash::Hash;
use core::num::NonZeroUsize;
use core::{cmp, mem};
use hashbrown::HashMap;
use polonius_the_crab::{polonius, polonius_return};

/// An LRU Cache
pub struct LruCache<K, V> {
    l1_map: HashMap<K, V>,
    l2_map: HashMap<K, V>,
    cap: NonZeroUsize,
    flips: usize,
}

impl<K: Hash + Eq, V> LruCache<K, V> {
    /// Creates a new LRU Cache that holds `cap` items.
    /// It can fetch upto the last `cap*2` items, but only
    /// the last `cap` items is guaranteed to be in the cache.
    ///
    /// When the cache is full (reached cap items), then a "flip" occurs internally,
    /// where the full cache is backed up and an empty cache is brought in its place.
    /// Then as cache misses occur, the cache gets populated internally from the backup
    /// cache if the item is found there or a miss is reported to the user.
    ///
    /// # Example
    ///
    /// ```
    /// use fliplru::LruCache;
    /// use std::num::NonZeroUsize;
    /// let mut cache: LruCache<isize, &str> = LruCache::new(NonZeroUsize::new(10).unwrap());
    /// ```
    pub fn new(cap: NonZeroUsize) -> LruCache<K, V> {
        LruCache {
            l1_map: HashMap::with_capacity(cap.into()),
            l2_map: HashMap::with_capacity(cap.into()),
            cap,
            flips: 0,
        }
    }

    /// Returns a reference to the value of the key in the cache or `None` if it is not
    /// present in the cache.
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
        let mut this = self;
        polonius!(|this| -> Option<&'polonius V> {
            if let Some(v) = this.l1_map.get(k) {
                polonius_return!(Some(v));
            }
        });

        match this.l2_map.remove_entry(k) {
            Some((rk, rv)) => {
                this.put(rk, rv);
                this.l1_map.get(k)
            }
            None => None,
        }
    }

    /// Returns a mutable reference to the value of the key in the cache or `None` if it
    /// is not present in the cache. Moves the key to the l1_map if it exists in the l2_map.
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
        let mut this = self;
        polonius!(|this| -> Option<&'polonius mut V> {
            if let Some(v) = this.l1_map.get_mut(k) {
                polonius_return!(Some(v));
            }
        });

        match this.l2_map.remove_entry(k) {
            Some((rk, rv)) => {
                this.put(rk, rv);
                this.l1_map.get_mut(k)
            }
            None => None,
        }
    }

    /// Puts a key-value pair into cache. If the key already exists in the cache, then it updates
    /// the key's value and returns the old value. Otherwise, `None` is returned.
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
        if self.l1_map.len() == self.cap.into() {
            mem::swap(&mut self.l2_map, &mut self.l1_map);
            let _ = mem::replace(&mut self.l1_map, HashMap::with_capacity(self.cap.into()));
            self.flips += 1;
        }
        // invalidate any existing entry in L2 cache
        let ov = self.l2_map.remove(&k);
        match self.l1_map.insert(k, v) {
            Some(l1_v) => Some(l1_v),
            None => ov,
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

    /// Returns metric on the number of times the cache became full.
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

    /// Reset the flip metric.
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
