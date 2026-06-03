// ─── Crisis Scenario Simulations ─────────────────────────────────────────────
// Each function runs a complete crisis event through the mesh, exposing how
// the system behaves under real-world conditions. Not tests — demonstrations.

use crate::mesh::MeshSimulator;
use crate::message::{Message, Signal, Visibility};
use crate::transport::SharedInbox;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// ─── Scenario 1: Flash Flood — Tamil Nadu Coastline ──────────────────────────
// Rising water cuts off nodes progressively. Some nodes report from indoors.
// One node is lying ("my area is safe" while surrounded by flood reports).
// One node's phone dies mid-crisis. GPS is partially available.
// Real question: does the network maintain situational awareness as nodes die?

pub fn scenario_flood() {
    println!("\n╔══════════════════════════════════════════════════════╗");
    println!("║  SCENARIO 1 — Flash Flood, Tamil Nadu Coast          ║");
    println!("║  T+0: Cyclone makes landfall. Storm surge incoming.   ║");
    println!("╚══════════════════════════════════════════════════════╝\n");

    let inbox: SharedInbox = Arc::new(Mutex::new(HashMap::new()));
    let mut sim = MeshSimulator::new();

    // Coastal village: 8 civilians, 1 liar, 1 will go offline
    sim.add_node("ravi", "coastal-north", inbox.clone()); // fisherman, outdoor
    sim.add_node("priya", "coastal-north", inbox.clone()); // teacher, indoor
    sim.add_node("kumar", "coastal-south", inbox.clone()); // shopkeeper
    sim.add_node("meera", "coastal-south", inbox.clone()); // liar — politically motivated to downplay
    sim.add_node("arjun", "inland-relief", inbox.clone()); // relief coordinator, can help
    sim.add_node("lakshmi", "inland-relief", inbox.clone()); // nurse, can help
    sim.add_node("selvam", "coastal-north", inbox.clone()); // will lose phone in flood
    sim.add_node("district-control", "inland-relief", inbox.clone()); // official node

    sim.connect("ravi", "priya");
    sim.connect("priya", "kumar");
    sim.connect("kumar", "meera");
    sim.connect("meera", "arjun");
    sim.connect("arjun", "lakshmi");
    sim.connect("lakshmi", "district-control");
    sim.connect("ravi", "selvam");

    println!("=== T+0: Cyclone makes landfall ===");
    println!("Network: 8 nodes across coastal and inland zones\n");

    // Round 1: T+15 minutes — surge begins
    println!("--- T+15 min: Storm surge reaches coastal zone ---");

    sim.originate("ravi", {
        let mut m = Message::new(
            "r1",
            "ravi",
            Signal::new(4, true, false, true, 5, Visibility::Direct).with_gps(11.3410, 79.8012),
            Some("water rising fast, knee deep on main street"),
        );
        m.priority = crate::message::MessagePriority::High;
        m
    });

    sim.originate(
        "priya",
        Message::new(
            "r2",
            "priya",
            Signal::new(2, false, false, false, 2, Visibility::Unknown).with_gps(11.3415, 79.8018),
            Some("indoors, can hear water, cannot see outside"),
        ),
    );

    sim.originate("selvam", {
        let mut m = Message::new(
            "r3",
            "selvam",
            Signal::new(5, true, false, true, 5, Visibility::Direct),
            Some("boat torn loose, water entering ground floor"),
        );
        m.priority = crate::message::MessagePriority::Critical;
        m
    });

    // Meera lies — politically motivated to downplay disaster response
    sim.originate(
        "meera",
        Message::new(
            "r4",
            "meera",
            Signal::new(1, false, true, false, 5, Visibility::Direct),
            Some("situation manageable, no evacuation needed"),
        ),
    );

    sim.originate(
        "arjun",
        Message::new(
            "r5",
            "arjun",
            Signal::new(1, false, true, true, 5, Visibility::Direct)
                .with_gps(11.3180, 79.7940)
                .with_resource("transport"),
            Some("inland relief camp ready, have 3 trucks available"),
        ),
    );

    sim.drain();
    println!("\n--- Consensus Round 1 ---");
    sim.run_consensus();
    sim.new_round();

    // Round 2: T+45 minutes — selvam's phone submerged
    println!("\n--- T+45 min: Selvam's phone submerged by rising water ---");
    if let Some(n) = sim.nodes.get_mut("selvam") {
        n.go_offline();
    }

    sim.originate("ravi", {
        let mut m = Message::rescue(
            "rescue-ravi",
            "ravi",
            Some("trapped on roof, 4 family members, GPS: 11.3410,79.8012"),
        );
        m.signal = m.signal.with_gps(11.3410, 79.8012);
        m
    });

    sim.originate(
        "kumar",
        Message::new(
            "r6",
            "kumar",
            Signal::new(5, true, false, true, 5, Visibility::Direct).with_gps(11.3390, 79.8005),
            Some("entire street underwater, 12 families need boats"),
        ),
    );

    sim.originate(
        "meera",
        Message::new(
            "r7",
            "meera",
            Signal::new(1, false, true, false, 5, Visibility::Direct),
            Some("overreaction, drainage systems handling it"),
        ),
    );

    sim.originate(
        "arjun",
        Message::new(
            "r8",
            "arjun",
            Signal::new(1, false, true, true, 5, Visibility::Direct).with_resource("transport"),
            Some("dispatching trucks, need GPS coordinates"),
        ),
    );

    sim.drain();
    println!("\n--- Consensus Round 2 (meera flagged, selvam rescue kept alive) ---");
    sim.run_consensus();

    // Show rescued state
    println!("\n=== Flood Scenario — Final Network State ===");
    sim.print_all();

    // Resource matching report
    println!("\n=== Resource Matching Report ===");
    let mut help_needed: HashMap<String, String> = HashMap::new();
    let mut help_available: HashMap<String, String> = HashMap::new();
    for node in sim.nodes.values() {
        for msg in node.persistent_messages.values() {
            if msg.needs_help {
                help_needed.insert(msg.origin.clone(), msg.note.clone().unwrap_or_default());
            }
            if msg.can_help_others {
                help_available.insert(msg.origin.clone(), msg.note.clone().unwrap_or_default());
            }
        }
    }
    println!("Nodes needing help: {}", help_needed.len());
    for (origin_id, note) in &help_needed {
        println!("  🆘 [{}]: {}", origin_id, &note[..note.len().min(60)]);
    }
    println!("Nodes that can help: {}", help_available.len());
    for (origin_id, note) in &help_available {
        println!("  ✅ [{}]: {}", origin_id, &note[..note.len().min(60)]);
    }
}

