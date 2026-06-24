//! `nova-bus` — NovaCore's typed event bus.
//!
//! A multi-producer broadcast: every [`Subscriber`] receives a clone of each
//! published [`Event`]. The hot path (frame redraw, process output) and the
//! fan-out path (config/plugin changes) share one simple, lock-light API.

#![forbid(unsafe_code)]

use crossbeam_channel::{unbounded, Receiver, Sender};
use std::sync::{Arc, Mutex};

/// Engine-level events the UI and plugins subscribe to.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    SessionSpawned {
        session: u64,
    },
    SessionClosed {
        session: u64,
    },
    /// A pipeline produced a value (carried as its text view to keep the bus
    /// `Clone + Send` without depending on `nova-value`).
    ValueProduced {
        session: u64,
        summary: String,
    },
    ProcessExited {
        pid: u32,
        code: i32,
    },
    HistoryAppended {
        session: u64,
        source: String,
    },
    WorkspaceChanged,
    Redraw,
}

/// A subscription handle; drop it to unsubscribe.
pub struct Subscriber {
    rx: Receiver<Event>,
}

impl Subscriber {
    /// Non-blocking receive of the next event, if any.
    pub fn try_next(&self) -> Option<Event> {
        self.rx.try_recv().ok()
    }
    /// Blocking receive.
    pub fn next(&self) -> Option<Event> {
        self.rx.recv().ok()
    }
    pub fn drain(&self) -> Vec<Event> {
        let mut out = Vec::new();
        while let Ok(e) = self.rx.try_recv() {
            out.push(e);
        }
        out
    }
}

/// The bus. Cheap to clone (shared sender list).
#[derive(Clone, Default)]
pub struct EventBus {
    subscribers: Arc<Mutex<Vec<Sender<Event>>>>,
}

impl EventBus {
    #[must_use]
    pub fn new() -> Self {
        EventBus::default()
    }

    #[must_use]
    pub fn subscribe(&self) -> Subscriber {
        let (tx, rx) = unbounded();
        self.subscribers.lock().unwrap().push(tx);
        Subscriber { rx }
    }

    /// Publish to all live subscribers, pruning closed ones.
    pub fn publish(&self, event: Event) {
        let mut subs = self.subscribers.lock().unwrap();
        subs.retain(|tx| tx.send(event.clone()).is_ok());
    }

    #[must_use]
    pub fn subscriber_count(&self) -> usize {
        self.subscribers.lock().unwrap().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn broadcast_to_all_subscribers() {
        let bus = EventBus::new();
        let a = bus.subscribe();
        let b = bus.subscribe();
        bus.publish(Event::Redraw);
        bus.publish(Event::WorkspaceChanged);
        assert_eq!(a.drain(), vec![Event::Redraw, Event::WorkspaceChanged]);
        assert_eq!(b.drain(), vec![Event::Redraw, Event::WorkspaceChanged]);
    }

    #[test]
    fn dropped_subscriber_is_pruned() {
        let bus = EventBus::new();
        {
            let _tmp = bus.subscribe();
            assert_eq!(bus.subscriber_count(), 1);
        }
        // Next publish prunes the dropped subscriber's closed channel.
        bus.publish(Event::Redraw);
        assert_eq!(bus.subscriber_count(), 0);
    }
}
