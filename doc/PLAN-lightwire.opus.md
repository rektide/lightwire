# Lightwire Design Document

> Control smart-bulb brightness as virtual PipeWire node's volume

---

## Problem Statement

Users want intuitive, system-wide control of smart lighting brightness. Current solutions require proprietary apps, cloud dependencies, or complex home automation setups. Lightwire solves this by leveraging the universal volume control metaphor present in all modern Linux desktop environments.

**Key Challenges:**
1. **Ecosystem Fragmentation** - Smart lights use different protocols (LIFX, Hue, WLED, etc.) with no common API
2. **Integration Complexity** - Existing solutions require cloud accounts, vendor apps, or complex middleware
3. **Desktop Context** - Users expect controls to appear in standard audio mixers, not separate applications
4. **Extensibility** - Must support multiple providers without architectural rewrites

**Solution Approach:**
Map each smart bulb to a virtual PipeWire audio sink. When the user adjusts the "volume" of that sink, the actual bulb brightness changes proportionally. This provides:
- Native integration with all desktop mixers (pavucontrol, GNOME Settings, etc.)
- Media key support for brightness adjustment
- Per-application brightness control (assign specific apps to specific "lights")
- Universal protocol support through a provider abstraction

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                          Lightwire Daemon                            │
│  ┌─────────────────┐     ┌─────────────────┐     ┌───────────────┐  │
│  │ Provider Registry│────▶│   LIFX Provider │────▶│  LIFX Bulbs   │  │
│  │  (Box<dyn>)     │     │  (Box<dyn>)     │     │  (LAN UDP)    │  │
│  └────────┬────────┘     └─────────────────┘     └───────────────┘  │
│           │                                                          │
│  ┌────────▼────────┐     ┌─────────────────┐     ┌───────────────┐  │
│  │  PipeWire       │────▶│  Drop-in Config │────▶│  PipeWire     │  │
│  │  Monitor        │     │  Manager        │     │  Server       │  │
│  └─────────────────┘     └─────────────────┘     └───────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
         │                                                        │
    Volume events                                           Virtual Nodes
    (0.0 - 1.0)                                             (Audio/Sink)
         │                                                        │
         └───────────────────────┬────────────────────────────────┘
                                 ▼
                    ┌────────────────────────┐
                    │  Desktop Mixer UI      │
                    │  (pavucontrol, etc.)   │
                    └────────────────────────┘
```

---

## Technology Selection

### Core Libraries

| Purpose | Library | Version | Rationale |
|---------|---------|---------|-----------|
| PipeWire Client | `pipewire-native` | 0.1 | Pure Rust, no FFI, full proxy system |
| LIFX Protocol | `lifx-core` | 0.4 | LAN-only, minimal deps, full protocol |
| Async Runtime | `tokio` | 1.x | Multi-threaded, UDP networking |
| Configuration | `figment2` | 0.4 | Multi-source config with profiles |
| CLI Framework | `clap` | 4.x | Derive macros, completions via `clap_complete` |
| Time Handling | `jiff` | 0.1 | Modern Rust datetime (not chrono) |
| Logging | `tracing` | 0.1 | Structured logging with async support |
| XDG Paths | `directories` | 5.x | Cross-platform config/cache directories |
| Trait Objects | `async-trait` | 0.1 | Async trait object safety |
| Error Types | `thiserror` | 1.x | Derive Error implementations |

### System Integration

- **PipeWire Drop-in Configs:** `~/.config/pipewire/pipewire.conf.d/`
- **Service Management:** systemd user services (`systemctl --user`)
- **Reload Signals:** `systemctl --user restart pipewire` (hot reload TBD)
- **Virtual Node Factory:** `support.null-audio-sink`

---

## Solution: Trait Objects with Dynamic Dispatch

We use trait objects (`Box<dyn Provider>`) for runtime polymorphism, enabling multiple providers to coexist in a single daemon instance. This trades minor heap allocation overhead for maximum flexibility and extensibility.

### Core Types

```rust
/// Unique identifier for a light within its provider
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LightId(pub String);

/// Normalized brightness value (0.0..=1.0)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Brightness(f32);

impl Brightness {
    pub fn new(value: f32) -> Self {
        Self(value.clamp(0.0, 1.0))
    }
    pub fn as_f32(&self) -> f32 { self.0 }
    pub fn as_u16(&self) -> u16 { (self.0 * 65535.0) as u16 }
    pub fn as_percent(&self) -> u8 { (self.0 * 100.0) as u8 }
}

