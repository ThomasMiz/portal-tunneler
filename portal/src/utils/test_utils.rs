use std::{
    fmt::Debug,
    ops::{Deref, DerefMut},
    sync::{atomic::AtomicBool, Arc},
};

pub struct DropChecker {
    tracked: Vec<Arc<DropCheckTracker>>,
}

impl DropChecker {
    pub const fn new() -> Self {
        Self { tracked: Vec::new() }
    }

    pub fn track<T: ToString>(&mut self, value: T) -> DC<T> {
        let tracker = Arc::new(DropCheckTracker {
            was_dropped: AtomicBool::new(false),
            name: Some(value.to_string()),
        });

        self.tracked.push(Arc::clone(&tracker));
        DC { tracker, value }
    }

    pub fn track_named<T, S: ToString>(&mut self, name: S, value: T) -> DC<T> {
        let tracker = Arc::new(DropCheckTracker {
            was_dropped: AtomicBool::new(false),
            name: Some(name.to_string()),
        });

        self.tracked.push(Arc::clone(&tracker));
        DC { tracker, value }
    }

    pub fn track_unnamed<T>(&mut self, value: T) -> DC<T> {
        let tracker = Arc::new(DropCheckTracker {
            was_dropped: AtomicBool::new(false),
            name: None,
        });

        self.tracked.push(Arc::clone(&tracker));
        DC { tracker, value }
    }

    pub fn ensure_all_dropped(&mut self) {
        for t in self.tracked.drain(..) {
            if !t.was_dropped.load(std::sync::atomic::Ordering::Relaxed) {
                match &t.name {
                    None => panic!("An unnamed value wasn't dropped"),
                    Some(name) => panic!("A value named {name} wasn't dropped"),
                }
            }
        }
    }
}

struct DropCheckTracker {
    was_dropped: AtomicBool,
    name: Option<String>,
}

impl Drop for DropChecker {
    fn drop(&mut self) {
        self.ensure_all_dropped()
    }
}

pub struct DC<T> {
    tracker: Arc<DropCheckTracker>,
    pub value: T,
}

impl<T> Deref for DC<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> DerefMut for DC<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

impl<T> Drop for DC<T> {
    fn drop(&mut self) {
        if self.tracker.was_dropped.swap(true, std::sync::atomic::Ordering::Relaxed) {
            match &self.tracker.name {
                None => panic!("An unnamed value was double-dropped"),
                Some(name) => panic!("A value named {name} was double-dropped"),
            }
        }
    }
}

impl<T: Debug> Debug for DC<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self.deref(), f)
    }
}

impl<T: PartialEq> PartialEq for DC<T> {
    fn eq(&self, other: &Self) -> bool {
        self.value.eq(&other.value)
    }
}

impl<T: Eq> Eq for DC<T> {}
