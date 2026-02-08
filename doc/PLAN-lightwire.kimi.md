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

## Available Components, Tools, and Libraries

### Core Libraries

#### PipeWire Client: `pipewire-native`

**Crate:** `pipewire-native` (pure Rust, no FFI)

**Capabilities:**
- Native Rust implementation of PipeWire protocol
- No C dependencies or bindgen complexity
- Full proxy system for Node/Registry interaction
- Event-driven architecture with `MainLoop`/`ThreadLoop`

**Key APIs:**
- `MainLoop` / `ThreadLoop` - Event loop management
- `Context` and `Core` - Server connection
- `Registry` - Object enumeration and node binding
- `Node` proxy - Subscribe to parameter changes (Props)
- `spa::param::ParamType::Props` - Volume parameter extraction

#### LIFX Protocol: `lifx-core`

**Crate:** `lifx-core` v0.4

**Capabilities:**
- Local/LAN protocol only (no cloud dependency)
- Minimal dependencies (`byteorder`, `thiserror`)
- Full protocol coverage including brightness control
- Message framing and checksum handling

**Limitations:** Requires custom UDP I/O implementation, but provides full control over discovery and command timing.

### Infrastructure Libraries

| Purpose | Library | Version | Rationale |
|---------|---------|---------|-----------|
| Async Runtime | `tokio` | 1.x | Multi-threaded runtime with UDP networking |
| Configuration | `figment2` | 0.4 | Multi-source config with profile support |
| CLI Framework | `clap` | 4.x | Derive-based argument parsing with completions |
| Time Handling | `jiff` | 0.1 | Modern Rust datetime library (preferred over chrono) |
| Logging | `tracing` | 0.1 | Structured logging with async support |
| XDG Paths | `directories` | 5.x | Cross-platform config/cache directories |
| Trait Objects | `async-trait` | 0.1 | Required for async trait object safety |

### System Integration

- **PipeWire Drop-in Configs:** `~/.config/pipewire/pipewire.conf.d/`
- **Service Management:** systemd user services (`systemctl --user`)
- **Reload Signals:** SIGHUP or `pw-cli` commands
- **Virtual Node Factory:** `support.null-audio-sink`

---

## Solution: Proposal B — Trait Objects with Dynamic Dispatch

### Design Rationale

After evaluating three provider abstraction approaches (see Appendix A), we selected **Proposal B: Trait Objects** for the following reasons:

1. **Multi-Provider Support** - Runtime polymorphism enables seamless support for multiple light ecosystems in a single daemon instance
2. **Extensibility** - Third-party providers can be compiled as plugins without core modification
3. **Registry Pattern** - Clean separation between provider registration and usage
4. **Acceptable Trade-offs** - Minor heap allocation overhead is justified by the flexibility gained

### Core Abstractions

```rust
/// Unique identifier for a light within a provider
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LightId(pub String);

/// Normalized brightness value (0.0 - 1.0)
#[derive(Clone, Copy, Debug)]
pub struct Brightness(f32);

impl Brightness {
    pub fn new(value: f32) -> Self {
        Self(value.clamp(0.0, 1.0))
    }
    pub fn as_f32(&self) -> f32 { self.0 }
    pub fn as_u16(&self) -> u16 { (self.0 * 65535.0) as u16 }
}

/// Current state of a light
#[derive(Clone, Debug)]
pub struct LightState {
    pub id: LightId,
    pub label: String,
    pub brightness: Brightness,
    pub power: bool,
}

/// Light as a trait object for dynamic dispatch
pub trait Light: Send + Sync + std::fmt::Debug {
    fn id(&self) -> &LightId;
    fn label(&self) -> &str;
    fn provider_name(&self) -> &str;
    fn to_state(&self) -> LightState;
}

/// Provider trait using async_trait for object safety
#[async_trait]
pub trait Provider: Send + Sync + std::fmt::Debug {
    fn name(&self) -> &'static str;
    
    /// Discover all lights on the network
    async fn discover(&self) -> Result<Vec<Box<dyn Light>>, ProviderError>;
    
    /// Get current state of a specific light
    async fn get_state(&self, id: &LightId) -> Result<LightState, ProviderError>;
    
    /// Set brightness (and optionally power) for a light
    async fn set_brightness(&self, id: &LightId, brightness: Brightness) -> Result<(), ProviderError>;
}
```