/// Common light state snapshot
#[derive(Clone, Debug)]
pub struct LightState {
    pub id: LightId,
    pub label: String,
    pub brightness: Brightness,
    pub power: bool,
}

/// Provider-specific error type
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("Network error: {0}")]
    Network(#[from] std::io::Error),
    #[error("Protocol error: {0}")]
    Protocol(String),
    #[error("Light not found: {0:?}")]
    NotFound(LightId),
    #[error("Timeout: {0}")]
    Timeout(String),
}
```

### Light Trait

```rust
/// Shared interface for all light types
/// 
/// Provides both borrowed (`state()`) and owned (`to_state()`) access patterns.
/// Prefer `state()` when possible to avoid cloning.
pub trait Light: Send + Sync + std::fmt::Debug {
    /// Unique identifier
    fn id(&self) -> &LightId;

    /// User-friendly label
    fn label(&self) -> &str;

    /// Provider name for namespacing
    fn provider_name(&self) -> &str;

    /// Current state as reference (zero-copy access)
    fn state(&self) -> &LightState;

    /// Current state as owned value (for concurrent contexts)
    fn to_state(&self) -> LightState {
        self.state().clone()
    }

    /// Optional: provider-specific metadata access
    fn metadata(&self) -> Option<&std::collections::HashMap<String, String>> {
        None
    }
}
```

### Provider Trait

```rust
use async_trait::async_trait;

/// Provider abstraction for smart-lighting ecosystems
#[async_trait]
pub trait Provider: Send + Sync + std::fmt::Debug {
    /// Provider identifier (e.g., "lifx", "hue", "wled")
    fn name(&self) -> &'static str;

    /// Discover all lights on the network
    async fn discover(&self) -> Result<Vec<Box<dyn Light>>, ProviderError>;

    /// Fetch current state of a specific light
    async fn get_state(&self, id: &LightId) -> Result<LightState, ProviderError>;

    /// Set brightness (and optionally power) for a light
    async fn set_brightness(&self, id: &LightId, brightness: Brightness) -> Result<(), ProviderError>;

    /// Optional: health check for provider connection
    async fn health_check(&self) -> Result<(), ProviderError> {
        Ok(())
    }
}
```

### Provider Registry

```rust
use std::collections::HashMap;

/// Central registry managing all providers
pub struct ProviderRegistry {
    providers: HashMap<String, Box<dyn Provider>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self { providers: HashMap::new() }
    }

    pub fn register(&mut self, provider: Box<dyn Provider>) {
        let name = provider.name().to_string();
        if self.providers.contains_key(&name) {
            tracing::warn!("Provider '{}' already registered, replacing", name);
        }
        self.providers.insert(name, provider);
    }

    pub fn get(&self, name: &str) -> Option<&dyn Provider> {
        self.providers.get(name).map(|p| p.as_ref())
    }

    /// Discover lights from all registered providers
    pub async fn discover_all(&self) -> Result<Vec<Box<dyn Light>>, ProviderError> {
        let mut all_lights = Vec::new();
        for (name, provider) in &self.providers {
            tracing::info!("Discovering lights from provider: {}", name);
            match provider.discover().await {
                Ok(lights) => {
                    tracing::info!("Found {} lights from {}", lights.len(), name);
                    all_lights.extend(lights);
                }
                Err(e) => {
                    tracing::error!("Failed to discover from {}: {}", name, e);
                }
            }
        }
        Ok(all_lights)
    }

    pub fn provider_names(&self) -> Vec<&str> {
        self.providers.keys().map(|s| s.as_str()).collect()
    }
}
```

---

## Volume Curves

Brightness perception is non-linear. The `curves/` module provides configurable mapping functions.

### Curve Trait

```rust
/// Volume-to-brightness mapping function
pub trait Curve: Send + Sync {
    /// Map PipeWire volume (0.0-1.0) to light brightness (0.0-1.0)
    fn apply(&self, volume: f32) -> f32;
    
    /// Inverse mapping for sync-to-pipewire
    fn inverse(&self, brightness: f32) -> f32;
    
    /// Curve identifier for config
    fn name(&self) -> &'static str;
}
```

### Built-in Curves

```rust
// curves/linear.rs
pub struct LinearCurve;

impl Curve for LinearCurve {
    fn apply(&self, volume: f32) -> f32 { volume }
    fn inverse(&self, brightness: f32) -> f32 { brightness }
    fn name(&self) -> &'static str { "linear" }
}

// curves/logarithmic.rs
pub struct LogarithmicCurve {
    pub base: f32,  // default: 10.0
}

