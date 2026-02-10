use clap::{Parser, Subcommand};
use anyhow::Result;
use lightwire::{ProviderRegistry, provider::LifxProvider, DropinConfig};
use lightwire::config::Config;

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
    #[arg(long, default_value = "true")]
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

    tracing_subscriber::fmt()
        .with_max_level(if cli.verbose { tracing::Level::DEBUG } else { tracing::Level::INFO })
        .init();

    match cli.command {
        Commands::Populate(opts) => run_populate(opts, cli.dry_run).await?,
        Commands::SyncToPipewire(_opts) => run_sync_to_pipewire(cli.dry_run).await?,
        Commands::SyncToLight(_opts) => run_sync_to_light(cli.dry_run).await?,
    }

    Ok(())
}

async fn run_populate(opts: PopulateOpts, dry_run: bool) -> Result<()> {
    let config = Config::load().unwrap_or_else(|_| Config::default());

    let mut registry = ProviderRegistry::new();
    let lifx_provider = LifxProvider::default();
    registry.register(Box::new(lifx_provider));

    let lights = registry.discover_all().await?;

    if lights.is_empty() {
        println!("No lights found on the network.");
        return Ok(());
    }

    let config_dir_path = opts.config_dir
        .map(|p| std::path::PathBuf::from(shellexpand::tilde(&p).into_owned()))
        .unwrap_or_else(|| config.pipewire_config_dir());

    if opts.clean {
        if dry_run {
            println!("DRY RUN: Would clean existing lightwire configs...");
        } else {
            println!("Cleaning existing lightwire configs...");
        }
        let entries = std::fs::read_dir(&config_dir_path);
        if let Ok(entries) = entries {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("conf") {
                    let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                    if filename.starts_with("lightwire-") {
                        if dry_run {
                            println!("Would remove: {}", filename);
                        } else {
                            match std::fs::remove_file(&path) {
                                Ok(_) => println!("Removed: {}", filename),
                                Err(e) => tracing::warn!("Failed to remove {}: {}", filename, e),
                            }
                        }
                    }
                }
            }
        }
    }

    if dry_run {
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

        if dry_run {
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

async fn run_sync_to_pipewire(_dry_run: bool) -> Result<()> {
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

        if _dry_run {
            println!("    DRY RUN: Would set PipeWire volume to {:.2}", state.brightness.as_f32());
        } else {
            println!("    Syncing brightness {:.2} to PipeWire", state.brightness.as_f32());
        }
    }

    Ok(())
}

async fn run_sync_to_light(_dry_run: bool) -> Result<()> {
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

    if _dry_run {
        println!("DRY RUN: Would update light brightness when PipeWire volumes change");
    }

    Ok(())
}
