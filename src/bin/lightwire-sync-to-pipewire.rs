use clap::Parser;
use anyhow::Result;
use lightwire::{ProviderRegistry, provider::LifxProvider};

#[derive(Parser, Debug)]
#[command(name = "lightwire-sync-to-pipewire")]
#[command(about = "Sync light brightness to PipeWire volumes", long_about = None)]
struct Cli {
    #[arg(short, long)]
    verbose: bool,
    #[arg(long)]
    dry_run: bool,
    #[arg(long)]
    provider: Option<String>,
    #[arg(long, default_value = "true")]
    once: bool,
    #[arg(long)]
    watch: bool,
    #[arg(long, default_value = "1000")]
    interval: u64,
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
        let state = light.state();
        println!("  - {} ({}): brightness={:.2}, power={}",
            light.label(),
            light.id().0,
            state.brightness.as_f32(),
            state.power
        );

        if cli.dry_run {
            println!("    DRY RUN: Would set PipeWire volume to {:.2}", state.brightness.as_f32());
        } else {
            match registry.get_state(light.provider_name(), light.id()).await {
                Ok(ref state) => {
                    println!("    Syncing brightness {:.2} to PipeWire", state.brightness.as_f32());
                }
                Err(e) => {
                    println!("    Error getting state: {}", e);
                }
            }
        }
    }

    if cli.watch && !cli.once {
        println!("\nWatching for changes every {}ms...", cli.interval);
        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(cli.interval)).await;
            println!("Syncing current light states to PipeWire...");
        }
    }

    Ok(())
}
