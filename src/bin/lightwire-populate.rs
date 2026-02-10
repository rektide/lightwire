use clap::Parser;
use anyhow::Result;
use lightwire::{ProviderRegistry, provider::LifxProvider, DropinConfig};
use lightwire::config::Config;

#[derive(Parser, Debug)]
#[command(name = "lightwire-populate")]
#[command(about = "Discover lights and create PipeWire configs", long_about = None)]
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

    tracing_subscriber::fmt()
        .with_max_level(if cli.verbose { tracing::Level::DEBUG } else { tracing::Level::INFO })
        .init();

    let config = Config::load().unwrap_or_else(|_| Config::default());

    let mut registry = ProviderRegistry::new();
    let lifx_provider = LifxProvider::default();
    registry.register(Box::new(lifx_provider));

    let lights = registry.discover_all().await?;

    if lights.is_empty() {
        println!("No lights found on the network.");
        return Ok(());
    }

    let config_dir_path = cli.config_dir
        .map(|p| std::path::PathBuf::from(shellexpand::tilde(&p).into_owned()))
        .unwrap_or_else(|| config.pipewire_config_dir());

    if cli.dry_run {
        println!("DRY RUN: Would write to: {}", config_dir_path.display());
    }

    for light in &lights {
        let dropin = DropinConfig::new(
            light.provider_name().to_string(),
            light.label().to_string(),
            light.id().clone(),
            "lightwire".to_string(),
        );

        println!("Found: {} ({})", light.label(), light.id().0);

        if cli.dry_run {
            println!("Would create: {}", dropin.filename());
            println!("--- Config ---");
            println!("{}", dropin.generate());
            println!("--- End Config ---");
        } else {
            std::fs::create_dir_all(&config_dir_path)?;
            dropin.write_to(&config_dir_path)?;
            println!("Created: {}", dropin.filename());
        }
    }

    println!("\n{} light(s) configured.", lights.len());
    println!("PipeWire config directory: {}", config_dir_path.display());
    println!("\nTo load new nodes, run: systemctl --user restart pipewire");

    Ok(())
}