### Provider Registry

```rust
/// Central registry managing all providers
pub struct ProviderRegistry {
    providers: HashMap<String, Box<dyn Provider>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }
    
    /// Register a provider
    pub fn register<P: Provider + 'static>(&mut self, provider: P) {
        let name = provider.name().to_string();
        self.providers.insert(name, Box::new(provider));
    }
    
    /// Get provider by name
    pub fn get(&self, name: &str) -> Option<&dyn Provider> {
        self.providers.get(name).map(|p| p.as_ref())
    }
    
    /// Discover lights from all registered providers
    pub async fn discover_all(&self) -> Result<Vec<Box<dyn Light>>, ProviderError> {
        let mut lights = Vec::new();
        for (name, provider) in &self.providers {
            match provider.discover().await {
                Ok(mut found) => lights.append(&mut found),
                Err(e) => tracing::warn!("Provider '{}' discovery failed: {}", name, e),
            }
        }
        Ok(lights)
    }
    
    /// Get all provider names
    pub fn provider_names(&self) -> Vec<&str> {
        self.providers.keys().map(|s| s.as_str()).collect()
    }
}
```

### LIFX Implementation Example

```rust
/// LIFX-specific light representation
#[derive(Debug)]
pub struct LifxLight {
    pub id: LightId,
    pub label: String,
    pub addr: SocketAddr,
    pub target: u64,  // LIFX target address
    pub state: LightState,
    // LIFX-specific fields
    pub hue: u16,
    pub saturation: u16,
    pub kelvin: u16,
}

impl Light for LifxLight {
    fn id(&self) -> &LightId { &self.id }
    fn label(&self) -> &str { &self.label }
    fn provider_name(&self) -> &str { "lifx" }
    fn to_state(&self) -> LightState { self.state.clone() }
}

#[derive(Debug)]
pub struct LifxProvider {
    socket: UdpSocket,
    timeout: Duration,
    broadcast_addr: SocketAddr,
}

#[async_trait]
impl Provider for LifxProvider {
    fn name(&self) -> &'static str { "lifx" }
    
    async fn discover(&self) -> Result<Vec<Box<dyn Light>>, ProviderError> {
        // Send GetService broadcast, collect responses
        // Parse StateService messages, build LifxLight objects
        // Return as Box<dyn Light>
        todo!()
    }
    
    async fn get_state(&self, id: &LightId) -> Result<LightState, ProviderError> {
        // Send GetColor to specific bulb
        // Parse StateColor response
        todo!()
    }
    
    async fn set_brightness(&self, id: &LightId, brightness: Brightness) -> Result<(), ProviderError> {
        // Convert Brightness to LIFX level (0-65535)
        // Send SetColor message
        todo!()
    }
}
```

---

## Implementation Strategies

### Strategy 1: Monolithic Crate with Feature Flags

All providers compiled into a single crate, enabled via Cargo features.

