use clap::Parser;
use rand::Rng;

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

fn generate_id() -> String {
    let mut rng = rand::thread_rng();
    format!("byz-{:04x}", rng.gen::<u16>())
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let my_node_id = if let Some(id_path) = args.identity {
        id_path
    } else {
        generate_id()
    };

    match crci_core::byzantine_behavior::run_byzantine_behavior(
        &args.target,
        &my_node_id,
        &args.mode,
        args.interval,
        args.duration,
    )
    .await
    {
        Ok((injections, failures)) => {
            println!(
                "[byzantine-agent] Done. Total injections attempted: {}, Total rejected/failed: {}",
                injections + failures,
                failures
            );
        }
        Err(e) => {
            eprintln!("Error running byzantine behavior: {}", e);
            std::process::exit(1);
        }
    }
}
