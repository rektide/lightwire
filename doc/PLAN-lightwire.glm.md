# Lightwire Design Document

> Control smart-bulb brightness as virtual PipeWire node's volume

## Problem Statement

Lightwire enables seamless control of smart lighting through any PipeWire-compatible audio interface. The system creates virtual audio sink nodes—one per light—that users can adjust through desktop mixers, media keys, or application volume controls. These volume changes (0-100%) are translated in real-time to brightness commands for smart bulbs on the local network.

**Key Requirements:**

1. **Provider Abstraction**: Support multiple smart-lighting ecosystems (LIFX, Philips Hue, WLED, etc.) through a unified interface without coupling core logic to any specific protocol
2. **Dynamic Multi-Provider**: Runtime configuration of multiple providers simultaneously, enabling mixed-light setups
3. **Zero Cloud Dependency**: All communication must be local/LAN-based only
4. **Real-Time Synchronization**: Sub-100ms latency from PipeWire volume change to light brightness adjustment
5. **Extensibility**: Users should be able to add custom providers without modifying core lightwire code

The design challenge is creating a flexible abstraction layer that balances:
- Runtime flexibility (trait objects)
- Clean separation of concerns
- Reasonable performance overhead
- Ease of testing and mocking

---

## Available Components & Tools

### PipeWire Integration

**Library**: `pipewire-native` (pure Rust)
- Native Rust implementation of PipeWire protocol
- Event-driven architecture with `MainLoop`/`ThreadLoop`
- Full proxy system for Node/Registry interaction
- Key APIs: `Context`, `Core`, `Registry`, `Node` proxy

**Alternative**: `pw-cli` subprocess invocation
- Simpler but less reliable for real-time monitoring
- Used for initial sync operations

### Configuration Management

**Library**: `figment2`
- Type-safe configuration loading from TOML/YAML
- Environment variable support
- Hierarchical profiles (dev, production, etc.)
- XDG-compliant config directory handling via `directories` crate

### Async Runtime

**Library**: `tokio` with `rt-multi-thread`
- Async/await for network operations (UDP, HTTP)
- Compatible with PipeWire's callback-based event loop
- `tokio::net::UdpSocket` for LIFX protocol
- `reqwest` for future Hue bridge API integration

### Smart Light Protocol Libraries

**LIFX**: `lifx-core` v0.4
- Pure Rust LIFX LAN protocol
- Discovery via UDP broadcast
- Minimal dependencies (`byteorder`, `thiserror`)

**Hue**: `huey` or custom HTTP client
- Philips Hue bridge API via HTTP
- Discovery via SSDP/mDNS
- Requires bridge IP and username

**WLED**: Custom HTTP/JSON client
- RESTful API over HTTP
- JSON status/control endpoints
- Simple protocol, easy to implement

### CLI & Testing

**CLI**: `clap` v4 with derive macros
- Declarative argument parsing
- Subcommand structure
- Auto-generated help and completion via `clap_complete`

**Testing**: `nextest`
- Faster test runner with better output
- Integration test support with test fixtures

**Formatting**: `oxfmt` for consistent code style

### Error Handling

- `thiserror` for provider-specific error types
- `anyhow` for application-level error context
- Unified `ProviderError` enum for trait object compatibility

---

## Solution: Trait-Based Dynamic Dispatch

We use trait objects (`Box<dyn Provider>`) for runtime polymorphism, enabling multiple providers to coexist in a single daemon instance. This approach trades minor heap allocation overhead for maximal flexibility and extensibility.

### Core Abstractions

```rust
use std::net::SocketAddr;

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
    #[error("Light not found: {0}")]
    NotFound(LightId),
    #[error("Timeout: {0}")]
    Timeout(String),
}
```

### Light Trait Object

```rust
/// Shared interface for all light types
pub trait Light: Send + Sync {
    /// Unique identifier
    fn id(&self) -> &LightId;

    /// User-friendly label
    fn label(&self) -> &str;

    /// Provider name for namespacing
    fn provider_name(&self) -> &str;

    /// Current state (cached or live)
    fn state(&self) -> &LightState;

    /// Optional: provider-specific metadata access
    fn metadata(&self) -> Option<&std::collections::HashMap<String, String>> {
        None
    }
}
```