// ─── Scenario 2: Earthquake — Türkiye, 04:17 AM ──────────────────────────────
// Most people asleep. Sudden mass casualty event. Many simultaneous panic buttons.
// GPS disrupted by infrastructure damage. Many nodes go offline at once.
// Aftershock at T+22 minutes causes secondary collapses.
// Real question: does MCE declare correctly? Does the system survive mass simultaneous offline?

pub fn scenario_earthquake() {
    println!("\n╔══════════════════════════════════════════════════════╗");
    println!("║  SCENARIO 2 — Earthquake M7.8, Türkiye, 04:17 AM     ║");
    println!("║  Most people asleep. Infrastructure destroyed.        ║");
    println!("╚══════════════════════════════════════════════════════╝\n");

    let inbox: SharedInbox = Arc::new(Mutex::new(HashMap::new()));
    let mut sim = MeshSimulator::new();

    // Apartment block — everyone in the same zone, different floors
    for i in 1..=10usize {
        let id = format!("apt-{:02}", i);
        sim.add_node(&id, "block-a", inbox.clone());
    }
    // Search and rescue team nearby
    sim.add_node("sar-001", "rescue-staging", inbox.clone());
    sim.add_node("sar-002", "rescue-staging", inbox.clone());

    // Linear connectivity — mesh through the building
    for i in 1..9usize {
        sim.connect(&format!("apt-{:02}", i), &format!("apt-{:02}", i + 1));
    }
    sim.connect("apt-09", "sar-001");
    sim.connect("sar-001", "sar-002");

    println!("=== T+0: M7.8 earthquake strikes ===");
    println!("10 residents in apartment block + 2 SAR teams\n");

    // T+0: Panic buttons triggered — 7 people trapped, 3 phones destroyed immediately
    for i in 1..=7usize {
        let id = format!("apt-{:02}", i);
        let msg_id = format!("panic-{:02}", i);
        let note = match i {
            1 => Some("floor collapsed, 3 people trapped"),
            2 => Some("pinned under debris, leg injury"),
            3 => Some("gas smell, fire risk"),
            5 => Some("family of 5, two children injured"),
            _ => None,
        };
        sim.originate(&id, Message::rescue(&msg_id, &id, note));
    }

    // 3 phones instantly destroyed (go offline)
    println!("  ⚡ apt-08, apt-09, apt-10 phones destroyed by collapse");
    for i in [8usize, 9, 10] {
        let id = format!("apt-{:02}", i);
        if let Some(n) = sim.nodes.get_mut(&id) {
            n.go_offline();
        }
    }

    // SAR team reports what they observe
    sim.originate(
        "sar-001",
        Message::new(
            "sar-obs-1",
            "sar-001",
            Signal::new(5, false, true, true, 5, Visibility::Direct).with_gps(37.5765, 36.9228),
            Some("multiple structural collapses visible, dispatching teams"),
        ),
    );

    sim.drain();
    println!("\n--- Consensus T+0 (MCE should declare) ---");
    sim.run_consensus();
    sim.new_round();

    // T+22 minutes: Aftershock M5.2
    println!("\n--- T+22 min: Aftershock M5.2 — secondary collapses ---");
    if let Some(n) = sim.nodes.get_mut("apt-04") {
        n.go_offline();
    }
    if let Some(n) = sim.nodes.get_mut("apt-06") {
        n.go_offline();
    }

    // Remaining survivors send updated status
    sim.originate("apt-01", {
        let mut m = Message::rescue(
            "panic-01-update",
            "apt-01",
            Some("aftershock worsened collapse, now 5 people confirmed trapped"),
        );
        m.signal = m.signal.with_gps(37.5762, 36.9225);
        m
    });

    sim.originate(
        "sar-002",
        Message::new(
            "sar-obs-2",
            "sar-002",
            Signal::new(5, false, true, true, 5, Visibility::Direct),
            Some("aftershock caused 2 additional collapses, need more teams"),
        ),
    );

    sim.drain();
    println!("\n--- Consensus T+22 (rescue msgs from offline nodes kept alive) ---");
    sim.run_consensus();

    println!("\n=== Earthquake Scenario — Rescue Request Survival Check ===");
    for (id, node) in &sim.nodes {
        let rescue_count = node
            .persistent_messages
            .values()
            .filter(|m| m.message_type == "rescue")
            .count();
        if rescue_count > 0 || !node.is_online {
            println!(
                "  [{}] online:{} rescue_msgs_held:{}",
                id, node.is_online, rescue_count
            );
        }
    }
}

