use clap::Parser;
use crci::integration::{MessageKind, PipelineMessage};
use rand::Rng;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::time::sleep;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long)]
    target: String,

    #[arg(long)]
    identity: Option<String>,

    #[arg(long)]
    mode: String,

    #[arg(long, default_value_t = 500)]
    interval: u64,

    #[arg(long, default_value_t = 30)]
    duration: u64,
}

async fn send_msg(target: &str, msg: &PipelineMessage) -> std::io::Result<()> {
    let mut stream = TcpStream::connect(target).await?;
    let payload = serde_json::to_vec(msg)?;
    stream.write_u32(payload.len() as u32).await?;
    stream.write_all(&payload).await?;
    Ok(())
}

fn generate_id() -> String {
    let mut rng = rand::thread_rng();
    format!("byz-{:04x}", rng.gen::<u16>())
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let start = std::time::Instant::now();
    let mut injections = 0;
    let mut failures = 0;

    let my_node_id = if let Some(id_path) = args.identity {
        id_path
    } else {
        generate_id()
    };

    println!("[byzantine-agent] Starting in mode: {}", args.mode);

    let end_time = start + Duration::from_secs(args.duration);
    let mut seq = 0;

    while std::time::Instant::now() < end_time {
        seq += 1;
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();

        match args.mode.as_str() {
            "replay" => {
                let msg = PipelineMessage {
                    id: "replayed-msg-1".to_string(),
                    origin_node: my_node_id.clone(),
                    zone: "zone-0".to_string(),
                    severity: 3,
                    kind: MessageKind::Normal,
                    payload_bytes: 50,
                    reputation: 1.0,
                    round: 0,
                    seq: 1, // Same seq every time to trigger replay
                };
                match send_msg(&args.target, &msg).await {
                    Ok(_) => {
                        println!("[byzantine-agent][replay] injected at {} seq={}", ts, seq);
                        injections += 1;
                    }
                    Err(_) => {
                        failures += 1;
                    }
                }
                sleep(Duration::from_millis(args.interval)).await;
            }
            "false_allclear" => {
                let msg = PipelineMessage {
                    id: format!("false-allclear-{}", seq),
                    origin_node: my_node_id.clone(),
                    zone: "zone-0".to_string(),
                    severity: 5,
                    kind: MessageKind::Normal,
                    payload_bytes: 50,
                    reputation: 1.0,
                    round: 0,
                    seq,
                };
                match send_msg(&args.target, &msg).await {
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
                sleep(Duration::from_millis(args.interval)).await;
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
                    if send_msg(&args.target, &msg).await.is_ok() {
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
                sleep(Duration::from_millis(args.interval)).await;
            }
            "flood" => {
                let msg = PipelineMessage {
                    id: format!("flood-{}", seq),
                    origin_node: my_node_id.clone(),
                    zone: "zone-0".to_string(),
                    severity: 3,
                    kind: MessageKind::Normal,
                    payload_bytes: 50,
                    reputation: 1.0,
                    round: 0,
                    seq,
                };
                if send_msg(&args.target, &msg).await.is_ok() {
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
                        origin_node: my_node_id.clone(),
                        zone: "zone-0".to_string(),
                        severity: 3,
                        kind: MessageKind::Normal,
                        payload_bytes: 50,
                        reputation: 1.0,
                        round: 0,
                        seq: seq * 10 + i,
                    };
                    if send_msg(&args.target, &msg).await.is_ok() {
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
                sleep(Duration::from_millis(args.interval)).await;
            }
            _ => {
                eprintln!("Unknown mode: {}", args.mode);
                break;
            }
        }
    }

    println!(
        "[byzantine-agent] Done. Total injections attempted: {}, Total rejected/failed: {}",
        injections + failures,
        failures
    );
}
