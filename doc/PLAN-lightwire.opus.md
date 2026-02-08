# Lightwire Design Document

> Control smart-bulb brightness as virtual PipeWire node's volume

## Overview

Lightwire creates virtual PipeWire audio sink nodes—one per light—by managing drop-in configuration files in `~/.config/pipewire/pipewire.conf.d/`. When applications or the user adjusts the volume of these virtual nodes, Lightwire monitors the changes and translates them to brightness commands for LIFX smart bulbs on the local network.

This enables controlling light brightness through any PipeWire-compatible volume interface (desktop mixers, media keys, application settings).

---

## Architecture

```
┌─────────────────┐     ┌────────────────────┐     ┌───────────────┐
│  PipeWire       │────▶│    Lightwire       │────▶│  LIFX Bulbs   │
│  Volume Control │     │  (daemon)          │     │  (LAN UDP)    │
└─────────────────┘     └────────────────────┘     └───────────────┘
        │                        │                        │
   User adjusts          Monitors Props           Sends brightness
   volume 0-100%         parameter changes        commands 0-100%
                                 │
                                 ▼
                    ┌────────────────────────┐
                    │  pipewire.conf.d/      │
                    │  lightwire-lifx-*.conf │
                    └────────────────────────┘
```

### Components

1. **Config Manager** - Generates/removes PipeWire drop-in configs for each discovered light
2. **PipeWire Monitor** - Connects to PipeWire, watches for volume changes on managed nodes
3. **LIFX Bridge** - Translates volume (0.0-1.0) to brightness and sends to bulbs
4. **Device Discovery** - Finds LIFX bulbs on the local network

---

## Virtual Node Creation via Drop-in Configs

Since `pipewire-native-rs` does not yet support `create_object`, we use PipeWire's native configuration system. Each light gets a drop-in file:

**File:** `~/.config/pipewire/pipewire.conf.d/lightwire-lifx-<label>.conf`

```
context.objects = [
  {
    factory = adapter
    args = {
      factory.name = support.null-audio-sink
      node.name = "lightwire.lifx.<label>"
      node.description = "LIFX: <Label>"
      media.class = Audio/Sink
      object.linger = true
      audio.position = [ FL FR ]
      monitor.channel-volumes = true
    }
  }
]
```

### Lifecycle

1. **Discovery** - `lightwire scan` discovers LIFX bulbs on LAN
2. **Sync** - `lightwire sync` creates/updates drop-in configs for each bulb
3. **Reload** - Signal PipeWire to reload: `systemctl --user restart pipewire.service` (or `pw-cli load-module`)
4. **Monitor** - `lightwire daemon` watches node volume changes and forwards to bulbs
5. **Cleanup** - `lightwire remove <label>` deletes the drop-in config

### File Naming Convention

```
~/.config/pipewire/pipewire.conf.d/
├── lightwire-lifx-bedroom.conf
├── lightwire-lifx-living-room.conf
└── lightwire-lifx-desk-lamp.conf
```

- Prefix: `lightwire-<provider>-` (e.g., `lightwire-lifx-`)
- Suffix: sanitized bulb label (lowercase, hyphens for spaces)
- Extension: `.conf`

---

## Technology Selection

### PipeWire Client: `pipewire-native`

**Crate:** `pipewire-native` (pure Rust, no FFI)

Rationale:
- Native Rust implementation of PipeWire protocol
- No C dependencies or bindgen complexity
- Full proxy system for Node/Registry interaction
- Event-driven architecture with `MainLoop`/`ThreadLoop`

Key APIs needed:
- `MainLoop` / `ThreadLoop` for event loop
- `Context` and `Core` for server connection
- `Registry` for object enumeration and binding to nodes
- `Node` proxy for subscribing to parameter changes (Props)

### LIFX Control: `lifx-core`

**Crate:** `lifx-core` v0.4

Rationale:
- Local/LAN protocol only (no cloud dependency)
- Minimal dependencies (`byteorder`, `thiserror`)
- Full protocol coverage including brightness control
- No external server process required

Trade-off: We must implement UDP I/O ourselves, but this gives full control over discovery and command timing.

---

## CLI Tools

Three focused CLI tools, each with a single responsibility:

