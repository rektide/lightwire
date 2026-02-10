use clap::Parser;
use anyhow::Result;

#[derive(Parser, Debug)]
#[command(name = "lightwire-populate")]
struct Cli {
    #[arg(short, long)]
    verbose: bool,
    #[arg(long)]
    dry_run: bool,
    #[arg(long)]
    provider: Option<String>,
    #[arg(long)]
    config_dir: Option<String>,
    #[arg(long)]
    clean: bool,
    #[arg(long, default_value = "true")]
    set_brightness: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let subscriber = tracing_subscriber::fmt()
        .with_max_level(if cli.verbose { tracing::Level::DEBUG } else { tracing::Level::INFO })
        .init();

    println!("Populate: dry_run={}", cli.dry_run);
    Ok(())
}
