use crate::integration::{MessageKind, PipelineMessage};
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::time::sleep;

async fn send_msg(target: &str, msg: &PipelineMessage) -> std::io::Result<()> {
    let mut stream = TcpStream::connect(target).await?;
    let payload = serde_json::to_vec(msg)?;
    stream.write_u32(payload.len() as u32).await?;
    stream.write_all(&payload).await?;
    Ok(())
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

    println!("[byzantine-agent] Starting in mode: {}", mode);

    let end_time = start + Duration::from_secs(duration_secs);
    let mut seq = 0;

    while std::time::Instant::now() < end_time {
        seq += 1;
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();

        match mode {
            "replay" => {
                let msg = PipelineMessage {
                    id: "replayed-msg-1".to_string(),
                    origin_node: my_node_id.to_string(),
                    zone: "zone-0".to_string(),
                    severity: 3,
                    kind: MessageKind::Normal,
                    payload_bytes: 50,
                    reputation: 1.0,
                    round: 0,
                    seq: 1, // Same seq every time to trigger replay
                };
                match send_msg(target, &msg).await {
                    Ok(_) => {
                        println!("[byzantine-agent][replay] injected at {} seq={}", ts, seq);
                        injections += 1;
                    }
                    Err(_) => {
                        failures += 1;
                    }
                }
                sleep(Duration::from_millis(interval_ms)).await;
            }
            "false_allclear" => {
                let msg = PipelineMessage {
                    id: format!("false-allclear-{}", seq),
                    origin_node: my_node_id.to_string(),
                    zone: "zone-0".to_string(),
                    severity: 5,
                    kind: MessageKind::Normal,
                    payload_bytes: 50,
                    reputation: 1.0,
                    round: 0,
                    seq,
                };
                match send_msg(target, &msg).await {
                    Ok(_) => {
                        println!(
                            "[byzantine-agent][false_allclear] injected at {} seq={}",
                            ts, seq
                        );
                        injections += 1;
                    }
                    Err(_) => {
                        failures += 1;
                    }
                }
                sleep(Duration::from_millis(interval_ms)).await;
            }
            "sybil" => {
                let mut sybil_injections = 0;
                for i in 0..10 {
                    let msg = PipelineMessage {
                        id: format!("sybil-{}-{}", seq, i),
                        origin_node: format!("sybil-node-{}-{}", seq, i),
                        zone: "zone-0".to_string(),
                        severity: 5,
                        kind: MessageKind::Panic,
                        payload_bytes: 50,
                        reputation: 1.0,
                        round: 0,
                        seq,
                    };
                    if send_msg(target, &msg).await.is_ok() {
                        sybil_injections += 1;
                    } else {
                        failures += 1;
                    }
                }
                println!(
                    "[byzantine-agent][sybil] injected {} messages at {} seq={}",
                    sybil_injections, ts, seq
                );
                injections += sybil_injections;
                sleep(Duration::from_millis(interval_ms)).await;
            }
            "flood" => {
                let msg = PipelineMessage {
                    id: format!("flood-{}", seq),
                    origin_node: my_node_id.to_string(),
                    zone: "zone-0".to_string(),
                    severity: 3,
                    kind: MessageKind::Normal,
                    payload_bytes: 50,
                    reputation: 1.0,
                    round: 0,
                    seq,
                };
                if send_msg(target, &msg).await.is_ok() {
                    injections += 1;
                } else {
                    failures += 1;
                }
                // No sleep for flood
            }
            "partition_heal" => {
                let mut part_injections = 0;
                for i in 0..5 {
                    let msg = PipelineMessage {
                        id: format!("part-heal-{}-{}", seq, i),
                        origin_node: my_node_id.to_string(),
                        zone: "zone-0".to_string(),
                        severity: 3,
                        kind: MessageKind::Normal,
                        payload_bytes: 50,
                        reputation: 1.0,
                        round: 0,
                        seq: seq * 10 + i,
                    };
                    if send_msg(target, &msg).await.is_ok() {
                        part_injections += 1;
                    } else {
                        failures += 1;
                    }
                }
                println!(
                    "[byzantine-agent][partition_heal] injected {} messages at {} seq={}",
                    part_injections, ts, seq
                );
                injections += part_injections;
                sleep(Duration::from_millis(interval_ms)).await;
            }
            _ => {
                return Err(format!("Unknown mode: {}", mode).into());
            }
        }
    }

    Ok((injections, failures))
}
