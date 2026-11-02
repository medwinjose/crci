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

    /// Experiments: originate this many signed messages from this node (needs --listen)
    #[arg(long, default_value_t = 0)]
    pub emit_count: u64,

    /// Experiments: milliseconds between originated messages
    #[arg(long, default_value_t = 1000)]
    pub emit_interval_ms: u64,

    /// Experiments: milliseconds after process start before the first originated message
    #[arg(long, default_value_t = 0)]
    pub emit_delay_ms: u64,

    /// Fault injection for experiments: this many milliseconds after process start, send every
    /// outgoing gossip message --replay-copies times, as a replaying relay would
    #[arg(long)]
    pub replay_after_ms: Option<u64>,

    /// Experiments: count --emit-delay-ms and --replay-after-ms from this Unix time (ms)
    /// instead of process start, so nodes started one after another share one schedule
    #[arg(long)]
    pub experiment_epoch_unix_ms: Option<u64>,

    /// Fault injection for experiments: copies of each message sent once the fault is active
    #[arg(long, default_value_t = 2)]
    pub replay_copies: usize,
}
