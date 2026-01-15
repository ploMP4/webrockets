use dashmap::{DashMap, DashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::Message;

type ConnectionID = u64;

pub(crate) struct ChannelStore {
    next_id: AtomicU64,
    senders: DashMap<ConnectionID, Arc<mpsc::Sender<Arc<Message>>>>,
    groups: DashMap<Arc<str>, DashSet<ConnectionID>>,
}

impl ChannelStore {
    pub(crate) fn new() -> Self {
        Self {
            next_id: AtomicU64::new(1),
            senders: DashMap::new(),
            groups: DashMap::new(),
        }
    }

    pub(crate) fn register(&self, tx: Arc<mpsc::Sender<Arc<Message>>>) -> ConnectionID {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.senders.insert(id, tx);
        id
    }

    pub(crate) fn register_with_group(
        &self,
        group: &str,
        tx: Arc<mpsc::Sender<Arc<Message>>>,
    ) -> u64 {
        let id = self.register(tx);
        self.join(id, group);
        id
    }

    pub(crate) fn join(&self, conn_id: ConnectionID, group: &str) -> bool {
        self.groups.entry(group.into()).or_default().insert(conn_id)
    }

    pub(crate) fn leave(&self, conn_id: ConnectionID, group: &str) -> bool {
        self.groups
            .get(group)
            .is_some_and(|members| members.remove(&conn_id).is_some())
    }

    pub(crate) fn get_groups(&self, conn_id: ConnectionID) -> Vec<String> {
        self.groups
            .iter()
            .filter(|members| members.contains(&conn_id))
            .map(|members| members.key().to_string())
            .collect()
    }

    pub(crate) fn group_size(&self, group: &str) -> usize {
        self.groups
            .get(group)
            .map(|members| members.len())
            .unwrap_or(0)
    }

    pub(crate) fn remove(&self, conn_id: ConnectionID) {
        self.senders.remove(&conn_id);
        for group in self.groups.iter() {
            group.remove(&conn_id);
        }
    }

    pub(crate) fn broadcast(
        &self,
        groups: &[String],
        msg: Arc<Message>,
        exclude: Option<ConnectionID>,
    ) {
        let mut blocked_senders = Vec::new();

        for group in groups {
            let Some(group_members) = self.groups.get(group.as_str()) else {
                continue;
            };

            for conn_id in group_members.iter() {
                let conn_id = *conn_id;

                if exclude == Some(conn_id) {
                    continue;
                }

                if let Some(sender) = self.senders.get(&conn_id) {
                    match sender.try_send(Arc::clone(&msg)) {
                        Ok(()) => {}
                        Err(mpsc::error::TrySendError::Full(_)) => {
                            blocked_senders.push(Arc::clone(&sender));
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