### 1. `lightwire-populate` — Discover lights, create PipeWire configs

```
lightwire-populate [OPTIONS]

Discovers lights on the network and creates PipeWire drop-in configs.

Options:
  --provider <name>     Light provider (default: lifx)
  --config-dir <path>   PipeWire config directory
  --dry-run             Show what would be created without writing
  --clean               Remove configs for lights no longer found
```

### 2. `lightwire-sync-to-pipewire` — Light state → PipeWire volume

```
lightwire-sync-to-pipewire [OPTIONS]

Reads current brightness from lights and sets corresponding PipeWire node volumes.

Options:
  --provider <name>     Light provider (default: lifx)
  --once                Sync once and exit (default: watch for light changes)
  --interval <ms>       Polling interval for light state (default: 1000)
```

### 3. `lightwire-sync-to-light` — PipeWire volume → Light brightness

```
lightwire-sync-to-light [OPTIONS]

Watches PipeWire node volumes and updates light brightness accordingly.

Options:
  --provider <name>     Light provider (default: lifx)
  --once                Sync once and exit (default: watch for volume changes)
```

### Example Workflow

```bash
# Discover and create configs
$ lightwire-populate --provider lifx
Found 3 LIFX bulbs:
  - Bedroom (d073d5xxxxxx)
  - Living Room (d073d5yyyyyy)
  - Desk Lamp (d073d5zzzzzz)
Created: ~/.config/pipewire/pipewire.conf.d/lightwire-lifx-bedroom.conf
Created: ~/.config/pipewire/pipewire.conf.d/lightwire-lifx-living-room.conf
Created: ~/.config/pipewire/pipewire.conf.d/lightwire-lifx-desk-lamp.conf

# Restart PipeWire to load new nodes
$ systemctl --user restart pipewire

# Initialize PipeWire volumes from current light brightness
$ lightwire-sync-to-pipewire --provider lifx --once

# Run the daemon to push volume changes to lights
$ lightwire-sync-to-light --provider lifx
Watching: lightwire.lifx.bedroom, lightwire.lifx.living-room, lightwire.lifx.desk-lamp
```

---

## Core Data Flow

### Startup Sequence (daemon mode)

```
1. Read existing lightwire-*.conf files to get managed node names
2. Initialize pipewire::init()
3. Create MainLoop with app properties
4. Connect to PipeWire server via Context
5. Get Registry, enumerate existing nodes
6. For each managed node found:
   a. Bind to node proxy
   b. Subscribe to Props parameters
7. Start LIFX discovery to map node names → bulb addresses
8. Run main loop
```

### Volume Change Handling

```
1. PipeWire emits NodeEvents::param with Props
2. Extract volume from Props POD structure (channelVolumes)
3. Look up bulb address from node.name
4. Clamp volume to 0.0..1.0 range
5. Convert to LIFX brightness (0-65535 u16)
6. Send SetColor to the specific bulb
```

---

## Module Structure

```
lightwire/
├── Cargo.toml
├── src/
│   ├── lib.rs                    # Core library
│   ├── bin/
│   │   ├── lightwire-populate.rs
│   │   ├── lightwire-sync-to-pipewire.rs
│   │   └── lightwire-sync-to-light.rs
│   ├── provider/
│   │   ├── mod.rs                # Provider trait + LightState
│   │   └── lifx.rs               # LIFX implementation
│   ├── pipewire/
│   │   ├── mod.rs
│   │   ├── dropin.rs             # Config file generation
│   │   ├── volume.rs             # Volume get/set via pw-cli or native
│   │   └── monitor.rs            # Watch for volume changes
│   └── types.rs                  # LightId, Brightness, etc.
└── lightwire-lifx/               # Optional standalone crate
    ├── Cargo.toml
    └── src/lib.rs
```

---

## Provider Interface Design

The provider abstraction is critical for supporting multiple light ecosystems (LIFX, Hue, WLED, etc.). Here are three proposals:

### Proposal A: Trait with Associated Types (Recommended)

Each provider defines its own `Light` type with provider-specific details. The trait uses associated types for zero-cost abstraction.

