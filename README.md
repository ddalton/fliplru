# fliplru

[flip LRU](https://github.com/ddalton/fliplru) is a LRU cache that has some profiling built-in to help tune the cache capacity.

In its current form, it has a very simple API consisting of only the put and get methods.
It keeps track of the number of times (flips) the cache capacity is reached. It empties the cache and refills it in a way the performance is not affected (see benchmarks below).
If the flips count is 0, then the cache is oversized (in essence there is no cache misses). If the flip count is very high and close to the number of accesses/capacity then the cache is not being used effectively and the capacity has to be increased.

The API has been inspired by [lru](https://crates.io/crates/lru) by [Jerome Froelich](https://github.com/jeromefroe). 
# Status

It is a basic cache with some profiling. Very useful where a high performance get API is desired.

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

