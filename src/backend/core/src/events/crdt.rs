//! Conflict-free Replicated Data Types (CRDTs) for merging parallel agent outputs.
//!
//! When multiple agents work on the same logical entity concurrently, their outputs
//! must be merged deterministically without coordination. CRDTs provide this guarantee:
//! any order of merge operations converges to the same result.
//!
//! Provided types:
//! - [`LWWRegister`]: Last-Writer-Wins register for scalar values.
//! - [`GSet`]: Grow-only set -- elements can be added but never removed.
//! - [`ORSet`]: Observed-Remove set -- supports both add and remove.
//! - [`GCounter`]: Grow-only counter distributed across nodes.
//! - [`MergeableState`]: Composite container holding registers, sets, and counters.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

// =============================================================================
// LWW Register
// =============================================================================

/// A Last-Writer-Wins Register.
///
/// Conflicts are resolved by timestamp. If timestamps are equal, the higher
/// `node_id` (lexicographic on UUID bytes) wins to ensure determinism.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LWWRegister<T: Clone> {
    value: T,
    timestamp: DateTime<Utc>,
    node_id: Uuid,
}

impl<T: Clone> LWWRegister<T> {
    /// Create a new register with an initial value.
    pub fn new(value: T, node_id: Uuid) -> Self {
        Self {
            value,
            timestamp: Utc::now(),
            node_id,
        }
    }

    /// Create a register with an explicit timestamp.
    pub fn with_timestamp(value: T, timestamp: DateTime<Utc>, node_id: Uuid) -> Self {
        Self {
            value,
            timestamp,
            node_id,
        }
    }

    /// Get the current value.
    pub fn value(&self) -> &T {
        &self.value
    }

    /// Get the timestamp of the last write.
    pub fn timestamp(&self) -> DateTime<Utc> {
        self.timestamp
    }

    /// Get the node that performed the last write.
    pub fn node_id(&self) -> Uuid {
        self.node_id
    }

    /// Update the register value. Only succeeds if the new timestamp is newer
    /// (or equal with a higher node_id).
    pub fn set(&mut self, value: T, timestamp: DateTime<Utc>, node_id: Uuid) {
        if self.should_accept(timestamp, node_id) {
            self.value = value;
            self.timestamp = timestamp;
            self.node_id = node_id;
        }
    }

    /// Merge with another register. The winning write is kept.
    pub fn merge(&mut self, other: &LWWRegister<T>) {
        if self.should_accept(other.timestamp, other.node_id) {
            self.value = other.value.clone();
            self.timestamp = other.timestamp;
            self.node_id = other.node_id;
        }
    }

    /// Determine whether a candidate write should replace the current value.
    fn should_accept(&self, timestamp: DateTime<Utc>, node_id: Uuid) -> bool {
        timestamp > self.timestamp
            || (timestamp == self.timestamp && node_id > self.node_id)
    }
}

// =============================================================================
// G-Set (Grow-Only Set)
// =============================================================================

/// A grow-only set. Elements can be added but never removed.
///
/// Merge is simply set union, which is commutative, associative, and idempotent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GSet<T: Eq + std::hash::Hash + Clone> {
    elements: HashSet<T>,
}

impl<T: Eq + std::hash::Hash + Clone> GSet<T> {
    /// Create an empty grow-only set.
    pub fn new() -> Self {
        Self {
            elements: HashSet::new(),
        }
    }

    /// Insert an element.
    pub fn insert(&mut self, element: T) {
        self.elements.insert(element);
    }

    /// Check membership.
    pub fn contains(&self, element: &T) -> bool {
        self.elements.contains(element)
    }

    /// Get the number of elements.
    pub fn len(&self) -> usize {
        self.elements.len()
    }

    /// Check if the set is empty.
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    /// Iterate over elements.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.elements.iter()
    }

    /// Merge with another GSet (set union).
    pub fn merge(&mut self, other: &GSet<T>) {
        for element in &other.elements {
            self.elements.insert(element.clone());
        }
    }
}

impl<T: Eq + std::hash::Hash + Clone> Default for GSet<T> {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// OR-Set (Observed-Remove Set)
// =============================================================================

/// An Observed-Remove Set.
///
/// Each add operation generates a unique tag (UUID). A remove operation only
/// removes the tags that were *observed* at the time of removal. Concurrent
/// adds of the same element with different tags survive a remove, achieving
/// "add wins" semantics on concurrent add/remove.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ORSet<T: Eq + std::hash::Hash + Clone> {
    /// value -> set of unique add-tags that are still live
    elements: HashMap<T, HashSet<Uuid>>,
    /// value -> set of unique add-tags that have been removed
    tombstones: HashMap<T, HashSet<Uuid>>,
}

