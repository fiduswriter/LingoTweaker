//! Bounded engine reuse, including concurrent first requests for one language.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock};

type BuildResult<T> = Result<Arc<T>, String>;
type Pending<T> = Arc<OnceLock<BuildResult<T>>>;

struct Cached<T> {
    value: Arc<T>,
    used_at: u64,
}

struct State<T> {
    ready: BTreeMap<String, Cached<T>>,
    pending: BTreeMap<String, Pending<T>>,
    failed: BTreeMap<String, String>,
    clock: u64,
}

pub(crate) struct EngineCache<T> {
    state: Mutex<State<T>>,
}

impl<T> EngineCache<T> {
    pub(crate) fn new() -> Self {
        Self {
            state: Mutex::new(State {
                ready: BTreeMap::new(),
                pending: BTreeMap::new(),
                failed: BTreeMap::new(),
                clock: 0,
            }),
        }
    }

    pub(crate) fn get_or_try_init(
        &self,
        key: &str,
        capacity: impl FnOnce() -> usize,
        build: impl FnOnce() -> Result<T, String>,
    ) -> BuildResult<T> {
        let pending = {
            let mut state = self.state.lock().expect("engine cache lock");
            state.clock += 1;
            let stamp = state.clock;
            if let Some(cached) = state.ready.get_mut(key) {
                cached.used_at = stamp;
                return Ok(Arc::clone(&cached.value));
            }
            if let Some(error) = state.failed.get(key) {
                return Err(error.clone());
            }
            Arc::clone(state.pending.entry(key.to_owned()).or_default())
        };

        // Only callers for this key wait. No cache lock is held during a build
        // or while waiting, so hits and builds for other languages can proceed.
        let result = pending.get_or_init(|| build().map(Arc::new)).clone();

        let capacity = capacity();
        let mut evicted = None;
        {
            let mut state = self.state.lock().expect("engine cache lock");
            if state
                .pending
                .get(key)
                .is_some_and(|entry| Arc::ptr_eq(entry, &pending))
            {
                // Exactly one waiter publishes the result. Other waiters may
                // still hold it after eviction; they must not reinsert it.
                state.pending.remove(key);
                match &result {
                    Ok(value) if capacity > 0 => {
                        if state.ready.len() >= capacity {
                            let oldest = state
                                .ready
                                .iter()
                                .min_by_key(|(_, cached)| cached.used_at)
                                .map(|(key, _)| key.clone());
                            if let Some(oldest) = oldest {
                                evicted = state.ready.remove(&oldest);
                            }
                        }
                        state.clock += 1;
                        let used_at = state.clock;
                        state.ready.insert(
                            key.to_owned(),
                            Cached {
                                value: Arc::clone(value),
                                used_at,
                            },
                        );
                    }
                    Err(error) => {
                        state.failed.insert(key.to_owned(), error.clone());
                    }
                    _ => {} // capacity = 0: share the build, retain no engine.
                }
            }
        }
        // A large engine's destructor must not stall cache hits on other keys.
        drop(evicted);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Barrier;

    #[test]
    fn concurrent_cold_requests_share_one_build() {
        let cache = EngineCache::new();
        let builds = AtomicUsize::new(0);
        let start = Barrier::new(16);
        std::thread::scope(|scope| {
            let handles: Vec<_> = (0..16)
                .map(|_| {
                    scope.spawn(|| {
                        start.wait();
                        cache
                            .get_or_try_init(
                                "en-US",
                                || 8,
                                || {
                                    builds.fetch_add(1, Ordering::SeqCst);
                                    Ok(42)
                                },
                            )
                            .unwrap()
                    })
                })
                .collect();
            let values: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
            assert!(values.iter().all(|value| Arc::ptr_eq(value, &values[0])));
        });
        assert_eq!(builds.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn different_languages_build_concurrently() {
        let cache = EngineCache::new();
        let building = Barrier::new(2);
        std::thread::scope(|scope| {
            for key in ["en-US", "de-DE"] {
                let cache = &cache;
                let building = &building;
                scope.spawn(move || {
                    cache
                        .get_or_try_init(
                            key,
                            || 8,
                            || {
                                building.wait();
                                Ok(42)
                            },
                        )
                        .unwrap();
                });
            }
        });
    }

    #[test]
    fn eviction_preserves_recent_hits_and_in_flight_owners() {
        let cache = EngineCache::new();
        let en = cache.get_or_try_init("en", || 2, || Ok(1)).unwrap();
        let de = cache.get_or_try_init("de", || 2, || Ok(2)).unwrap();
        let hit = cache.get_or_try_init("en", || 2, || panic!("cache hit"));
        assert!(Arc::ptr_eq(&en, &hit.unwrap()));
        cache.get_or_try_init("fr", || 2, || Ok(3)).unwrap();
        assert_eq!(*de, 2, "requests may still own an evicted engine");
        assert_eq!(Arc::strong_count(&de), 1, "cache releases the engine");
        let rebuilt = cache.get_or_try_init("de", || 2, || Ok(4)).unwrap();
        assert_eq!(*rebuilt, 4);
    }

    #[test]
    fn zero_capacity_does_not_retain_engines() {
        let cache = EngineCache::new();
        let first = cache.get_or_try_init("en", || 0, || Ok(1)).unwrap();
        assert_eq!(Arc::strong_count(&first), 1);
        let second = cache.get_or_try_init("en", || 0, || Ok(2)).unwrap();
        assert_eq!(*second, 2);
        let state = cache.state.lock().unwrap();
        assert!(state.ready.is_empty());
        assert!(state.pending.is_empty());
    }

    #[test]
    fn failures_are_reused_without_rebuilding() {
        let cache = EngineCache::<()>::new();
        assert_eq!(
            cache.get_or_try_init("de", || 8, || Err("missing dictionary".into())),
            Err("missing dictionary".into())
        );
        assert_eq!(
            cache.get_or_try_init("de", || 8, || panic!("negative cache hit")),
            Err("missing dictionary".into())
        );
        assert!(cache.state.lock().unwrap().pending.is_empty());
    }

    #[test]
    fn concurrent_hits_and_evictions_do_not_race() {
        let cache = EngineCache::new();
        std::thread::scope(|scope| {
            for _ in 0..8 {
                let cache = &cache;
                scope.spawn(move || {
                    for _ in 0..500 {
                        for key in ["en", "de", "fr"] {
                            assert_eq!(*cache.get_or_try_init(key, || 1, || Ok(42)).unwrap(), 42);
                        }
                    }
                });
            }
        });
        let state = cache.state.lock().unwrap();
        assert_eq!(state.ready.len(), 1);
        assert!(state.pending.is_empty());
    }
}
