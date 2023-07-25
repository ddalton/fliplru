# fliplru

[flip LRU](https://github.com/ddalton/fliplru) is a LRU cache that has some profiling built-in to help tune the cache capacity.

The goals of this cache data structure are two-fold:
1. The get API should be fast with low overhead as it is the main API of a cache.
2. Expose some profiling metrics to help with deciding on the appropriate capacity.

There is another goal but it is implementation related and I didn't want to use a linked list.

The implementation is based on 2 hashmaps to provide the LRU functionality. So the total capacity of this cache is `cap*2` internally.
The `cap` LRU items are guaranteed to be in the cache. The `cap+` to `cap*2` LRU items maybe in the cache, but this is not guaranteed.

The hashbrown map is used along with the 2 cache design to provide a fast get API.
A flips metric is exposed to help tune the cache capacity. Flips represent the number of times the cache capacity is reached. It empties the cache and refills it in a way the performance is not affected (see benchmarks below).
If the flips count is 0, then the cache is oversized. If the flip count is very high and close to the number of accesses/capacity then the cache is not being used effectively and the capacity has to be increased.

The API has been inspired by [lru](https://crates.io/crates/lru) by [Jerome Froelich](https://github.com/jeromefroe). 

## Find if cache capacity is too small for use case
Below is example of a check to find if the cache has been configured with less capacity.
In this example the cache is accessed for 20 times and the flip count is 8 for a cache capacity of 2. This indicates there is no caching occurring for the access pattern.

```
let mut cache = LruCache::new(NonZeroUsize::new(2).unwrap());
for i in 0..5 {
    cache.put(i, i);
}
for i in 0..20 {
    cache.get(&(i % 5));
}

assert_eq!(cache.get_flips(), 8);
```

## Find if cache capacity is too large for use case
Below is example of a check to find if the cache has been configured with more than enough capacity.
In this example the cache is accessed for 20 times and the number of flips is 0 for a cache capacity of 5. This indicates the cache fully satisfies every access made.

```
let mut cache = LruCache::new(NonZeroUsize::new(5).unwrap());
for i in 0..5 {
    cache.put(i, i);
}
for i in 0..20 {
    cache.get(&(i % 5));
}

assert_eq!(cache.get_flips(), 0);
```

# Status

It is a basic LRU cache with metrics to help with cache capacity tuning. Provides a fast get API.

# Benchmarks

The benchmarks has been inspired by [HashLRU](https://gitlab.com/liberecofr/hashlru) by Christian Mauduit
Benchmarks of various caches using the get API. Run on MacBook Air with 100k items cache.

```
running 6 tests
test tests::bench_read_usize_builtin_hashmap    ... bench:          10 ns/iter (+/- 0)
test tests::bench_read_usize_extern_caches      ... bench:          24 ns/iter (+/- 0)
test tests::bench_read_usize_extern_fliplru     ... bench:           7 ns/iter (+/- 0)
test tests::bench_read_usize_extern_lru         ... bench:          10 ns/iter (+/- 0)
test tests::bench_read_usize_hashlru_cache      ... bench:          10 ns/iter (+/- 0)
test tests::bench_read_usize_hashlru_sync_cache ... bench:          15 ns/iter (+/- 0)

test result: ok. 0 passed; 0 failed; 0 ignored; 6 measured; 0 filtered out; finished in 14.49s
```

To run the benchmarks:

```shell
cd bench
rustup default nightly
cargo bench
```