impl Curve for LogarithmicCurve {
    fn apply(&self, volume: f32) -> f32 {
        if volume <= 0.0 { return 0.0; }
        (volume.powf(1.0 / self.base.log10())).clamp(0.0, 1.0)
    }
    fn inverse(&self, brightness: f32) -> f32 {
        brightness.powf(self.base.log10()).clamp(0.0, 1.0)
    }
    fn name(&self) -> &'static str { "logarithmic" }
}

// curves/gamma.rs
pub struct GammaCurve {
    pub gamma: f32,  // default: 2.2 (sRGB-like)
}

impl Curve for GammaCurve {
    fn apply(&self, volume: f32) -> f32 {
        volume.powf(self.gamma).clamp(0.0, 1.0)
    }
    fn inverse(&self, brightness: f32) -> f32 {
        brightness.powf(1.0 / self.gamma).clamp(0.0, 1.0)
    }
    fn name(&self) -> &'static str { "gamma" }
}

// curves/perceptual.rs — attempt to match human perception
pub struct PerceptualCurve;

impl Curve for PerceptualCurve {
    fn apply(&self, volume: f32) -> f32 {
        // CIE 1931 lightness approximation
        if volume <= 0.08 {
            volume / 9.033
        } else {
            ((volume + 0.16) / 1.16).powf(3.0)
        }.clamp(0.0, 1.0)
    }
    fn inverse(&self, brightness: f32) -> f32 {
        if brightness <= 0.008856 {
            brightness * 9.033
        } else {
            1.16 * brightness.powf(1.0/3.0) - 0.16
        }.clamp(0.0, 1.0)
    }
    fn name(&self) -> &'static str { "perceptual" }
}
```

### Curve Configuration

```toml
[curves]
default = "perceptual"

[curves.custom]
type = "gamma"
gamma = 2.4

# Per-light curve override
[lights."Desk Lamp"]
curve = "linear"
```

---

## Mute Handling

When a PipeWire node is muted, the default behavior sets brightness to 0 (lights off).

Future enhancement: mute applies a color filter instead (see ticket `mute-filter`).

```rust
#[derive(Clone, Debug, Default)]
pub enum MuteAction {
    #[default]
    Off,           // Set brightness to 0
    Ignore,        // Keep current brightness
    Filter(ColorFilter),  // Future: apply color tint
}

#[derive(Clone, Debug)]
pub struct ColorFilter {
    pub hue_shift: i16,      // -180 to 180
    pub saturation: f32,     // 0.0 to 1.0
    pub name: String,        // "sepia", "night", etc.
}
```

---

## CLI Structure

Lightwire provides both a unified binary with subcommands AND standalone binaries for each command. All commands support `--dry-run`.

### Unified Binary

```
lightwire <COMMAND> [OPTIONS]

Commands:
  populate         Discover lights, create PipeWire configs
  sync-to-pipewire Read light brightness, set PipeWire volumes
  sync-to-light    Watch PipeWire volumes, update light brightness

Global Options:
  --dry-run        Show what would happen without making changes
  --config <PATH>  Config file path
  --provider <NAME> Provider to use (default: all configured)
  -v, --verbose    Increase logging verbosity
```

### Standalone Binaries

Each command is also available as a standalone binary:
- `lightwire-populate`
- `lightwire-sync-to-pipewire`
- `lightwire-sync-to-light`

### Command: `populate`

```
lightwire populate [OPTIONS]

Discovers lights on the network and creates PipeWire drop-in configs.

Options:
  --provider <NAME>     Light provider (default: all)
  --config-dir <PATH>   PipeWire config directory
  --dry-run             Show what would be created without writing
  --clean               Remove configs for lights no longer found
  --set-brightness      After creating configs, sync current brightness
                        to PipeWire (default: true)
  --no-set-brightness   Skip brightness sync after populate
```

### Command: `sync-to-pipewire`

```
lightwire sync-to-pipewire [OPTIONS]

Reads current brightness from lights and sets corresponding PipeWire node volumes.

Options:
  --provider <NAME>     Light provider (default: all)
  --dry-run             Show what would be set without changing
  --once                Sync once and exit (default)
  --watch               Continuously poll lights for changes
  --interval <MS>       Polling interval when watching (default: 1000)
```

### Command: `sync-to-light`

```
lightwire sync-to-light [OPTIONS]

Watches PipeWire node volumes and updates light brightness accordingly.

Options:
  --provider <NAME>     Light provider (default: all)
  --dry-run             Show what would be sent without changing lights
  --once                Sync once and exit
  --daemon              Run continuously (default)
