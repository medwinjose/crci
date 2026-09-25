use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub trait LegacyTransport {
    fn send(&self, to: &str, data: &[u8]);
    fn node_id(&self) -> &str;
}

pub type SharedInbox = Arc<Mutex<HashMap<String, Vec<(String, Vec<u8>)>>>>;

pub struct SimTransport {
    pub node_id: String,
    pub inbox: SharedInbox,
}

impl SimTransport {
    pub fn new(node_id: &str, inbox: SharedInbox) -> SimTransport {
        SimTransport {
            node_id: node_id.to_string(),
            inbox,
        }
    }
}

impl LegacyTransport for SimTransport {
    fn send(&self, to: &str, data: &[u8]) {
        let mut inbox = match self.inbox.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        inbox
            .entry(to.to_string())
            .or_default()
            .push((self.node_id.clone(), data.to_vec()));
    }

    fn node_id(&self) -> &str {
        &self.node_id
    }
}
