struct Node {
    id: String,
    is_honest: bool,
    reputation: f64,
}

impl Node {
    fn new(id: &str, is_honest: bool) -> Node {
        Node {
            id: id.to_string(),
            is_honest,
            reputation: 1.0,
        }
    }

    fn penalize(&mut self) {
        self.reputation -= 0.2;
        if self.reputation < 0.0 {
            self.reputation = 0.0;
        }
    }

    fn is_trusted(&self) -> bool {
    self.reputation > 0.41
    }

    fn status(&self) {
        let trust_label = if self.is_trusted() { "TRUSTED" } else { "ISOLATED" };
        println!("ID: {:10} | Honest: {:5} | Reputation: {:.2} | Status: {}",
            self.id, self.is_honest.to_string(), self.reputation, trust_label);
    }
}

fn main() {
    let mut nodes = vec![
        Node::new("node-001", true),
        Node::new("node-002", true),
        Node::new("node-003", false),  // Byzantine node
        Node::new("node-004", true),
    ];

    println!("=== CRCI Network — Initial State ===");
    for node in &nodes {
        node.status();
    }

    // Simulate detection: node-003 gets caught lying 3 times
    println!("\n=== Detecting Byzantine Behaviour on node-003 ===");
    for i in 1..=3 {
        nodes[2].penalize();
        println!("Strike {}: reputation now {:.2}", i, nodes[2].reputation);
    }

    println!("\n=== CRCI Network — After Detection ===");
    for node in &nodes {
        node.status();
    }
}