impl<T: Eq + std::hash::Hash + Clone> ORSet<T> {
    /// Create an empty OR-Set.
    pub fn new() -> Self {
        Self {
            elements: HashMap::new(),
            tombstones: HashMap::new(),
        }
    }

    /// Add an element, returning the unique tag for this addition.
    pub fn add(&mut self, element: T) -> Uuid {
        let tag = Uuid::new_v4();
        self.elements
            .entry(element)
            .or_default()
            .insert(tag);
        tag
    }

    /// Remove an element by tombstoning all currently observed tags.
    ///
    /// Returns `true` if the element was present and removed.
    pub fn remove(&mut self, element: &T) -> bool {
        if let Some(tags) = self.elements.remove(element) {
            if tags.is_empty() {
                return false;
            }
            self.tombstones
                .entry(element.clone())
                .or_default()
                .extend(tags);
            true
        } else {
            false
        }
    }

    /// Check if an element is in the set (has at least one live tag).
    pub fn contains(&self, element: &T) -> bool {
        self.elements
            .get(element)
            .map_or(false, |tags| !tags.is_empty())
    }

    /// Return the set of live elements.
    pub fn elements(&self) -> Vec<&T> {
        self.elements
            .iter()
            .filter(|(_, tags)| !tags.is_empty())
            .map(|(elem, _)| elem)
            .collect()
    }

    /// Number of live elements.
    pub fn len(&self) -> usize {
        self.elements
            .iter()
            .filter(|(_, tags)| !tags.is_empty())
            .count()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Merge with another OR-Set.
    ///
    /// For each element, the live tags are the union of both sides' live tags
    /// minus the union of both sides' tombstones.
    pub fn merge(&mut self, other: &ORSet<T>) {
        // Merge tombstones first (union).
        for (elem, other_tombstones) in &other.tombstones {
            self.tombstones
                .entry(elem.clone())
                .or_default()
                .extend(other_tombstones);
        }

        // Merge elements: union of all tags, then remove tombstoned tags.
        for (elem, other_tags) in &other.elements {
            let entry = self.elements.entry(elem.clone()).or_default();
            entry.extend(other_tags);
        }

        // Clean up: remove tombstoned tags from live elements.
        for (elem, tombstoned) in &self.tombstones {
            if let Some(live_tags) = self.elements.get_mut(elem) {
                for tag in tombstoned {
                    live_tags.remove(tag);
                }
            }
        }
    }
}

impl<T: Eq + std::hash::Hash + Clone> Default for ORSet<T> {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// G-Counter (Grow-Only Counter)
// =============================================================================

/// A grow-only distributed counter.
///
/// Each node maintains its own monotonically increasing count. The counter
/// value is the sum across all nodes. Merge takes the max per node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GCounter {
    counts: HashMap<Uuid, i64>,
}

impl GCounter {
    /// Create a new counter.
    pub fn new() -> Self {
        Self {
            counts: HashMap::new(),
        }
    }

    /// Increment the counter for a specific node.
    pub fn increment(&mut self, node_id: Uuid, amount: i64) {
        assert!(amount >= 0, "GCounter only supports non-negative increments");
        *self.counts.entry(node_id).or_insert(0) += amount;
    }

    /// Get the total counter value (sum of all nodes).
    pub fn value(&self) -> i64 {
        self.counts.values().sum()
    }

    /// Get the count for a specific node.
    pub fn node_value(&self, node_id: &Uuid) -> i64 {
        self.counts.get(node_id).copied().unwrap_or(0)
    }