```
lightwire/
├── Cargo.toml
├── src/
│   ├── lib.rs                      # Core library exports
│   ├── main.rs                     # Daemon entry point
│   ├── bin/
│   │   ├── lightwire-populate.rs   # CLI: discover lights, create configs
│   │   └── lightwire-cli.rs        # CLI: management commands
│   ├── core/
│   │   ├── mod.rs                  # Core types (LightId, Brightness, LightState)
│   │   ├── error.rs                # ProviderError, Result types
│   │   └── registry.rs             # ProviderRegistry
│   ├── provider/
│   │   ├── mod.rs                  # Provider trait, Light trait
│   │   ├── lifx.rs                 # LIFX provider implementation
│   │   ├── hue.rs                  # Hue provider (feature-gated)
│   │   └── wled.rs                 # WLED provider (feature-gated)
│   ├── pipewire/
│   │   ├── mod.rs
│   │   ├── dropin.rs               # Config file generation
│   │   ├── monitor.rs              # Volume change monitoring
│   │   └── volume.rs               # Volume get/set utilities
│   └── sync/
│       ├── mod.rs                  # Bidirectional sync logic
│       ├── pw_to_light.rs          # PipeWire → Light direction
│       └── light_to_pw.rs          # Light → PipeWire direction
└── tests/
    └── integration_tests.rs
```

**Cargo.toml:**
```toml
[features]
default = ["lifx"]
lifx = []
hue = []
wled = []

[[bin]]
name = "lightwire"
path = "src/main.rs"

[[bin]]
name = "lightwire-populate"
path = "src/bin/lightwire-populate.rs"

[[bin]]
name = "lightwire-cli"
path = "src/bin/lightwire-cli.rs"
```

**Pros:**
- Simple dependency management
- Easy cross-provider integration testing
- Single binary distribution

**Cons:**
- All providers must be compiled together
- Binary size grows with each provider
- Cannot add providers without recompiling

---

### Strategy 2: Workspace with Separate Provider Crates

Each provider is a separate crate in a workspace, loaded dynamically.

```
lightwire/
├── Cargo.toml                    # Workspace manifest
├── crates/
│   ├── lightwire-core/           # Core traits and types
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── types.rs          # LightId, Brightness, LightState
│   │       ├── error.rs          # ProviderError
│   │       └── registry.rs       # ProviderRegistry
│   ├── lightwire-pipewire/       # PipeWire integration
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── dropin.rs
│   │       └── monitor.rs
│   ├── lightwire-lifx/           # LIFX provider crate
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs
│   ├── lightwire-hue/            # Hue provider crate
│   │   └── ...
│   └── lightwire-wled/           # WLED provider crate
│       └── ...
├── lightwire-daemon/             # Main daemon executable
│   ├── Cargo.toml
│   └── src/
│       └── main.rs
└── lightwire-cli/                # CLI tools
    ├── Cargo.toml
    └── src/
        ├── populate.rs
        └── manage.rs
```

**Workspace Cargo.toml:**
```toml
[workspace]
members = [
    "crates/lightwire-core",
    "crates/lightwire-pipewire",
    "crates/lightwire-lifx",
    "crates/lightwire-hue",
    "crates/lightwire-wled",
    "lightwire-daemon",
    "lightwire-cli",
]
```

**Pros:**
- Providers can be developed independently
- Users only compile providers they need
- Clean dependency boundaries
- Easier to publish individual crates to crates.io

**Cons:**
- More complex build system
- Potential version mismatches between crates
- More crates to manage

---

### Strategy 3: Hybrid — Core + Plugin Providers

Core registry supports dynamic provider loading (via dlopen or WASM plugins).

```
lightwire/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── main.rs
│   ├── core/
│   │   ├── mod.rs
│   │   └── plugin.rs             # Plugin loading system
│   └── bin/
│       └── ...
└── providers/
    ├── builtin/
    │   ├── lifx/
    │   │   ├── Cargo.toml
    │   │   └── src/lib.rs
    │   └── mod.rs                # Re-export builtin providers
    └── external/                 # Directory for .so/.dll plugins
        └── .gitkeep
```

**Plugin Trait:**
```rust
/// Marker trait for provider plugins
pub trait ProviderPlugin: Send + Sync {
    fn create_provider(&self, config: &Config) -> Box<dyn Provider>;
}

/// Plugin registration function signature
pub type RegisterFn = unsafe extern "C" fn() -> Box<dyn ProviderPlugin>;
```

**Pros:**
- Runtime provider loading
- Binary distribution of providers
- Core can be minimal