```

### Example Workflow

```bash
# Discover lights and create configs (also syncs current brightness)
$ lightwire populate --provider lifx
Found 3 LIFX bulbs:
  - Bedroom (d073d5xxxxxx) — brightness: 75%
  - Living Room (d073d5yyyyyy) — brightness: 50%
  - Desk Lamp (d073d5zzzzzz) — brightness: 100%
Created: ~/.config/pipewire/pipewire.conf.d/lightwire-lifx-bedroom.conf
Created: ~/.config/pipewire/pipewire.conf.d/lightwire-lifx-living-room.conf
Created: ~/.config/pipewire/pipewire.conf.d/lightwire-lifx-desk-lamp.conf
Set PipeWire volumes from current brightness.

# Restart PipeWire to load new nodes
$ systemctl --user restart pipewire

# Run the daemon (can also use standalone binary)
$ lightwire sync-to-light
Watching: lightwire.lifx.bedroom, lightwire.lifx.living-room, lightwire.lifx.desk-lamp

# Dry run to see what would happen
$ lightwire sync-to-light --dry-run --once
Would set 'Bedroom' brightness to 0.75 (volume: 0.75)
Would set 'Living Room' brightness to 0.50 (volume: 0.50)
Would set 'Desk Lamp' brightness to 1.00 (volume: 1.00)
```

---

## Virtual Node Creation via Drop-in Configs

Each light gets a drop-in config file in `~/.config/pipewire/pipewire.conf.d/`.

### File Naming Convention

```
~/.config/pipewire/pipewire.conf.d/
├── lightwire-lifx-bedroom.conf
├── lightwire-lifx-living-room.conf
└── lightwire-lifx-desk-lamp.conf
```

- Prefix: `lightwire-<provider>-`
- Suffix: sanitized label (lowercase, hyphens for spaces/special chars)
- Extension: `.conf`

### Generated Config Template

```
# Generated by lightwire - do not edit manually
# Light: Bedroom (d073d5xxxxxx)
# Provider: lifx

context.objects = [
  {
    factory = adapter
    args = {
      factory.name = support.null-audio-sink
      node.name = "lightwire.lifx.bedroom"
      node.description = "LIFX: Bedroom"
      media.class = Audio/Sink
      object.linger = true
      audio.position = [ FL FR ]
      monitor.channel-volumes = true
    }
  }
]
```

---

## Module Structure

```
lightwire/
├── Cargo.toml
├── src/
│   ├── lib.rs                    # Core library exports
│   ├── bin/
│   │   ├── lightwire.rs          # Unified CLI (subcommands)
│   │   ├── lightwire-populate.rs
│   │   ├── lightwire-sync-to-pipewire.rs
│   │   └── lightwire-sync-to-light.rs
│   ├── types.rs                  # LightId, Brightness, LightState
│   ├── provider/
│   │   ├── mod.rs                # Provider + Light traits, Registry
│   │   ├── error.rs              # ProviderError
│   │   └── lifx.rs               # LIFX implementation
│   ├── curves/
│   │   ├── mod.rs                # Curve trait + registry
│   │   ├── linear.rs
│   │   ├── logarithmic.rs
│   │   ├── gamma.rs
│   │   └── perceptual.rs
│   ├── pipewire/
│   │   ├── mod.rs
│   │   ├── dropin.rs             # Config file generation
│   │   ├── volume.rs             # Volume get/set
│   │   └── monitor.rs            # Watch for volume changes
│   └── config.rs                 # Configuration loading
└── tests/
    ├── integration/
    └── fixtures/
```

---

## Configuration

### User Config: `~/.config/lightwire/config.toml`

```toml
[pipewire]
config_dir = "~/.config/pipewire/pipewire.conf.d"
node_prefix = "lightwire"

[curves]
default = "perceptual"

[lifx]
discovery_timeout_ms = 5000
broadcast_address = "255.255.255.255"
port = 56700

# Per-light overrides
[lights."Bedroom"]
min_brightness = 0.1    # Never go fully dark
max_brightness = 1.0
curve = "linear"
mute_action = "off"     # "off" | "ignore"

