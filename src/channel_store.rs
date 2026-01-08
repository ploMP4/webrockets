use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::Message;

pub(crate) struct ChannelStore {
    next_id: AtomicU64,
    data: DashMap<u64, Arc<mpsc::Sender<Arc<Message>>>>,
    grouped: DashMap<String, Vec<Arc<mpsc::Sender<Arc<Message>>>>>,
}

impl ChannelStore {
    pub(crate) fn new() -> Self {
        Self {
            next_id: AtomicU64::new(1),
            data: DashMap::new(),
            grouped: DashMap::new(),
        }
    }

    pub(crate) fn register(&self, group: &str, tx: Arc<mpsc::Sender<Arc<Message>>>) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);

        self.data.insert(id, Arc::clone(&tx));
        self.grouped
            .entry(group.to_string())
            .or_default()
            .push(tx);

        id
    }

    pub(crate) fn remove(&self, id: u64) {
        self.data.remove(&id);
    }

    pub(crate) fn broadcast(&self, groups: &[String], msg: Arc<Message>) {
        let capacity: usize = groups
            .iter()
            .filter_map(|g| self.grouped.get(g).map(|e| e.len()))
            .sum();

        if capacity == 0 {
            return;
        }

        let mut blocked_senders = Vec::new();
        for group in groups {
            if let Some(entry) = self.grouped.get(group) {
                for tx in entry.iter() {
                    match tx.try_send(Arc::clone(&msg)) {
                        Ok(()) => {}
                        Err(mpsc::error::TrySendError::Full(_)) => {
                            blocked_senders.push(Arc::clone(tx));
                        }
                        Err(mpsc::error::TrySendError::Closed(_)) => {}
                    }
                }
            }
        }

        if !blocked_senders.is_empty() {
            tokio::spawn(async move {
                for tx in blocked_senders {
                    let _ = tx.send(Arc::clone(&msg)).await;
                }
            });
        }
    }
}