### Provider Trait Object

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

    /// Optional: provider-specific configuration
    fn config(&self) -> Option<&figment2::Figment> {
        None
    }
}
```

### Provider Registry

```rust
/// Registry managing multiple providers
pub struct ProviderRegistry {
    providers: HashMap<String, Box<dyn Provider>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
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
                    tracing::error!("Failed to discover lights from {}: {}", name, e);
                }
            }
        }
        Ok(all_lights)
    }

    /// Find a specific light by ID across all providers
    pub fn find_light(&self, id: &LightId) -> Option<(String, Box<dyn Light>)> {
        for (name, provider) in &self.providers {
            // This requires each provider to implement a lightweight lookup
            // In practice, we'd cache discovered lights or have a lookup method
        }
        None
    }
}
```

### Example: LIFX Implementation

```rust
pub struct LifxProvider {
    socket: tokio::net::UdpSocket,
    timeout: Duration,
    broadcast_addr: SocketAddr,
}

impl LifxProvider {
    pub fn new(timeout_ms: u64, broadcast: SocketAddr) -> io::Result<Self> {
        let socket = tokio::net::UdpSocket::bind("0.0.0.0:0")?;
        Ok(Self {
            socket,
            timeout: Duration::from_millis(timeout_ms),
            broadcast_addr: broadcast,
        })
    }
}

#[async_trait]
impl Provider for LifxProvider {
    fn name(&self) -> &'static str {
        "lifx"
    }

    async fn discover(&self) -> Result<Vec<Box<dyn Light>>, ProviderError> {
        // Send GetService broadcast, collect responses
        // Return Vec<Box<LifxLight>>
        let mut lights = Vec::new();
        // ... discovery logic ...
        Ok(lights)
    }

    async fn get_state(&self, id: &LightId) -> Result<LightState, ProviderError> {
        // Send GetPower and GetColor messages
        // Return LightState
    }

    async fn set_brightness(&self, id: &LightId, brightness: Brightness) -> Result<(), ProviderError> {
        // Send SetColor with brightness
        Ok(())
    }
}

pub struct LifxLight {
    pub id: LightId,
    pub label: String,
    pub addr: SocketAddr,
    pub state: LightState,
    pub port: u16,
}

impl Light for LifxLight {
    fn id(&self) -> &LightId { &self.id }
    fn label(&self) -> &str { &self.label }
    fn provider_name(&self) -> &str { "lifx" }
    fn state(&self) -> &LightState { &self.state }
}
```

---

## Implementation Strategies

### Strategy A: Monolithic with Provider Modules

**File Structure:**

```
lightwire/
├── Cargo.toml
├── src/
│   ├── lib.rs                    # Public API, re-exports
│   ├── error.rs                  # Error types
│   ├── types.rs                  # LightId, Brightness, LightState
│   ├── traits.rs                 # Provider and Light traits
│   ├── registry.rs              # ProviderRegistry implementation
│   ├── config.rs                 # Config loading with figment2
│   ├── provider/
│   │   ├── mod.rs                # Provider implementations
│   │   ├── lifx.rs               # LIFX provider
│   │   ├── hue.rs                # Hue provider (future)
│   │   └── wled.rs               # WLED provider (future)
│   ├── pipewire/
│   │   ├── mod.rs
│   │   ├── dropin.rs             # Config file generation
│   │   ├── volume.rs             # Volume operations
│   │   └── monitor.rs            # Real-time volume monitoring
│   └── bin/
│       ├── populate.rs           # lightwire-populate CLI
│       ├── sync-to-pipewire.rs    # lightwire-sync-to-pipewire CLI
│       └── sync-to-light.rs       # lightwire-sync-to-light daemon
└── tests/
    └── integration/
        ├── e2e_test.rs
        └── fixtures/
            └── pipewire/
                └── test-dropin.conf
```

**Pros:**
- Simple dependency graph
- All code in one crate, easy to navigate
- Easy to add new providers (new file in provider/)
- Binaries share library code naturally

**Cons:**
- Single large crate may become unwieldy
- Harder to release provider-specific libraries separately
- Tests for different features intermingled

---

### Strategy B: Workspace with Separate Provider Crates

**File Structure:**

```
lightwire/
├── Cargo.toml                    # Workspace root
├── Cargo.lock
├── lightwire-core/               # Core library
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── error.rs
│       ├── types.rs
│       ├── traits.rs
│       └── registry.rs
├── lightwire-pipewire/           # PipeWire integration
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── dropin.rs
│       ├── volume.rs
│       └── monitor.rs
├── lightwire-provider-lifx/      # LIFX provider crate
│   ├── Cargo.toml
│   └── src/lib.rs
├── lightwire-provider-hue/       # Hue provider crate (optional)
│   ├── Cargo.toml
│   └── src/lib.rs
├── lightwire-cli/                # CLI binaries
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── populate.rs
│       ├── sync_to_pipewire.rs
│       └── sync_to_light.rs
└── tests/
    └── integration/
        └── e2e_test.rs
```

**Cargo Workspace Root:**

```toml
[workspace]
members = [
    "lightwire-core",
    "lightwire-pipewire",
    "lightwire-provider-lifx",
    "lightwire-provider-hue",
    "lightwire-cli",
]
resolver = "2"

