use std::collections::{HashSet, VecDeque};

pub struct DedupCache {
    capacity: usize,
    order: VecDeque<String>,
    set: HashSet<String>,
}

impl DedupCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            order: VecDeque::new(),
            set: HashSet::new(),
        }
    }

    pub fn insert(&mut self, hash: &str) -> bool {
        if self.set.contains(hash) {
            return false;
        }
        if self.order.len() >= self.capacity {
            if let Some(oldest) = self.order.pop_front() {
                self.set.remove(&oldest);
            }
        }
        let owned = hash.to_string();
        self.order.push_back(owned.clone());
        self.set.insert(owned);
        true
    }

    pub fn len(&self) -> usize {
        self.set.len()
    }

    pub fn is_empty(&self) -> bool {
        self.set.is_empty()
    }
}
