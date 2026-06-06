use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    name = "crci",
    version,
    about = "Crisis Response Communication Infrastructure — decentralised mesh node"
)]
pub struct Cli {
    /// Node listening address
    #[arg(long, default_value = "0.0.0.0:9000")]
    pub listen: String,

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
}