**Cons:**
- Complex FFI boundary
- ABI stability concerns
- Overkill for initial implementation

---

## Virtual Node Creation

Lightwire manages drop-in configuration files in `~/.config/pipewire/pipewire.conf.d/`:

**File:** `lightwire-lifx-bedroom.conf`

```conf
# Generated by lightwire - do not edit manually
# Light: Bedroom (d073d5xxxxxx)
# Provider: lifx
# Generated: 2024-01-15T09:30:00Z

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

### File Naming Convention

```
~/.config/pipewire/pipewire.conf.d/
├── lightwire-lifx-bedroom.conf
├── lightwire-lifx-living-room.conf
├── lightwire-hue-desk-lamp.conf
└── lightwire-wled-strip.conf
```

Format: `lightwire-{provider}-{sanitized-label}.conf`

---

## CLI Tools

### 1. `lightwire-populate` — Discovery and Config Generation

```bash
lightwire-populate [OPTIONS]

Options:
  --providers <names>   Comma-separated providers (default: all available)
  --config-dir <path>   PipeWire config directory
  --dry-run             Show what would be created without writing
  --clean               Remove configs for lights no longer found

Examples:
  # Discover all lights from all providers
  lightwire-populate

  # Discover only LIFX lights
  lightwire-populate --providers lifx

  # Preview changes without writing
  lightwire-populate --dry-run
```

### 2. `lightwire-daemon` — Main Sync Service

```bash
lightwire-daemon [OPTIONS]

Options:
  --providers <names>   Comma-separated providers (default: all available)
  --config <path>       Lightwire configuration file
  --no-populate         Skip initial population on startup

Examples:
  # Run with all providers
  lightwire-daemon

  # Run with specific providers only
  lightwire-daemon --providers lifx,hue
```

### 3. `lightwire-cli` — Management Commands

```bash
lightwire-cli <COMMAND>

Commands:
  list              List all configured lights
  remove <name>     Remove a light's config
  sync              One-time sync: light state → PipeWire
  reload            Signal PipeWire to reload configs

Examples:
  lightwire-cli list
  lightwire-cli remove bedroom
  lightwire-cli sync
  lightwire-cli reload
```

---

## Configuration

### User Config: `~/.config/lightwire/config.toml`

```toml
[daemon]
# Providers to load (defaults to all compiled-in providers)
providers = ["lifx", "hue"]
# Sync interval for light → PipeWire direction (ms)
sync_interval_ms = 1000

[pipewire]
config_dir = "~/.config/pipewire/pipewire.conf.d"
node_prefix = "lightwire"

[lifx]
discovery_timeout_ms = 5000
broadcast_address = "255.255.255.255"
port = 56700

[hue]
bridge_address = "192.168.1.100"
api_key = "your-api-key-here"

# Per-light overrides
[lights.bedroom]
min_brightness = 0.1    # Never go fully dark
max_brightness = 1.0
enabled = true