[workspace.dependencies]
tokio = { version = "1", features = ["net", "rt-multi-thread"] }
thiserror = "1"
async-trait = "0.1"
figment2 = "0.4"
```

**Pros:**
- Clean separation of concerns
- Can publish provider crates independently
- Easy to add external providers
- Faster compilation (crate cache)
- Easier testing per-component

**Cons:**
- More complex dependency management
- Version synchronization required
- More boilerplate for small providers

---

### Strategy C: Feature-Flagged Monolith

**File Structure:**

```
lightwire/
├── Cargo.toml                    # Feature flags
├── src/
│   ├── lib.rs
│   ├── error.rs
│   ├── types.rs
│   ├── traits.rs
│   ├── registry.rs
│   ├── pipewire/mod.rs
│   ├── provider/
│   │   ├── mod.rs
│   │   └── lifx.rs              # #[cfg(feature = "lifx")]
│   └── bin/
│       └── main.rs              # Dynamic binary selection
```

**Cargo.toml:**

```toml
[package]
name = "lightwire"
version = "0.1.0"
edition = "2021"

[features]
default = ["lifx"]
lifx = ["dep:lifx-core"]
hue = ["dep:huey"]
wled = ["dep:reqwest"]

[dependencies]
# Core deps
tokio = { version = "1", features = ["net", "rt-multi-thread"] }
thiserror = "1"
async-trait = "0.1"
figment2 = "0.4"
pipewire-native = "0.1"

# Provider deps (optional)
lifx-core = { version = "0.4", optional = true }
huey = { version = "0.5", optional = true }
reqwest = { version = "0.11", optional = true }

[[bin]]
name = "lightwire-populate"
path = "src/bin/populate.rs"
required-features = ["lifx"]

[[bin]]
name = "lightwire-sync-to-pipewire"
path = "src/bin/sync_to_pipewire.rs"

[[bin]]
name = "lightwire-sync-to-light"
path = "src/bin/sync_to_light.rs"
```

**Pros:**
- Single binary distribution per feature set
- Conditional compilation keeps binary small
- Users only pay for providers they use
- Simpler than workspace for small projects

**Cons:**
- Can't add external providers easily
- All provider code in same repo
- Feature flag complexity can grow

---

## Recommended Approach

**Start with Strategy B (Workspace with Separate Provider Crates):**

1. **Initial implementation**: `lightwire-core` + `lightwire-pipewire` + `lightwire-provider-lifx`
2. **Add CLI**: `lightwire-cli` depends on core, pipewire, and selected providers
3. **Future growth**: Add `lightwire-provider-hue`, `lightwire-provider-wled` as needed

**Rationale:**
- Clean separation from day one
- Allows publishing provider crates for community use
- Fast compilation during development
- Easy to test components in isolation
- Future-proof for external provider ecosystem

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    lightwire-cli                             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐  │
│  │  populate    │  │sync-to-pw    │  │  sync-to-light   │  │
│  └──────────────┘  └──────────────┘  └──────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                          │
        ┌─────────────────┼─────────────────┐
        ▼                 ▼                 ▼
┌───────────────┐  ┌─────────────────┐  ┌─────────────┐
│   pipewire    │  │     core        │  │  providers  │
│   integration │  │   (traits)      │  │ (dyn objs)  │
└───────────────┘  └─────────────────┘  └─────────────┘
        │                 │                     │
        ▼                 ▼                     ▼
┌───────────────┐  ┌─────────────────┐  ┌─────────────┐
│ pw-native     │  │ ProviderRegistry│  │ LifxProvider│
│ dropin files  │  │ LightId         │  │ HueProvider │
│ volume monitor│  │ Brightness      │  │ WledProvider│
└───────────────┘  └─────────────────┘  └─────────────┘
                                           │
                                           ▼
                                  ┌─────────────────┐
                                  │ Physical Lights │
                                  │ (LIFX/Hue/WLED) │
                                  └─────────────────┘
```

---

## Appendix: Rejected Alternatives

### Alternative A: Associated Types (Compile-Time Polymorphism)

**Proposal:**

```rust
pub trait Provider {
    type Light: Light;

    fn name(&self) -> &'static str;
    async fn discover(&self) -> Result<Vec<Self::Light>, ProviderError>;
    async fn set_brightness(&self, light: &Self::Light, brightness: Brightness)
        -> Result<(), ProviderError>;
}

// Usage: requires concrete types
async fn run<P: Provider>(provider: P) { ... }
```

**Pros:**
- Zero-cost abstraction (no heap allocation)
- Full type safety
- Compiler optimizes away all abstraction layers
- Provider-specific Light types can have rich fields