    /// Merge with another counter (take max per node).
    pub fn merge(&mut self, other: &GCounter) {
        for (node_id, &count) in &other.counts {
            let entry = self.counts.entry(*node_id).or_insert(0);
            *entry = (*entry).max(count);
        }
    }
}

impl Default for GCounter {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// MergeableState
// =============================================================================

/// A composite CRDT container for merging parallel agent outputs.
///
/// This holds named registers (LWW), sets (OR-Set), and counters (G-Counter)
/// under string keys, allowing agents to write to a shared state that can be
/// merged deterministically regardless of message ordering.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MergeableState {
    /// Named LWW registers for scalar JSON values.
    pub registers: HashMap<String, LWWRegister<serde_json::Value>>,
    /// Named OR-Sets for mutable string collections.
    pub sets: HashMap<String, ORSet<String>>,
    /// Named grow-only counters.
    pub counters: HashMap<String, GCounter>,
}

impl MergeableState {
    /// Create an empty mergeable state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a register value.
    pub fn set_register(
        &mut self,
        key: impl Into<String>,
        value: serde_json::Value,
        node_id: Uuid,
    ) {
        let key = key.into();
        let now = Utc::now();
        match self.registers.get_mut(&key) {
            Some(reg) => reg.set(value, now, node_id),
            None => {
                self.registers
                    .insert(key, LWWRegister::with_timestamp(value, now, node_id));
            }
        }
    }

    /// Get a register value.
    pub fn get_register(&self, key: &str) -> Option<&serde_json::Value> {
        self.registers.get(key).map(|r| r.value())
    }

    /// Add an element to a named set.
    pub fn add_to_set(&mut self, key: impl Into<String>, element: String) -> Uuid {
        self.sets.entry(key.into()).or_default().add(element)
    }

    /// Remove an element from a named set.
    pub fn remove_from_set(&mut self, key: &str, element: &str) -> bool {
        self.sets
            .get_mut(key)
            .map_or(false, |set| set.remove(&element.to_string()))
    }

    /// Get the elements in a named set.
    pub fn get_set(&self, key: &str) -> Vec<&String> {
        self.sets
            .get(key)
            .map_or_else(Vec::new, |set| set.elements())
    }

    /// Increment a named counter.
    pub fn increment_counter(&mut self, key: impl Into<String>, node_id: Uuid, amount: i64) {
        self.counters
            .entry(key.into())
            .or_default()
            .increment(node_id, amount);
    }

    /// Get a counter value.
    pub fn get_counter(&self, key: &str) -> i64 {
        self.counters.get(key).map_or(0, |c| c.value())
    }

    /// Merge with another `MergeableState`. This is the core CRDT merge --
    /// commutative, associative, and idempotent.
    pub fn merge(&mut self, other: &MergeableState) {
        // Merge registers.
        for (key, other_reg) in &other.registers {
            match self.registers.get_mut(key) {
                Some(reg) => reg.merge(other_reg),
                None => {
                    self.registers.insert(key.clone(), other_reg.clone());
                }
            }
        }

        // Merge sets.
        for (key, other_set) in &other.sets {
            match self.sets.get_mut(key) {
                Some(set) => set.merge(other_set),
                None => {
                    self.sets.insert(key.clone(), other_set.clone());
                }
            }
        }

        // Merge counters.
        for (key, other_counter) in &other.counters {
            match self.counters.get_mut(key) {
                Some(counter) => counter.merge(other_counter),
                None => {
                    self.counters.insert(key.clone(), other_counter.clone());
                }
            }
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    // -------------------------------------------------------------------------
    // LWWRegister tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_lww_register_newer_wins() {
        let node_a = Uuid::new_v4();
        let node_b = Uuid::new_v4();
        let t1 = Utc::now();
        let t2 = t1 + Duration::seconds(1);

        let mut reg = LWWRegister::with_timestamp("first".to_string(), t1, node_a);
        reg.merge(&LWWRegister::with_timestamp("second".to_string(), t2, node_b));

        assert_eq!(reg.value(), "second");
    }

    #[test]
    fn test_lww_register_older_loses() {
        let node_a = Uuid::new_v4();
        let node_b = Uuid::new_v4();
        let t1 = Utc::now();
        let t2 = t1 + Duration::seconds(1);

        let mut reg = LWWRegister::with_timestamp("second".to_string(), t2, node_a);
        reg.merge(&LWWRegister::with_timestamp("first".to_string(), t1, node_b));

        assert_eq!(reg.value(), "second");
    }

    #[test]
    fn test_lww_register_tiebreak_by_node_id() {
        let node_a = Uuid::from_bytes([0; 16]);
        let node_b = Uuid::from_bytes([255; 16]);
        let t = Utc::now();

        let mut reg = LWWRegister::with_timestamp("from_a".to_string(), t, node_a);
        reg.merge(&LWWRegister::with_timestamp("from_b".to_string(), t, node_b));

        // node_b has higher UUID, so it wins the tiebreak.
        assert_eq!(reg.value(), "from_b");
    }

    #[test]
    fn test_lww_register_merge_commutative() {
        let node_a = Uuid::new_v4();
        let node_b = Uuid::new_v4();
        let t1 = Utc::now();
        let t2 = t1 + Duration::seconds(1);

        let reg_a = LWWRegister::with_timestamp(10, t1, node_a);
        let reg_b = LWWRegister::with_timestamp(20, t2, node_b);

        let mut r1 = reg_a.clone();
        r1.merge(&reg_b);

        let mut r2 = reg_b.clone();
        r2.merge(&reg_a);

        assert_eq!(r1.value(), r2.value());
    }

    // -------------------------------------------------------------------------
    // GSet tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_gset_add_and_contains() {
        let mut set = GSet::new();
        set.insert("apple".to_string());
        set.insert("banana".to_string());

        assert!(set.contains(&"apple".to_string()));
        assert!(set.contains(&"banana".to_string()));
        assert!(!set.contains(&"cherry".to_string()));
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_gset_merge_union() {
        let mut set_a = GSet::new();
        set_a.insert("x".to_string());
        set_a.insert("y".to_string());

        let mut set_b = GSet::new();
        set_b.insert("y".to_string());
        set_b.insert("z".to_string());

        set_a.merge(&set_b);
        assert_eq!(set_a.len(), 3);
        assert!(set_a.contains(&"x".to_string()));
        assert!(set_a.contains(&"y".to_string()));
        assert!(set_a.contains(&"z".to_string()));
    }

    #[test]
    fn test_gset_merge_idempotent() {
        let mut set_a = GSet::new();
        set_a.insert("a".to_string());

        let set_b = set_a.clone();
        set_a.merge(&set_b);
        assert_eq!(set_a.len(), 1);
    }

    // -------------------------------------------------------------------------
    // ORSet tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_orset_add_remove() {
        let mut set = ORSet::new();
        set.add("hello".to_string());
        assert!(set.contains(&"hello".to_string()));

        set.remove(&"hello".to_string());
        assert!(!set.contains(&"hello".to_string()));
    }