// ─── Scenario 3: Urban Conflict — Active Combat Zone ─────────────────────────
// Enemy nodes deliberately injected into the network claiming false "all clear."
// Signal jamming periodically cuts sections of the mesh.
// Civilians and military nodes coexist — priorities conflict.
// Real question: does Byzantine detection catch coordinated false-flag actors?

pub fn scenario_conflict() {
    println!("\n╔══════════════════════════════════════════════════════╗");
    println!("║  SCENARIO 3 — Urban Conflict, Active Combat Zone      ║");
    println!("║  3 enemy-controlled nodes seeding false intelligence  ║");
    println!("╚══════════════════════════════════════════════════════╝\n");

    let inbox: SharedInbox = Arc::new(Mutex::new(HashMap::new()));
    let mut sim = MeshSimulator::new();

    // Civilian zone
    sim.add_node("civ-001", "district-7", inbox.clone());
    sim.add_node("civ-002", "district-7", inbox.clone());
    sim.add_node("civ-003", "district-7", inbox.clone());
    sim.add_node("civ-004", "district-7", inbox.clone());
    // Enemy-injected nodes — look like civilians, report false "safe"
    sim.add_node("enemy-a", "district-7", inbox.clone());
    sim.add_node("enemy-b", "district-7", inbox.clone());
    sim.add_node("enemy-c", "district-7", inbox.clone());
    // UN observer team
    sim.add_node("un-001", "district-7", inbox.clone());

    sim.connect("civ-001", "civ-002");
    sim.connect("civ-002", "enemy-a");
    sim.connect("enemy-a", "civ-003");
    sim.connect("civ-003", "enemy-b");
    sim.connect("enemy-b", "civ-004");
    sim.connect("civ-004", "enemy-c");
    sim.connect("enemy-c", "un-001");

    println!("=== Active conflict — 3 enemy nodes seeding false intelligence ===\n");

    // Round 1: Enemy nodes outnumbered 4:3 — honest majority should win
    sim.originate(
        "civ-001",
        Message::new(
            "c1",
            "civ-001",
            Signal::new(5, true, false, true, 5, Visibility::Direct),
            Some("active shelling, building on fire, need evacuation"),
        ),
    );

    sim.originate(
        "civ-002",
        Message::new(
            "c2",
            "civ-002",
            Signal::new(5, true, false, true, 5, Visibility::Direct),
            Some("sniper fire, cannot leave building"),
        ),
    );

    sim.originate(
        "civ-003",
        Message::new(
            "c3",
            "civ-003",
            Signal::new(4, true, false, true, 4, Visibility::Direct),
            Some("explosions heard, indirect fire in area"),
        ),
    );

    sim.originate(
        "civ-004",
        Message::new(
            "c4",
            "civ-004",
            Signal::new(5, true, false, true, 5, Visibility::Direct),
            Some("casualties on street, medical needed urgently"),
        ),
    );

    // Enemy nodes report false all-clear
    for (id, msg_id) in [("enemy-a", "e1"), ("enemy-b", "e2"), ("enemy-c", "e3")] {
        sim.originate(
            id,
            Message::new(
                msg_id,
                id,
                Signal::new(1, false, true, false, 5, Visibility::Direct),
                Some("area secure, no threat observed"),
            ),
        );
    }

    sim.originate(
        "un-001",
        Message::new(
            "un1",
            "un-001",
            Signal::new(4, false, true, true, 4, Visibility::Direct),
            Some("UN observer: confirmed civilian distress, requesting corridor"),
        ),
    );

    sim.drain();
    println!("--- Consensus Round 1 (enemy nodes should be flagged) ---");
    sim.run_consensus();
    sim.print_all();
}