[lights.desk-lamp]
enabled = false         # Skip this light
```

---

## Implementation Phases

### Phase 1: Core Foundation
- [ ] Define core types (`LightId`, `Brightness`, `LightState`, `ProviderError`)
- [ ] Implement `Provider` and `Light` traits (Proposal B)
- [ ] Implement `ProviderRegistry`
- [ ] Unit tests for registry and types

### Phase 2: LIFX Provider
- [ ] Implement `LifxProvider` with discovery
- [ ] UDP socket management and message framing
- [ ] Brightness get/set commands
- [ ] Unit tests with mock UDP

### Phase 3: PipeWire Integration
- [ ] Drop-in config generation
- [ ] `lightwire-populate` CLI tool
- [ ] Volume change monitoring via `pipewire-native`
- [ ] Config file lifecycle management

### Phase 4: Sync Daemon
- [ ] Implement `lightwire-daemon`
- [ ] Bidirectional sync (PipeWire ↔ Lights)
- [ ] Error recovery and reconnection
- [ ] systemd service files

### Phase 5: Multi-Provider Support
- [ ] Add second provider (Hue or WLED)
- [ ] Provider-specific configuration
- [ ] Mixed-provider testing

### Phase 6: Polish and Distribution
- [ ] Shell completions
- [ ] Man pages
- [ ] AUR/Homebrew packages

---

## Success Criteria

1. **Configuration**: `lightwire-populate` creates correct drop-in configs for discovered bulbs
2. **Visibility**: Virtual nodes appear in `pavucontrol`, GNOME Settings, etc. after PipeWire reload
3. **Responsiveness**: Volume changes translate to brightness changes within 100ms
4. **Multi-Provider**: Single daemon instance supports multiple provider types simultaneously
5. **Offline Operation**: Works entirely on LAN (no cloud/internet required)
6. **Clean Removal**: Removing a config causes the node to disappear after PipeWire reload
7. **Extensibility**: New providers can be added without modifying existing code

---

## Dependencies

```toml
[dependencies]
pipewire-native = "0.1"
lifx-core = "0.4"
tokio = { version = "1", features = ["net", "rt-multi-thread", "fs", "macros"] }
figment2 = "0.4"
clap = { version = "4", features = ["derive", "env"] }
jiff = "0.1"
tracing = "0.1"
directories = "5"
async-trait = "0.1"
serde = { version = "1", features = ["derive"] }
toml = "0.8"
thiserror = "1"
```

---

## Appendix A: Alternative Provider Designs Considered

### Proposal A: Trait with Associated Types

Each provider defines its own `Light` type with provider-specific details. The trait uses associated types for zero-cost abstraction.

```rust
/// Provider trait with associated Light type
pub trait Provider {
    /// Provider-specific light representation
    type Light: Light;
    
    /// Provider name for config file prefixes
    fn name(&self) -> &'static str;
    
    /// Discover all lights on the network
    async fn discover(&self) -> Result<Vec<Self::Light>, ProviderError>;
    
    /// Get current state of a specific light
    async fn get_state(&self, id: &LightId) -> Result<LightState, ProviderError>;
    
    /// Set brightness (and optionally power) for a light
    async fn set_brightness(&self, id: &LightId, brightness: Brightness) -> Result<(), ProviderError>;
}

/// Common light interface
pub trait Light {
    fn id(&self) -> &LightId;
    fn label(&self) -> &str;
    fn state(&self) -> &LightState;
}

/// LIFX-specific implementation
pub struct LifxProvider {
    socket: UdpSocket,
    timeout: Duration,
}

pub struct LifxLight {
    pub id: LightId,
    pub label: String,
    pub addr: SocketAddr,
    pub state: LightState,
    // LIFX-specific fields
    pub hue: u16,
    pub saturation: u16,
    pub kelvin: u16,
}

impl Provider for LifxProvider {
    type Light = LifxLight;
    fn name(&self) -> &'static str { "lifx" }
    // ... implementation
}
```

**Pros:**
- Type-safe at compile time
- Zero-cost abstraction (no heap allocation)
- Provider-specific Light types with full access to fields
- Excellent IDE support and auto-completion

**Cons:**
- Cannot easily store `Vec<Box<dyn Provider>>` for multi-provider support
- Requires compile-time knowledge of all providers
- Each provider usage site must be generic over the Provider type
- Difficult to support runtime provider registration

**Verdict:** Rejected in favor of Proposal B. While type-safe, the lack of runtime polymorphism makes multi-provider support cumbersome. Would require significant generic propagation throughout the codebase.

---

### Proposal C: Enum-Based (Closed Set of Providers)

If the set of providers is known at compile time, use enums for exhaustive matching.

```rust
/// Closed set of light types
#[derive(Clone, Debug)]
pub enum LightKind {
    Lifx(LifxLight),
    Hue(HueLight),
    Wled(WledLight),
}