    #[test]
    fn test_orset_concurrent_add_wins() {
        // Simulate: node A adds "x", node B adds "x", node A removes "x".
        // After merge, "x" should still be present because B's add was concurrent.
        let mut set_a = ORSet::new();
        let mut set_b = ORSet::new();

        set_a.add("x".to_string());
        set_b.add("x".to_string());

        // A removes its observed copy.
        set_a.remove(&"x".to_string());
        assert!(!set_a.contains(&"x".to_string()));

        // Merge: B's tag survives because it was not in A's tombstones.
        set_a.merge(&set_b);
        assert!(set_a.contains(&"x".to_string()));
    }

    #[test]
    fn test_orset_merge_commutative() {
        let mut set_a = ORSet::new();
        let mut set_b = ORSet::new();

        set_a.add("p".to_string());
        set_a.add("q".to_string());
        set_b.add("q".to_string());
        set_b.add("r".to_string());

        let mut merge_ab = set_a.clone();
        merge_ab.merge(&set_b);

        let mut merge_ba = set_b.clone();
        merge_ba.merge(&set_a);

        let mut elems_ab: Vec<_> = merge_ab.elements().into_iter().cloned().collect();
        let mut elems_ba: Vec<_> = merge_ba.elements().into_iter().cloned().collect();
        elems_ab.sort();
        elems_ba.sort();

        assert_eq!(elems_ab, elems_ba);
    }

    #[test]
    fn test_orset_remove_nonexistent() {
        let mut set = ORSet::<String>::new();
        assert!(!set.remove(&"ghost".to_string()));
    }

    // -------------------------------------------------------------------------
    // GCounter tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_gcounter_increment_and_value() {
        let node_a = Uuid::new_v4();
        let node_b = Uuid::new_v4();

        let mut counter = GCounter::new();
        counter.increment(node_a, 5);
        counter.increment(node_b, 3);
        counter.increment(node_a, 2);

        assert_eq!(counter.value(), 10);
        assert_eq!(counter.node_value(&node_a), 7);
        assert_eq!(counter.node_value(&node_b), 3);
    }

    #[test]
    fn test_gcounter_merge() {
        let node_a = Uuid::new_v4();
        let node_b = Uuid::new_v4();

        let mut counter_1 = GCounter::new();
        counter_1.increment(node_a, 5);
        counter_1.increment(node_b, 2);

        let mut counter_2 = GCounter::new();
        counter_2.increment(node_a, 3);
        counter_2.increment(node_b, 7);

        counter_1.merge(&counter_2);

        // max(5,3) + max(2,7) = 5 + 7 = 12
        assert_eq!(counter_1.value(), 12);
        assert_eq!(counter_1.node_value(&node_a), 5);
        assert_eq!(counter_1.node_value(&node_b), 7);
    }

