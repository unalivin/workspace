use std::collections::VecDeque;
use std::sync::{Condvar, Mutex};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FullPolicy {
    BlockReader,
    DropNewest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct QueueCounters {
    pub accepted: u64,
    pub dropped: u64,
    pub blocked: u64,
}

pub struct BoundedIntake<T> {
    capacity: usize,
    state: Mutex<State<T>>,
    readable: Condvar,
    writable: Condvar,
}

struct State<T> {
    items: VecDeque<T>,
    counters: QueueCounters,
}

impl<T> BoundedIntake<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            state: Mutex::new(State {
                items: VecDeque::new(),
                counters: QueueCounters::default(),
            }),
            readable: Condvar::new(),
            writable: Condvar::new(),
        }
    }

    pub fn push(&self, item: T, policy: FullPolicy, timeout: Duration) -> Result<(), T> {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if policy == FullPolicy::DropNewest && state.items.len() >= self.capacity {
            state.counters.dropped += 1;
            return Err(item);
        }
        while state.items.len() >= self.capacity {
            state.counters.blocked += 1;
            let result = self
                .writable
                .wait_timeout(state, timeout)
                .unwrap_or_else(|error| error.into_inner());
            state = result.0;
            if result.1.timed_out() && state.items.len() >= self.capacity {
                return Err(item);
            }
        }
        state.items.push_back(item);
        state.counters.accepted += 1;
        self.readable.notify_one();
        Ok(())
    }

    pub fn pop(&self, timeout: Duration) -> Option<T> {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        while state.items.is_empty() {
            let result = self
                .readable
                .wait_timeout(state, timeout)
                .unwrap_or_else(|error| error.into_inner());
            state = result.0;
            if result.1.timed_out() && state.items.is_empty() {
                return None;
            }
        }
        let item = state.items.pop_front();
        self.writable.notify_one();
        item
    }

    pub fn counters(&self) -> QueueCounters {
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .counters
    }

    pub fn len(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .items
            .len()
    }
}
