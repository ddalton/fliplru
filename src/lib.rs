use core::borrow::Borrow;
use core::hash::Hash;
use hashbrown::HashMap;
use std::{mem::replace, mem::swap, num::NonZeroUsize};

/// An LRU Cache
pub struct LruCache<K, V> {
    l1_map: HashMap<K, V>,
    l2_map: HashMap<K, V>,
    cap: NonZeroUsize,
}

impl<K: Hash + Eq, V> LruCache<K, V> {
    /// Creates a new LRU Cache that holds at most `cap*2` items. Though it never
    /// reaches the `cap*2` capacity most of the time.
    ///
    /// When the cache is full (reached cap items), then a "flip" occurs internally,
    /// where the full cache is backed up and an empty cache is brought in its place.
    /// Then as cache misses occur, the cache gets populated internally from the backup
    /// cache if the item is found there or a miss is reported to the user.
    ///
    /// Once the cache becomes full, the "flip" happens again and the full cache becomes
    /// the backup cache. Any items in the old backup cache is dropped. So that is why it
    /// does not represent a true `cap*2` items capacity.
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
        if !self.l1_map.contains_key(k) {
            match self.l2_map.remove_entry(k) {
                Some((rk, rv)) => {
                    self.put(rk, rv);
                }
                None => (),
            };
        }

        return self.l1_map.get(k);
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
    /// assert_eq!(None, cache.put(2, "beta"));
    ///
    /// assert_eq!(cache.get(&1), Some(&"a"));
    /// assert_eq!(cache.get(&2), Some(&"beta"));
    /// ```
    pub fn put(&mut self, k: K, v: V) -> Option<V> {
        if self.l1_map.len() >= self.cap.into() {
            swap(&mut self.l2_map, &mut self.l1_map);
            let _ = replace(&mut self.l1_map, HashMap::with_capacity(self.cap.into()));
        }
        self.l1_map.insert(k, v)
    }

    pub fn cap(&self) -> NonZeroUsize {
        self.cap
    }

    pub fn len(&self) -> usize {
        self.l1_map.len() + self.l2_map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.l1_map.len() == 0 && self.l2_map.len() == 0
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

        assert_eq!(cache.put("apple", "red"), None);
        assert_eq!(cache.put("banana", "yellow"), None);

        assert_eq!(cache.cap().get(), 2);
        assert_eq!(cache.len(), 2);
        assert!(!cache.is_empty());
        assert_opt_eq(cache.get(&"apple"), "red");
        assert_opt_eq(cache.get(&"banana"), "yellow");
    }
}
