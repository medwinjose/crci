// ─── Transport Trait ─────────────────────────────────────────────────────────
// In real life: this is the radio inside the phone. It could be Bluetooth,
// WiFi Direct, or LoRa. The node logic never changes regardless of which
// radio is underneath — it just calls send() and the trait handles the rest.
//
// Technical: a trait in Rust is like an interface in other languages.
// Any type that implements Transport must provide these two functions.
// This lets us swap the real radio for a simulation without changing
// any node logic — the node just sees "something I can send bytes through."

#[allow(dead_code)]
pub trait Transport {
    // Send raw bytes to a specific peer by their ID
    fn send(&self, to: &str, data: &[u8]);

    // Return this transport's own node ID
    fn node_id(&self) -> &str;
}

// ─── Simulated Transport ─────────────────────────────────────────────────────
// In real life: this doesn't exist. In simulation: it's a fake radio that
// delivers bytes directly in memory, letting us test mesh behaviour without
// actual Bluetooth hardware.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// A shared inbox — bytes waiting to be processed by each node
// Arc = shared ownership across threads
// Mutex = only one thread can read/write at a time (prevents corruption)
pub type SharedInbox = Arc<Mutex<HashMap<String, Vec<Vec<u8>>>>>;

pub struct SimTransport {
    #[allow(dead_code)]
    pub node_id: String,
    pub inbox: SharedInbox, // shared across all nodes in the simulation
}

impl SimTransport {
    pub fn new(node_id: &str, inbox: SharedInbox) -> SimTransport {
        SimTransport {
            node_id: node_id.to_string(),
            inbox,
        }
    }
}

impl Transport for SimTransport {
    fn send(&self, to: &str, data: &[u8]) {
        // Drop bytes into the recipient's inbox
        let mut inbox = self.inbox.lock().unwrap();
        inbox.entry(to.to_string()).or_default().push(data.to_vec());
    }

    #[allow(dead_code)]
    fn node_id(&self) -> &str {
        &self.node_id
    }
}