// ─── Scenario 4: Industrial Disaster — Chemical Plant Explosion ───────────────
// Hazmat situation. Wind direction matters. Some zones should evacuate,
// others should shelter-in-place. Visibility field critically important here.
// First responders need to coordinate without entering contaminated zone.
// Real question: does zone isolation prevent cross-zone penalty contamination?

pub fn scenario_hazmat() {
    println!("\n╔══════════════════════════════════════════════════════╗");
    println!("║  SCENARIO 4 — Chemical Plant Explosion, Evacuation   ║");
    println!("║  Wind: NE. Contamination plume moves SW.              ║");
    println!("╚══════════════════════════════════════════════════════╝\n");

    let inbox: SharedInbox = Arc::new(Mutex::new(HashMap::new()));
    let mut sim = MeshSimulator::new();

    // Contamination zone (upwind) — severity 5
    sim.add_node("worker-1", "plant-zone", inbox.clone());
    sim.add_node("worker-2", "plant-zone", inbox.clone());
    sim.add_node("security-1", "plant-zone", inbox.clone());
    // Evacuation zone (downwind) — severity 4, people need to leave
    sim.add_node("resident-a", "evac-zone", inbox.clone());
    sim.add_node("resident-b", "evac-zone", inbox.clone());
    sim.add_node("resident-c", "evac-zone", inbox.clone());
    // Safe zone (perpendicular to wind) — severity 1 honestly
    sim.add_node("safe-x", "clear-zone", inbox.clone());
    sim.add_node("safe-y", "clear-zone", inbox.clone());
    sim.add_node("safe-z", "clear-zone", inbox.clone());
    // First responders — staging at perimeter
    sim.add_node("hazmat-1", "perimeter", inbox.clone());
    sim.add_node("hazmat-2", "perimeter", inbox.clone());

    sim.connect("worker-1", "worker-2");
    sim.connect("worker-2", "security-1");
    sim.connect("security-1", "resident-a");
    sim.connect("resident-a", "resident-b");
    sim.connect("resident-b", "resident-c");
    sim.connect("resident-c", "hazmat-1");
    sim.connect("hazmat-1", "hazmat-2");
    sim.connect("safe-x", "safe-y");
    sim.connect("safe-y", "safe-z");

    println!("=== Chemical plant explosion — multi-zone crisis ===");
    println!("plant-zone: contaminated. evac-zone: at risk. clear-zone: safe.\n");

    // Plant zone: max severity, immediate rescue
    sim.originate(
        "worker-1",
        Message::rescue(
            "w-panic",
            "worker-1",
            Some("explosion, chemical burn, cannot evacuate, need hazmat rescue"),
        ),
    );
    sim.originate(
        "worker-2",
        Message::new(
            "w2",
            "worker-2",
            Signal::new(5, true, false, true, 5, Visibility::Direct)
                .with_gps(51.4912, -0.1442)
                .with_resource("medical"),
            Some("acid cloud visible, multiple workers down"),
        ),
    );
    sim.originate(
        "security-1",
        Message::new(
            "s1",
            "security-1",
            Signal::new(5, false, false, true, 5, Visibility::Direct),
            Some("plant sealed, shelter in place for all remaining staff"),
        ),
    );

    // Evac zone: severity 4 — get out
    for (id, msg_id, note) in [
        (
            "resident-a",
            "ra",
            "smell chemical, eyes burning, evacuating",
        ),
        (
            "resident-b",
            "rb",
            "family evacuating on foot, need transport",
        ),
        (
            "resident-c",
            "rc",
            "elderly parent cannot walk, need medical transport",
        ),
    ] {
        sim.originate(
            id,
            Message::new(
                msg_id,
                id,
                Signal::new(4, true, false, true, 4, Visibility::Direct).with_resource("transport"),
                Some(note),
            ),
        );
    }

    // Clear zone: honestly severity 1 — this should NOT be penalised
    for (id, msg_id) in [("safe-x", "sx"), ("safe-y", "sy"), ("safe-z", "sz")] {
        sim.originate(
            id,
            Message::new(
                msg_id,
                id,
                Signal::new(1, false, true, true, 5, Visibility::Direct).with_resource("shelter"),
                Some("area clear of contamination, can shelter evacuees"),
            ),
        );
    }

    // Hazmat team
    sim.originate(
        "hazmat-1",
        Message::new(
            "h1",
            "hazmat-1",
            Signal::new(1, false, true, true, 5, Visibility::Direct).with_resource("medical"),
            Some("hazmat team staged at perimeter, awaiting entry clearance"),
        ),
    );

    sim.drain();
    println!("--- Consensus (zones must be isolated — clear-zone severity 1 is honest) ---");
    sim.run_consensus();

    println!("\n=== Multi-Zone Analysis ===");
    let mut zone_summary: HashMap<String, (u32, u32)> = HashMap::new(); // zone -> (total nodes, rescue count)
    for node in sim.nodes.values() {
        let entry = zone_summary.entry(node.zone.clone()).or_insert((0, 0));
        entry.0 += 1;
        entry.1 += node
            .persistent_messages
            .values()
            .filter(|m| m.message_type == "rescue")
            .count() as u32;
    }
    for (zone, (nodes, rescues)) in &zone_summary {
        println!(
            "  Zone {:20} | {} nodes | {} rescue msgs",
            zone, nodes, rescues
        );
    }
}
