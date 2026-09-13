use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    name = "crci",
    version,
    about = "Crisis Response Communication Infrastructure — decentralised mesh node"
)]
pub struct Cli {
    /// Start a mesh node on the given listen address.
    #[arg(long)]
    pub listen: Option<String>,

    /// Human-readable node identifier.
    #[arg(long)]
    pub node_id: Option<String>,

    /// Optional peer address to dial immediately after startup.
    #[arg(long)]
    pub dial: Option<String>,

    /// Path to config file (TOML)
    #[arg(long, default_value = "crci.toml")]
    pub config: String,

    /// Argon2id memory cost in KiB (lower on memory-constrained devices)
    #[arg(long, default_value_t = 65536)]
    pub argon2_memory: u32,

    /// Enable verbose logging
    #[arg(short, long)]
    pub verbose: bool,

    /// Node identity seed (hex, 32 bytes) — omit to generate fresh keypair
    #[arg(long)]
    pub identity_seed: Option<String>,

    /// Message body to originate after startup/dial
    #[arg(long)]
    pub send: Option<String>,

    /// Target peer socket address for --send
    #[arg(long)]
    pub to: Option<String>,

    /// Severity level for --send (normal, rescue, panic)
    #[arg(long, default_value = "normal")]
    pub severity: String,

    /// Byzantine mode to run (e.g. flood, sybil, false_allclear, replay, partition_heal)
    #[arg(long)]
    pub byzantine: Option<String>,

    /// Comma-separated target peer addresses to dial at startup
    #[arg(long)]
    pub peers: Option<String>,

    /// Interval for byzantine injection in milliseconds
    #[arg(long, default_value_t = 500)]
    pub interval: u64,

    /// Duration for byzantine injection in seconds
    #[arg(long, default_value_t = 30)]
    pub duration: u64,
}