**Cons:**
- Cannot store `Vec<Box<dyn Provider>>` for multi-provider support
- Each provider requires separate generic instantiation
- Adding new providers requires code changes in consuming code
- No runtime provider registration

**Rejected Because:**
- Core requirement is **dynamic multi-provider support**
- Users need to configure providers at runtime via config file
- Mixed-light setups (e.g., LIFX + Hue) are a primary use case
- The performance overhead is negligible for this use case (IO-bound)

---

### Alternative C: Enum-Based Dispatch

**Proposal:**

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
            // ...
        }
    }
}
```

**Pros:**
- No heap allocation (enum is stack-allocated)
- Exhaustive pattern matching (compiler catches missing cases)
- Access to provider-specific fields without downcasting
- Zero runtime overhead

**Cons:**
- Closed set of providers (enum variants are compile-time)
- Adding new provider requires modifying enum and all match statements
- Cannot support external/community providers
- Enum grows with each new provider
- Breaking change for provider additions

**Rejected Because:**
- Extensibility is a key requirement
- External providers (community plugins) are a goal
- Enum would become unwieldy with many providers
- No clear benefit over trait objects for this IO-bound use case
- Boilerplate grows with each new provider variant

---

### Alternative D: Plugin System (Dynamic Library Loading)

**Proposal:**

Use `libloading` crate to load provider libraries at runtime from a plugins directory.

```rust
pub struct PluginManager {
    libraries: Vec<libloading::Library>,
    providers: Vec<Box<dyn Provider>>,
}

impl PluginManager {
    pub fn load_from_dir(&mut self, path: &Path) -> Result<(), Error> {
        for entry in fs::read_dir(path)? {
            let lib = unsafe { libloading::Library::new(entry.path())? };
            let provider: Box<dyn Provider> = unsafe {
                lib.get::<extern "C" fn() -> Box<dyn Provider>>(b"create_provider")?()
            };
            self.providers.push(provider);
            self.libraries.push(lib);
        }
        Ok(())
    }
}
```

**Pros:**
- Maximum extensibility (users can compile their own providers)
- Dynamic loading without recompiling lightwire
- Separate distribution of providers
- Language-agnostic plugins (could use C ABI)

**Cons:**
- High complexity
- Safety concerns with `unsafe` and dynamic loading
- Platform-specific (works differently on Linux/Mac/Windows)
- Difficult to debug across ABI boundaries
- Requires C-compatible API surface
- Not necessary for Rust ecosystem (cargo handles this better)

**Rejected Because:**
- Cargo workspaces provide a better solution for Rust plugins
- Safety and simplicity are higher priorities
- No immediate need for non-Rust providers
- Complexity outweighs benefits for this project scope
- Users can add providers via Cargo dependency instead

---

### Alternative E: Message-Passing (Actor Model)

**Proposal:**

Use separate tokio tasks for each provider, communicating via channels.

```rust
pub struct ProviderActor {
    receiver: mpsc::Receiver<ProviderMessage>,
    provider: Box<dyn Provider>,
}

pub enum ProviderMessage {
    Discover { respond_to: oneshot::Sender<Result<Vec<LightState>, ProviderError>> },
    SetBrightness { light_id: LightId, brightness: Brightness },
}

async fn run_provider_actor(mut actor: ProviderActor) {
    while let Some(msg) = actor.receiver.recv().await {
        match msg {
            ProviderMessage::Discover { respond_to } => {
                let result = actor.provider.discover().await;
                let _ = respond_to.send(result);
            }
            ProviderMessage::SetBrightness { light_id, brightness } => {
                let _ = actor.provider.set_brightness(&light_id, brightness).await;
            }
        }
    }
}
```

**Pros:**
- Natural concurrency model
- Providers run in isolated tasks
- Backpressure via channel buffering
- Clear async boundaries

**Cons:**
- Overkill for single-threaded async runtime
- Message serialization overhead
- Harder to reason about call stacks
- More boilerplate for each provider operation
- Not necessary for this use case (providers are mostly IO-bound)

**Rejected Because:**
- Direct async calls are simpler and clearer
- tokio already handles concurrency efficiently
- Channel overhead is unnecessary complexity
- No isolation requirement (providers are trusted code)
- Makes error handling and debugging harder

---

## Summary

**Selected Design: Trait-Based Dynamic Dispatch (Strategy B)**

The trait object approach provides the right balance of:
- **Flexibility**: Runtime multi-provider support
- **Extensibility**: Easy to add new providers
- **Simplicity**: Clear, maintainable code
- **Performance**: Acceptable overhead for IO-bound operations
- **Ecosystem**: Publishable provider crates for community

The workspace structure (Strategy B) ensures clean separation while enabling independent provider development and testing.
