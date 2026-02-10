use clap::Parser;
use anyhow::Result;

#[derive(Parser, Debug)]
#[command(name = "lightwire-sync-to-pipewire")]
struct Cli {
    #[arg(short, long)]
    verbose: bool,
    #[arg(long)]
    dry_run: bool,
    #[arg(long)]
    provider: Option<String>,
    #[arg(long)]
    once: bool,
    #[arg(long)]
    watch: bool,
    #[arg(long, default_value = "1000")]
    interval: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let subscriber = tracing_subscriber::fmt()
        .with_max_level(if cli.verbose { tracing::Level::DEBUG } else { tracing::Level::INFO })
        .init();

    println!("Sync to PipeWire: dry_run={}", cli.dry_run);
    Ok(())
}
