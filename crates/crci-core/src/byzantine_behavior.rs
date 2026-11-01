use crate::identity::Identity;
use crate::message::{Message, MessageType, Signal, Visibility};
use crate::runtime::WireMessage;
use std::time::Duration;
use tokio::time::sleep;

/// Send one message to `target` over an authenticated connection, as `sender` with `key`.
async fn send_msg(
    target: &str,
    sender: &str,
    key: &Identity,
    msg: &WireMessage,
) -> Result<(), crate::transport::TransportError> {
    crate::transport::tcp::send_authenticated(
        target,
        sender,
        &key.signing_key,
        std::slice::from_ref(msg),
    )
    .await
}

/// A message from `origin`, signed with `key`.
fn signed(
    key: &Identity,
    id: &str,
    origin: &str,
    severity: u8,
    message_type: MessageType,
    seq: u64,
) -> WireMessage {
    let mut msg = Message::new(
        id,
        origin,
        Signal::new(severity, false, false, true, 3, Visibility::Direct),
        None,
    );
    msg.message_type = message_type;
    msg.seq = seq;
    WireMessage::from_message(&msg, key)
}

pub async fn run_byzantine_behavior(
    target: &str,
    my_node_id: &str,
    mode: &str,
    interval_ms: u64,
    duration_secs: u64,
) -> Result<(usize, usize), Box<dyn std::error::Error + Send + Sync>> {
    let start = std::time::Instant::now();
    let mut injections = 0;
    let mut failures = 0;

    if !matches!(
        mode,
        "replay"
            | "false_allclear"
            | "sybil"
            | "flood"
            | "partition_heal"
            | "bad_signature"
            | "unsigned"
            | "unsigned_mce"
    ) {
        return Err(format!("Unknown mode: {}", mode).into());
    }

    println!("[byzantine-agent] Starting in mode: {}", mode);

    let key = Identity::new(my_node_id);
    // Replay mode resends this exact signed message every time.
    let replayed = signed(
        &key,
        "replayed-msg-1",
        my_node_id,
        3,
        MessageType::Normal,
        1,
    );

    let end_time = start + Duration::from_secs(duration_secs);
    let mut seq = 0;

    while std::time::Instant::now() < end_time {
        seq += 1;
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();

        // Messages for this round, all sent over connections authenticated with `key`.
        let batch: Vec<WireMessage> = match mode {
            "replay" => vec![replayed.clone()],
            "false_allclear" => vec![signed(
                &key,
                &format!("false-allclear-{}", seq),
                my_node_id,
                5,
                MessageType::Normal,
                seq,
            )],
            "sybil" => (0..10)
                .map(|i| {
                    let origin = format!("sybil-node-{}-{}", seq, i);
                    signed(
                        &Identity::new(&origin),
                        &format!("sybil-{}-{}", seq, i),
                        &origin,
                        5,
                        MessageType::Panic,
                        seq,
                    )
                })
                .collect(),
            "flood" => vec![signed(
                &key,
                &format!("flood-{}", seq),
                my_node_id,
                3,
                MessageType::Normal,
                seq,
            )],
            "partition_heal" => (0..5)
                .map(|i| {
                    signed(
                        &key,
                        &format!("part-heal-{}-{}", seq, i),
                        my_node_id,
                        3,
                        MessageType::Normal,
                        seq * 10 + i,
                    )
                })
                .collect(),
            "bad_signature" => {
                let mut wire = signed(
                    &key,
                    &format!("bad-sig-{}", seq),
                    my_node_id,
                    3,
                    MessageType::Normal,
                    seq,
                );
                if let Some(b) = wire.signature_bytes.first_mut() {
                    *b ^= 0xFF;
                }
                vec![wire]
            }
            "unsigned" | "unsigned_mce" => {
                let message_type = if mode == "unsigned_mce" {
                    MessageType::MassCasualtyEvent
                } else {
                    MessageType::Normal
                };
                let mut wire = signed(
                    &key,
                    &format!("{}-{}", mode, seq),
                    my_node_id,
                    5,
                    message_type,
                    seq,
                );
                wire.signature_bytes.clear();
                vec![wire]
            }
            _ => Vec::new(),
        };

        let mut sent = 0;
        for msg in &batch {
            if send_msg(target, my_node_id, &key, msg).await.is_ok() {
                sent += 1;
            } else {
                failures += 1;
            }
        }
        injections += sent;

        // Flood mode sends without pausing or logging each message.
        if mode != "flood" {
            println!(
                "[byzantine-agent][{}] injected {} message(s) at {} seq={}",
                mode, sent, ts, seq
            );
            sleep(Duration::from_millis(interval_ms)).await;
        }
    }

    Ok((injections, failures))
}