```rust
/// Unique identifier for a light within a provider
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LightId(pub String);

/// Normalized brightness value
#[derive(Clone, Copy, Debug)]
pub struct Brightness(f32);  // 0.0..=1.0

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
    // Provider can store extra data in `extra`
}

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
```

**LIFX Implementation:**

```rust
pub struct LifxProvider {
    socket: UdpSocket,
    timeout: Duration,
}

pub struct LifxLight {
    pub id: LightId,
    pub label: String,
    pub addr: SocketAddr,
    pub state: LightState,
    // LIFX-specific: color, kelvin, etc.
    pub hue: u16,
    pub saturation: u16,
    pub kelvin: u16,
}

impl Provider for LifxProvider {
    type Light = LifxLight;
    fn name(&self) -> &'static str { "lifx" }
    // ...
}
```

**Pros:** Type-safe, zero-cost, provider can have rich Light types  
**Cons:** Cannot easily store `Vec<Box<dyn Provider>>` for multi-provider support

---

### Proposal B: Trait Objects with Dynamic Dispatch

Use trait objects for runtime polymorphism, enabling multi-provider support in a single daemon.

```rust
/// Light as a trait object
pub trait Light: Send + Sync {
    fn id(&self) -> &LightId;
    fn label(&self) -> &str;
    fn provider_name(&self) -> &str;
}

/// Provider as a trait object
#[async_trait]
pub trait Provider: Send + Sync {
    fn name(&self) -> &'static str;
    
    async fn discover(&self) -> Result<Vec<Box<dyn Light>>, ProviderError>;
    async fn get_state(&self, id: &LightId) -> Result<LightState, ProviderError>;
    async fn set_brightness(&self, id: &LightId, brightness: Brightness) -> Result<(), ProviderError>;
}

/// Registry of providers
pub struct ProviderRegistry {
    providers: HashMap<String, Box<dyn Provider>>,
}

impl ProviderRegistry {
    pub fn register(&mut self, provider: Box<dyn Provider>) {
        self.providers.insert(provider.name().to_string(), provider);
    }
    
    pub async fn discover_all(&self) -> Result<Vec<Box<dyn Light>>, ProviderError> {
        let mut lights = Vec::new();
        for provider in self.providers.values() {
            lights.extend(provider.discover().await?);
        }
        Ok(lights)
    }
}
```

**Pros:** Easy multi-provider support, runtime flexibility  
**Cons:** Heap allocation, `async_trait` macro overhead, loses provider-specific Light fields

---

### Proposal C: Enum-Based (Closed Set of Providers)

If the set of providers is known at compile time, use enums for exhaustive matching.

```rust
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
            Self::Lifx(p) => p.discover().await.map(|v| v.into_iter().map(LightKind::Lifx).collect()),
            Self::Hue(p) => p.discover().await.map(|v| v.into_iter().map(LightKind::Hue).collect()),
            Self::Wled(p) => p.discover().await.map(|v| v.into_iter().map(LightKind::Wled).collect()),
        }
    }
}
```

**Pros:** No heap allocation, exhaustive matching, access to provider-specific fields  
**Cons:** Adding a provider requires modifying enums, not extensible by users

---

### Recommendation

**Start with Proposal A (Associated Types)** for the initial LIFX-only implementation:
- Clean separation, type-safe, zero overhead
- Easily testable with mock providers

**Migrate to Proposal B (Trait Objects)** when adding a second provider:
- Wrap each provider in `Box<dyn Provider>`
- Accept the minor overhead for runtime flexibility

The key types that remain stable across proposals:
- `LightId` — unique identifier
- `Brightness` — normalized 0.0–1.0
- `LightState` — common state snapshot
- `ProviderError` — unified error type

---

## Configuration

### User Config: `~/.config/lightwire/config.toml`

```toml
[pipewire]
config_dir = "~/.config/pipewire/pipewire.conf.d"
node_prefix = "lightwire"

[lifx]
discovery_timeout_ms = 5000
broadcast_address = "255.255.255.255"
port = 56700

# Per-light overrides
[lights."Bedroom"]
min_brightness = 0.1    # Never go fully dark
max_brightness = 1.0

[lights."Desk Lamp"]
enabled = false         # Skip this light
```

