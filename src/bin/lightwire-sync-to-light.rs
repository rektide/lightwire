use clap::Parser;
use anyhow::Result;
use lightwire::{ProviderRegistry, provider::LifxProvider};

#[derive(Parser, Debug)]
#[command(name = "lightwire-sync-to-light")]
#[command(about = "Watch PipeWire volumes and update light brightness", long_about = None)]
struct Cli {
    #[arg(short, long)]
    verbose: bool,
    #[arg(long)]
    dry_run: bool,
    #[arg(long)]
    provider: Option<String>,
    #[arg(long)]
    once: bool,
    #[arg(long, default_value = "true")]
    daemon: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_max_level(if cli.verbose { tracing::Level::DEBUG } else { tracing::Level::INFO })
        .init();

    let mut registry = ProviderRegistry::new();
    let lifx_provider = LifxProvider::default();
    registry.register(Box::new(lifx_provider));

    let lights = registry.discover_all().await?;

    if lights.is_empty() {
        println!("No lights found on the network.");
        return Ok(());
    }

    println!("Found {} light(s):", lights.len());
    for light in &lights {
        println!("  - {} ({})", light.label(), light.id().0);
    }

    println!("\nWatching PipeWire for volume changes...");

    if cli.dry_run {
        println!("DRY RUN: Would update light brightness when PipeWire volumes change");
    }

    if !cli.daemon && !cli.once {
        println!("Running once and exiting...");
    }

    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        if cli.once {
            break;
        }
    }

    Ok(())
}
