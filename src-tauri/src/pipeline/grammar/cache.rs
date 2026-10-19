/*!
 * SOURCE OF TRUTH KEYWORDS: RewriteCache, sentence_cache, lru, prefetch_cache
 * WHAT:  A small least-recently-used map from a sentence to the grammar model's
 *        rewrite of it.
 * WHY:   The cache is what makes the model cost nothing at the moment it
 *        matters. While the person is still talking the finished sentences are
 *        rewritten in the background and stored here; at the stop only the last
 *        sentence is new. It also makes a repeat free — History's retry and a
 *        re-dictated phrase — and keeps the pass deterministic for a sentence it
 *        has seen.
 *
 *        It stores the model's RAW rewrite, not the guarded one: the guard
 *        depends on the user's dictionary, which can change between the
 *        prefetch and the stop, and the guard costs microseconds. Bounded, so a
 *        long-lived app does not grow without limit; a plain Vec-backed order
 *        list is plenty at this size.
 * WHERE: Owned by pipeline/grammar/pass.rs.
 */

use std::collections::{HashMap, VecDeque};

use parking_lot::Mutex;

const CAPACITY: usize = 256;

#[derive(Default)]
struct Inner {
    map: HashMap<String, String>,
    /// Least recently used at the front.
    order: VecDeque<String>,
}

#[derive(Default)]
pub struct RewriteCache {
    inner: Mutex<Inner>,
}

impl RewriteCache {
    pub fn get(&self, sentence: &str) -> Option<String> {
        let mut inner = self.inner.lock();
        let hit = inner.map.get(sentence).cloned()?;
        if let Some(pos) = inner.order.iter().position(|k| k == sentence) {
            if let Some(key) = inner.order.remove(pos) {
                inner.order.push_back(key);
            }
        }
        Some(hit)
    }

    pub fn contains(&self, sentence: &str) -> bool {
        self.inner.lock().map.contains_key(sentence)
    }

    pub fn put(&self, sentence: &str, rewrite: String) {
        let mut inner = self.inner.lock();
        if inner.map.insert(sentence.to_string(), rewrite).is_some() {
            inner.order.retain(|k| k != sentence);
        } else if inner.map.len() > CAPACITY {
            if let Some(oldest) = inner.order.pop_front() {
                inner.map.remove(&oldest);
            }
        }
        inner.order.push_back(sentence.to_string());
    }

    pub fn clear(&self) {
        let mut inner = self.inner.lock();
        inner.map.clear();
        inner.order.clear();
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.inner.lock().map.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stored_rewrite_is_returned_and_an_unknown_sentence_is_not() {
        let cache = RewriteCache::default();
        cache.put("I has a idea.", "I have an idea.".into());
        assert_eq!(cache.get("I has a idea."), Some("I have an idea.".into()));
        assert_eq!(cache.get("Something else."), None);
        assert!(cache.contains("I has a idea."));
    }

    #[test]
    fn it_never_grows_past_its_capacity_and_evicts_the_least_recently_used() {
        let cache = RewriteCache::default();
        for i in 0..CAPACITY {
            cache.put(&format!("sentence {i}"), format!("rewrite {i}"));
        }
        // Touch the oldest so the NEXT oldest is the one that goes.
        assert!(cache.get("sentence 0").is_some());
        cache.put("one more", "rewrite".into());

        assert_eq!(cache.len(), CAPACITY);
        assert!(cache.contains("sentence 0"), "recently used survives");
        assert!(!cache.contains("sentence 1"), "least recently used is evicted");
        assert!(cache.contains("one more"));
    }

    #[test]
    fn overwriting_a_key_does_not_duplicate_it() {
        let cache = RewriteCache::default();
        cache.put("a b", "one".into());
        cache.put("a b", "two".into());
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.get("a b"), Some("two".into()));
    }

    #[test]
    fn clearing_empties_it() {
        let cache = RewriteCache::default();
        cache.put("a b", "c".into());
        cache.clear();
        assert!(!cache.contains("a b"));
    }
}