    #[test]
    fn test_gcounter_merge_commutative() {
        let node = Uuid::new_v4();

        let mut c1 = GCounter::new();
        c1.increment(node, 10);

        let mut c2 = GCounter::new();
        c2.increment(node, 5);

        let mut m1 = c1.clone();
        m1.merge(&c2);

        let mut m2 = c2.clone();
        m2.merge(&c1);

        assert_eq!(m1.value(), m2.value());
    }

    // -------------------------------------------------------------------------
    // MergeableState tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_mergeable_state_registers() {
        let node = Uuid::new_v4();
        let mut state = MergeableState::new();

        state.set_register("summary", serde_json::json!("first draft"), node);
        assert_eq!(
            state.get_register("summary"),
            Some(&serde_json::json!("first draft"))
        );
    }

    #[test]
    fn test_mergeable_state_sets() {
        let mut state = MergeableState::new();
        state.add_to_set("tags", "important".to_string());
        state.add_to_set("tags", "urgent".to_string());

        let tags = state.get_set("tags");
        assert_eq!(tags.len(), 2);
    }

    #[test]
    fn test_mergeable_state_counters() {
        let node = Uuid::new_v4();
        let mut state = MergeableState::new();

        state.increment_counter("api_calls", node, 3);
        state.increment_counter("api_calls", node, 2);

        assert_eq!(state.get_counter("api_calls"), 5);
    }

    #[test]
    fn test_mergeable_state_full_merge() {
        let node_a = Uuid::new_v4();
        let node_b = Uuid::new_v4();

        let mut state_a = MergeableState::new();
        state_a.set_register("result", serde_json::json!("from A"), node_a);
        state_a.add_to_set("findings", "finding-1".to_string());
        state_a.increment_counter("tokens", node_a, 100);

        let mut state_b = MergeableState::new();
        state_b.set_register("result", serde_json::json!("from B"), node_b);
        state_b.add_to_set("findings", "finding-2".to_string());
        state_b.increment_counter("tokens", node_b, 200);

        state_a.merge(&state_b);

        // Register: depends on which timestamp is newer (both set at ~same time
        // in test, but the key thing is merge doesn't panic and one value wins).
        assert!(state_a.get_register("result").is_some());

        // Set: union of findings.
        let findings = state_a.get_set("findings");
        assert_eq!(findings.len(), 2);

        // Counter: sum across nodes.
        assert_eq!(state_a.get_counter("tokens"), 300);
    }

    #[test]
    fn test_mergeable_state_merge_commutative() {
        let node_a = Uuid::new_v4();
        let node_b = Uuid::new_v4();
        let t1 = Utc::now();
        let t2 = t1 + Duration::seconds(1);

        let mut state_a = MergeableState::new();
        state_a.registers.insert(
            "key".to_string(),
            LWWRegister::with_timestamp(serde_json::json!("A"), t1, node_a),
        );
        state_a.increment_counter("c", node_a, 5);

        let mut state_b = MergeableState::new();
        state_b.registers.insert(
            "key".to_string(),
            LWWRegister::with_timestamp(serde_json::json!("B"), t2, node_b),
        );
        state_b.increment_counter("c", node_b, 3);

        let mut merge_ab = state_a.clone();
        merge_ab.merge(&state_b);

        let mut merge_ba = state_b.clone();
        merge_ba.merge(&state_a);

        // Registers converge.
        assert_eq!(merge_ab.get_register("key"), merge_ba.get_register("key"));
        // Counters converge.
        assert_eq!(merge_ab.get_counter("c"), merge_ba.get_counter("c"));
    }

    #[test]
    fn test_mergeable_state_merge_idempotent() {
        let node = Uuid::new_v4();
        let mut state = MergeableState::new();
        state.set_register("x", serde_json::json!(42), node);
        state.add_to_set("s", "elem".to_string());
        state.increment_counter("n", node, 10);

        let snapshot = state.clone();
        state.merge(&snapshot);

        assert_eq!(state.get_register("x"), Some(&serde_json::json!(42)));
        assert_eq!(state.get_set("s").len(), 1);
        assert_eq!(state.get_counter("n"), 10);
    }
}
