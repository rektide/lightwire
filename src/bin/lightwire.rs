use clap::{Parser, Subcommand};
use anyhow::Result;

#[derive(Parser, Debug)]
#[command(name = "lightwire")]
#[command(about = "Control smart-bulb brightness as virtual PipeWire node's volume", long_about = None)]
struct Cli {
    #[arg(short, long)]
    verbose: bool,
    #[arg(long)]
    dry_run: bool,
    #[arg(long)]
    config: Option<String>,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Populate(PopulateOpts),
    SyncToPipewire(SyncToPipewireOpts),
    SyncToLight(SyncToLightOpts),
}

#[derive(clap::Args, Debug)]
struct PopulateOpts {
    #[arg(long)]
    provider: Option<String>,
    #[arg(long)]
    config_dir: Option<String>,
    #[arg(long)]
    clean: bool,
    #[arg(long)]
    set_brightness: bool,
}

#[derive(clap::Args, Debug)]
struct SyncToPipewireOpts {
    #[arg(long)]
    provider: Option<String>,
    #[arg(long)]
    once: bool,
    #[arg(long)]
    watch: bool,
    #[arg(long, default_value = "1000")]
    interval: u64,
}

#[derive(clap::Args, Debug)]
struct SyncToLightOpts {
    #[arg(long)]
    provider: Option<String>,
    #[arg(long)]
    once: bool,
    #[arg(long)]
    daemon: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let subscriber = tracing_subscriber::fmt()
        .with_max_level(if cli.verbose { tracing::Level::DEBUG } else { tracing::Level::INFO })
        .init();

    match cli.command {
        Commands::Populate(opts) => run_populate(opts, cli.dry_run).await?,
        Commands::SyncToPipewire(opts) => run_sync_to_pipewire(opts, cli.dry_run).await?,
        Commands::SyncToLight(opts) => run_sync_to_light(opts, cli.dry_run).await?,
    }

    Ok(())
}

async fn run_populate(opts: PopulateOpts, dry_run: bool) -> Result<()> {
    println!("Populate command - dry_run: {}", dry_run);
    Ok(())
}

async fn run_sync_to_pipewire(opts: SyncToPipewireOpts, dry_run: bool) -> Result<()> {
    println!("Sync to PipeWire command - dry_run: {}", dry_run);
    Ok(())
}

async fn run_sync_to_light(opts: SyncToLightOpts, dry_run: bool) -> Result<()> {
    println!("Sync to Light command - dry_run: {}", dry_run);
    Ok(())
}
