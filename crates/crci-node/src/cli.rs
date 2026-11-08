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

    /// Human-readable name for log lines. Not authenticated: the node ID is derived from the
    /// node's public key.
    #[arg(long)]
    pub label: Option<String>,

    /// Optional peer address to dial immediately after startup.
    #[arg(long)]
    pub dial: Option<String>,

    /// Enable verbose logging
    #[arg(short, long)]
    pub verbose: bool,

    /// Keep the node key in this encrypted file (AES-256-GCM, Argon2id), created on first use,
    /// so the node keeps its ID across restarts. Needs --key-passphrase-file. Without it the
    /// node generates a new key, and so a new ID, at every start.
    #[arg(long, requires = "key_passphrase_file")]
    pub key_file: Option<std::path::PathBuf>,

    /// File whose first line is the passphrase for --key-file.
    #[arg(long, requires = "key_file")]
    pub key_passphrase_file: Option<std::path::PathBuf>,

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

    /// Fault injection for experiments: once the fault is active, corrupt the signature of every
    /// outgoing gossip message, as a relay forwarding forged data would
    #[arg(long)]
    pub corrupt_signatures: bool,

    /// With --corrupt-signatures: corrupt for this many milliseconds from the fault on, then
    /// sign correctly again (to test readmission); default: until the process ends
    #[arg(long)]
    pub corrupt_for_ms: Option<u64>,

    /// Experiments: which defences the node applies (full, static-bucket, vanilla); the
    /// baselines of the evaluation. Default: full, the protocol as shipped
    #[arg(long, default_value = "full")]
    pub defence: String,

    /// Experiments: strikes that ban a peer (default 3)
    #[arg(long)]
    pub strikes_to_ban: Option<u32>,

    /// Experiments: replay window W in sequence numbers (default 64; 0 = strict-monotonic;
    /// at most 1024)
    #[arg(long)]
    pub replay_window: Option<u64>,

    /// Experiments: messages per second added back to each neighbour's budget (default:
    /// the protocol's rate)
    #[arg(long)]
    pub sender_refill_per_sec: Option<f32>,
}