### Generated Drop-in: `lightwire-lifx-bedroom.conf`

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

## Key Implementation Details

### Drop-in File Generation

```rust
fn generate_dropin(light: &Light, provider: &str) -> String {
    let node_name = format!("lightwire.{}.{}", provider, sanitize_label(&light.label));
    format!(r#"
# Generated by lightwire - do not edit manually
# Light: {} ({})
# Provider: {}

context.objects = [
  {{
    factory = adapter
    args = {{
      factory.name = support.null-audio-sink
      node.name = "{}"
      node.description = "{}: {}"
      media.class = Audio/Sink
      object.linger = true
      audio.position = [ FL FR ]
      monitor.channel-volumes = true
    }}
  }}
]
"#, light.label, light.id, provider, node_name, provider.to_uppercase(), light.label)
}
```

### Volume Extraction from Props

```rust
node.add_listener(NodeEvents {
    param: some_closure!([bridge, node_name] seq, id, index, next, pod, {
        if id == spa::param::ParamType::Props {
            let volume = extract_volume_from_pod(pod);
            bridge.set_brightness(&node_name, volume);
        }
    }),
    ..Default::default()
});
```

### PipeWire Reload

```rust
fn reload_pipewire() -> io::Result<()> {
    // Option 1: systemctl (most reliable)
    Command::new("systemctl")
        .args(["--user", "restart", "pipewire.service"])
        .status()?;
    
    // Option 2: SIGHUP to pipewire process
    // Option 3: pw-cli command
    Ok(())
}
```

---

## Open Questions

1. **Hot Reload** - Can PipeWire reload configs without full restart?
   - Investigate `pw-cli load-module` or SIGHUP
   - May need to document "restart required" for now

2. **Node Matching** - How to reliably match node.name to bulb after PipeWire restart?
   - Use deterministic naming: `lightwire.<provider>.<sanitized-label>`
   - Store mapping in state file if needed

3. **Multiple Providers** - Future support for Hue, WLED, etc.
   - Provider trait with `discover()`, `set_brightness()`, `provider_name()`
   - Each provider generates its own prefixed configs

4. **Mute Handling** - What happens when node is muted?
   - Option A: Set brightness to 0 (lights off)
   - Option B: Ignore mute, only respond to volume
   - Option C: Configurable per-light

---

## Implementation Phases

### Phase 1: Core Types & LIFX Provider
- Define `LightId`, `Brightness`, `LightState`, `ProviderError`
- Implement `Provider` trait (Proposal A)
- Implement `LifxProvider` with discovery and brightness control
- Unit tests with mock UDP

### Phase 2: PipeWire Drop-in Generation
- Implement `pipewire::dropin` module
- `lightwire-populate` CLI tool
- Integration test: generate config, verify syntax

### Phase 3: PipeWire Volume Interface
- Implement `pipewire::volume` (get/set via `pw-cli` or `wpctl`)
- Implement `pipewire::monitor` (watch for changes via pipewire-native)
- `lightwire-sync-to-pipewire` CLI tool

### Phase 4: Light Sync Daemon
- `lightwire-sync-to-light` CLI tool
- Watch PipeWire volume → update light brightness
- Error recovery, reconnection logic

### Phase 5: Polish
- Systemd service files
- Documentation
- Optional: second provider (Hue/WLED) to validate Proposal B migration

---

## Dependencies

```toml
[dependencies]
pipewire-native = "0.1"      # PipeWire client
lifx-core = "0.4"            # LIFX protocol
tokio = { version = "1", features = ["net", "rt-multi-thread", "fs"] }
figment2 = "0.4"             # Config
clap = { version = "4", features = ["derive"] }
jiff = "0.1"                 # Time handling
tracing = "0.1"              # Logging
directories = "5"            # XDG paths
```

---

## Success Criteria

1. `lightwire sync` creates correct drop-in configs for discovered bulbs
2. Virtual nodes appear in `pavucontrol`, GNOME Settings, etc. after reload
3. `lightwire daemon` translates volume changes to brightness in real-time
4. Works entirely on LAN (no cloud/internet required)
5. Clean removal: `lightwire remove` deletes config, node disappears after reload
6. Sub-100ms latency from volume change to brightness change