[lights."Desk Lamp"]
enabled = false         # Skip this light
```

---

## Implementation Phases

### Phase 1: Core Foundation
- [ ] Define core types (`LightId`, `Brightness`, `LightState`, `ProviderError`)
- [ ] Implement `Provider` and `Light` traits
- [ ] Implement `ProviderRegistry`
- [ ] Unit tests for registry and types

### Phase 2: LIFX Provider
- [ ] Implement `LifxProvider` with UDP discovery
- [ ] Implement brightness get/set
- [ ] Integration tests with mock UDP

### Phase 3: Volume Curves
- [ ] Implement `Curve` trait
- [ ] Implement built-in curves (linear, logarithmic, gamma, perceptual)
- [ ] Curve configuration loading

### Phase 4: PipeWire Integration
- [ ] Implement drop-in config generation
- [ ] Implement `lightwire-populate` command
- [ ] Implement volume monitoring via `pipewire-native`
- [ ] Implement `lightwire-sync-to-pipewire` command
- [ ] Implement `lightwire-sync-to-light` command

### Phase 5: Unified CLI
- [ ] Implement `lightwire` wrapper binary with subcommands
- [ ] Ensure standalone binaries work identically
- [ ] Add `--dry-run` to all commands
- [ ] Shell completions via `clap_complete`

### Phase 6: Polish
- [ ] systemd service files
- [ ] Documentation
- [ ] Man pages
- [ ] Error recovery and reconnection logic

### Phase 7: Multi-Provider Support
- [ ] Add second provider (Hue or WLED)
- [ ] Provider-specific configuration schemas
- [ ] Mixed-provider testing

---

## Dependencies

```toml
[dependencies]
pipewire-native = "0.1"
lifx-core = "0.4"
tokio = { version = "1", features = ["net", "rt-multi-thread", "fs", "macros"] }
figment2 = "0.4"
clap = { version = "4", features = ["derive", "env"] }
clap_complete = "4"
jiff = "0.1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
directories = "5"
async-trait = "0.1"
serde = { version = "1", features = ["derive"] }
toml = "0.8"
thiserror = "1"

[dev-dependencies]
tokio-test = "0.4"
```

---

## Success Criteria

1. **Configuration**: `lightwire populate` creates correct drop-in configs for discovered bulbs
2. **Visibility**: Virtual nodes appear in `pavucontrol`, GNOME Settings, etc. after PipeWire reload
3. **Responsiveness**: Volume changes translate to brightness changes within 100ms
4. **Brightness Sync**: `populate --set-brightness` correctly initializes PipeWire volumes
5. **Multi-Provider**: Single daemon instance supports multiple provider types simultaneously
6. **Offline Operation**: Works entirely on LAN (no cloud/internet required)
7. **Clean Removal**: Removing a config causes the node to disappear after PipeWire reload
8. **Extensibility**: New providers and curves can be added without modifying existing code
9. **Dry Run**: All commands accurately report what they would do without side effects

---

## Open Questions

1. **Hot Reload** - Can PipeWire reload configs without full restart?
   - Investigate `pw-cli load-module` or SIGHUP
   - May need to document "restart required" for now

2. **Node Matching** - How to reliably match node.name to bulb after PipeWire restart?
   - Use deterministic naming: `lightwire.<provider>.<sanitized-label>`
   - Store mapping in state file if needed

3. **Group Control** - Support for controlling multiple lights as one node?
   - Implement as meta-provider that wraps multiple lights
   - Or use PipeWire node groups

---

## Appendix A: Alternative Provider Designs (Rejected)

### Proposal A: Trait with Associated Types

```rust
pub trait Provider {
    type Light: Light;
    fn name(&self) -> &'static str;
    async fn discover(&self) -> Result<Vec<Self::Light>, ProviderError>;
}
```

**Rejected because:** Cannot store `Vec<Box<dyn Provider>>` for multi-provider support. Requires compile-time knowledge of all providers and generic propagation throughout codebase.

### Proposal C: Enum-Based (Closed Set)

```rust
pub enum ProviderKind { Lifx(LifxProvider), Hue(HueProvider), ... }
```

**Rejected because:** Adding a provider requires modifying core enums. Not extensible by users. Every new provider adds variants to all match statements.

---

## Appendix B: Glossary

- **PipeWire** - Modern Linux audio/video server replacing PulseAudio and JACK
- **Drop-in Config** - Configuration snippet in a `.d/` directory, automatically loaded
- **Virtual Node** - Software audio device without physical hardware
- **Provider** - Implementation of Light/Provider traits for a specific ecosystem
- **LIFX** - Brand of WiFi smart bulbs using UDP-based LAN protocol
- **Brightness** - Normalized value 0.0-1.0 representing light output level
- **Volume** - Audio level 0.0-1.0, used as the control metaphor for brightness
- **Curve** - Function mapping volume to brightness (handles perceptual non-linearity)