impl LightKind {
    pub fn id(&self) -> &LightId {
        match self {
            Self::Lifx(l) => &l.id,
            Self::Hue(l) => &l.id,
            Self::Wled(l) => &l.id,
        }
    }
    
    pub fn label(&self) -> &str {
        match self {
            Self::Lifx(l) => &l.label,
            Self::Hue(l) => &l.label,
            Self::Wled(l) => &l.label,
        }
    }
}

/// Closed set of providers
#[derive(Clone)]
pub enum ProviderKind {
    Lifx(LifxProvider),
    Hue(HueProvider),
    Wled(WledProvider),
}

impl ProviderKind {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Lifx(_) => "lifx",
            Self::Hue(_) => "hue",
            Self::Wled(_) => "wled",
        }
    }
    
    pub async fn discover(&self) -> Result<Vec<LightKind>, ProviderError> {
        match self {
            Self::Lifx(p) => p.discover()
                .await
                .map(|v| v.into_iter().map(LightKind::Lifx).collect()),
            Self::Hue(p) => p.discover()
                .await
                .map(|v| v.into_iter().map(LightKind::Hue).collect()),
            Self::Wled(p) => p.discover()
                .await
                .map(|v| v.into_iter().map(LightKind::Wled).collect()),
        }
    }
    
    pub async fn get_state(&self, id: &LightId) -> Result<LightState, ProviderError> {
        match self {
            Self::Lifx(p) => p.get_state(id).await,
            Self::Hue(p) => p.get_state(id).await,
            Self::Wled(p) => p.get_state(id).await,
        }
    }
    
    pub async fn set_brightness(&self, id: &LightId, brightness: Brightness) 
        -> Result<(), ProviderError> {
        match self {
            Self::Lifx(p) => p.set_brightness(id, brightness).await,
            Self::Hue(p) => p.set_brightness(id, brightness).await,
            Self::Wled(p) => p.set_brightness(id, brightness).await,
        }
    }
}
```

**Pros:**
- No heap allocation (stack-allocated enums)
- Exhaustive matching ensures all cases handled
- Full access to provider-specific fields through pattern matching
- Zero-cost abstraction

**Cons:**
- Adding a provider requires modifying core enums
- Not extensible by users (closed set)
- Code bloat: every new provider adds variants to all match statements
- Cannot support plugin-style architecture
- Refactoring becomes tedious with many providers

**Verdict:** Rejected. The closed nature conflicts with our extensibility goals. While enums are efficient, the maintenance burden of modifying core types for each new provider outweighs the performance benefits.

---

## Appendix B: Open Questions

1. **Hot Reload** - Can PipeWire reload configs without full restart?
   - Investigate `pw-cli load-module` or SIGHUP
   - May need to document "restart required" for now

2. **Node Matching** - How to reliably match node.name to bulb after PipeWire restart?
   - Use deterministic naming: `lightwire.<provider>.<sanitized-label>`
   - Store mapping in state file if needed

3. **Mute Handling** - What happens when node is muted?
   - Option A: Set brightness to 0 (lights off)
   - Option B: Ignore mute, only respond to volume
   - Option C: Configurable per-light

4. **Volume Curve** - Linear or logarithmic mapping?
   - Humans perceive brightness logarithmically
   - May need configurable curve functions

5. **Group Control** - Support for controlling multiple lights as one node?
   - Implement as meta-provider that wraps multiple lights
   - Or use PipeWire node groups

---

## Appendix C: Glossary

- **PipeWire** - Modern Linux audio server replacing PulseAudio and JACK
- **Drop-in Config** - Configuration snippet placed in a directory, automatically loaded by the service
- **Virtual Node** - Software audio device that doesn't correspond to physical hardware
- **Provider** - Implementation of the Light/Provider traits for a specific ecosystem
- **LIFX** - Brand of WiFi-connected smart bulbs using UDP-based LAN protocol
- **Brightness** - Normalized value 0.0-1.0 representing light output level
- **Volume** - Audio level 0.0-1.0, used as the control metaphor for brightness
