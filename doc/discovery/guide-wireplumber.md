# Guide: WirePlumber Rust Bindings

## Tech Stack Overview

This guide documents the WirePlumber Rust bindings located at `.test-agent/wireplumber.rs/`. The project provides Rust bindings for libwireplumber, a high-level session and policy manager for PipeWire.

### Core Technologies

- **Rust** (v1.70+) - Primary language, providing memory safety and modern async capabilities
- **GLib / GObject** - The underlying object system used by WirePlumber
- **GIO** - GLib's I/O library for async operations and D-Bus integration
- **PipeWire** - Low-level audio/video processing daemon
- **libspa** - Simple Plugin API, providing data structures for PipeWire
- **GIR (GObject Introspection)** - Tool for automatically generating Rust bindings from GObject-based C libraries
- **Nix** - Reproducible build system for development

### Key Dependencies

From the `Cargo.toml` at [Cargo.toml:1](.test-agent/wireplumber.rs/Cargo.toml):

```toml
[dependencies]
libc = "0.2"
glib = { version = "0.19" }
gio = { version = "0.19" }
pipewire-sys = { version = "0.8" }
libspa-sys = { version = "0.8" }
libspa = { version = "0.8", optional = true }
serde = { version = "1.0", optional = true }
ffi = { version = "0.1.0", path = "sys", package = "wireplumber-sys" }
bitflags = "2"
futures-channel = { version = "0.3", optional = true }
glib-signal = { version = "0.4", optional = true }
```

### Build System

- **GIR.toml** configuration at [Gir.toml:1](.test-agent/wireplumber.rs/Gir.toml) controls what GObject interfaces are exposed
- Most bindings are auto-generated from GObject introspection data
- Manual overrides provided for complex or unsafe operations

## WirePlumber Architecture

WirePlumber is a **session and policy manager** that sits between applications and PipeWire. It provides:

- **Policy management** - Deciding which audio/video streams should connect where
- **Session management** - Maintaining persistent state of the audio graph
- **Dynamic module loading** - Extensible plugin architecture
- **Lua scripting** - Built-in scripting engine for configuration
- **Object abstraction** - High-level view of PipeWire objects

### Relation to PipeWire

WirePlumber **orchestrates and controls** PipeWire concepts:

| PipeWire Concept | WirePlumber Abstraction | Location |
|-----------------|----------------------|----------|
| Nodes (processing units) | `Node`, `ImplNode` | [src/pw/node.rs](.test-agent/wireplumber.rs/src/pw/node.rs), [src/local/node.rs](.test-agent/wireplumber.rs/src/local/node.rs) |
| Links (connections) | `Link`, `SiLink` | [src/pw/link.rs](.test-agent/wireplumber.rs/src/pw/link.rs), [src/auto/si_link.rs](.test-agent/wireplumber.rs/src/auto/si_link.rs) |
| Ports (connection points) | `Port` | [src/pw/port.rs](.test-agent/wireplumber.rs/src/pw/port.rs) |
| Devices (hardware) | `Device`, `SpaDevice` | [src/auto/device.rs](.test-agent/wireplumber.rs/src/auto/device.rs) |
| Global objects | `GlobalProxy`, `PipewireObject` | [src/auto/global_proxy.rs](.test-agent/wireplumber.rs/src/auto/global_proxy.rs) |
| Properties (key-value metadata) | `Properties` | [src/pw/properties.rs](.test-agent/wireplumber.rs/src/pw/properties.rs) |
| Metadata (shared state) | `Metadata`, `ImplMetadata` | [src/auto/metadata.rs](.test-agent/wireplumber.rs/src/auto/metadata.rs) |

### Key API Modules

Based on [src/lib.rs](.test-agent/wireplumber.rs/src/lib.rs):

#### Core Module ([src/core/mod.rs](.test-agent/wireplumber.rs/src/core/mod.rs))

The entry point for WirePlumber. Provides initialization and connection management:

```rust
use wireplumber::Core;

// Initialize the library
Core::init();

// Run with a main loop
Core::run(Some(properties), |context, mainloop, core| {
    // Application logic here
});
```

Key types:
- `Core` - Main WirePlumber instance
- `Object` - Base type for all WirePlumber objects
- `ObjectFeatures` - Feature flags for object capabilities

**Key methods:**
- `Core::connect_future()` - Async connection to PipeWire daemon
- `Core::load_component()` - Load external modules
- `Core::install_object_manager()` - Install an object manager to watch for changes

#### Registry Module ([src/registry/mod.rs](.test-agent/wireplumber.rs/src/registry/mod.rs))

Provides change notification and filtering for PipeWire objects:

```rust
use wireplumber::{
    registry::{ObjectManager, Interest, Constraint, ConstraintType},
    pw::Node,
};

let om = ObjectManager::new();
om.add_interest([
    Constraint::compare(ConstraintType::PwProperty, "media.class", "Audio/Sink", true),
].iter().collect::<Interest<Node>>());

core.install_object_manager(&om);
```

Key types:
- `ObjectManager` - Watches for object changes and emits signals
- `Interest` - Filter criteria for objects
- `Constraint` - Individual filter conditions
- `ConstraintType` - What to filter (properties, type, etc.)
- `ConstraintVerb` - Comparison type (equals, matches, etc.)

**Features:**
- Signal-based change notifications (`OBJECT_ADDED`, `OBJECTS_CHANGED`)
- Filtering by object type and properties
- Asynchronous waiting for installation with `installed_future()`

#### PipeWire Proxy Module ([src/pw/mod.rs](.test-agent/wireplumber.rs/src/pw/mod.rs))

Represents PipeWire objects on the remote service:

Key types:
- `Proxy` - Base for all remote objects
- `Node` - Processing nodes (audio streams, devices)
- `Port` - Connection points on nodes
- `Link` - Connections between ports
- `Device` - Hardware devices
- `Client` - Connected clients
- `Metadata` - Global metadata store
- `Endpoint` - High-level endpoint abstraction
- `Properties` - Key-value metadata

**Usage pattern:** Cannot create directly; must obtain via registry.

#### Session Module ([src/session/mod.rs](.test-agent/wireplumber.rs/src/session/mod.rs))

Manages WirePlumber session items for policy implementation:

Key types:
- `SessionItem` - Base for session-managed objects
- `SiFactory` - Creates session items
- `SiLink` - Session-managed link between endpoints
- `SiLinkable` - Objects that can be linked
- `SiEndpoint` - Session endpoint abstraction
- `SiAdapter` - Adapts nodes for session management
- `SiAcquisition` - Manages acquisition of resources

**Extension traits:**
- `SiAdapterExt2::set_ports_format_future()` - Configure port formats
- `SiAcquisitionExt2::acquire_future()` - Acquire resources asynchronously

#### Plugin Module ([src/plugin/mod.rs](.test-agent/wireplumber.rs/src/plugin/mod.rs))

Enables dynamic module loading and custom plugin development:

**Loading plugins:**
```rust
use wireplumber::plugin::Plugin;

core.load_component("libwireplumber-module-lua-scripting", "module", None)?;
let plugin = Plugin::find(&core, "lua-scripting")?;
plugin.activate_future(PluginFeatures::ENABLED).await?;
```

**Writing plugins:**
```rust
use wireplumber::plugin::{SimplePlugin, AsyncPluginImpl};

#[derive(Default)]
struct MyPlugin {
    // Plugin state
}

impl AsyncPluginImpl for MyPlugin {
    type EnableFuture = Pin<Box<dyn Future<Output = Result<(), Error>>>>;

    fn enable(&self, this: Self::Type) -> Self::EnableFuture {
        // Initialization logic
    }

    fn disable(&self) {
        // Cleanup
    }
}

impl SimplePlugin for MyPlugin {
    type Args = MyArgs;

    fn init_args(&self, args: Self::Args) {
        // Parse arguments
    }
}

plugin::simple_plugin_subclass! {
    impl ObjectSubclass for "my-plugin" as MyPlugin { }
}

plugin::plugin_export!(MyPlugin);
```

**Key types:**
- `Plugin` - Loaded plugin instance
- `SimplePlugin` - Trait for easy plugin implementation
- `AsyncPluginImpl` - Async plugin interface
- `ComponentLoader` - Loads external components

#### Lua Module ([src/lua/mod.rs](.test-agent/wireplumber.rs/src/lua/mod.rs))

Integration with WirePlumber's Lua scripting engine:

Key types:
- `LuaTable` - Lua table representation
- `LuaValue` - Individual Lua value
- `LuaVariant` - GLib variant for Lua data
- `LuaString` - Lua string wrapper
- `LuaError` - Lua-specific errors

**Usage:**
```rust
core.load_lua_script("config.lua", args)?;
```

**Serde support:**
```rust
#[cfg(feature = "serde")]
use wireplumber::lua::{from_variant, to_variant};

// Convert Rust structs to/from Lua tables
```

#### SPA Module ([src/spa/mod.rs](.test-agent/wireplumber.rs/src/spa/mod.rs))

Provides access to PipeWire's Simple Plugin API data structures:

Key types:
- `SpaPod` - Plain Old Data container
- `SpaPodBuilder` - Build POD objects
- `SpaPodParser` - Parse POD objects
- `SpaJson` - JSON parsing (v0.4.8+)
- `SpaJsonBuilder` - Build JSON
- `SpaJsonParser` - Parse JSON
- `SpaType` - Type information
- `SpaIdTable`, `SpaIdValue` - ID tables and values

**Experimental features:**
- `SpaProps` - SPA properties
- `SpaRoute` - Audio routing information
- `SpaRoutes` - Collection of routes

#### Local Module ([src/local/mod.rs](.test-agent/wireplumber.rs/src/local/mod.rs))

Wrappers for creating local PipeWire objects:

Key types:
- `ImplNode` - Implement a local node
- `ImplModule` - Implement a local module
- `ImplEndpoint` - Implement a local endpoint
- `ImplMetadata` - Implement local metadata
- `SpaDevice` - Wrap an SPA device

These allow creating PipeWire objects directly from Rust, rather than just observing them.

## Storage Subsystem

WirePlumber provides a state persistence system via the `State` type at [src/auto/state.rs](.test-agent/wireplumber.rs/src/auto/state.rs):

### State Management

```rust
use wireplumber::State;

// Create a state object with a name
let state = State::new("my-state");

// Save properties to disk
state.save(&properties)?;

// Load properties from disk
if let Some(loaded) = state.load() {
    // Use loaded properties
}

// Get the storage location
let location = state.location();
let name = state.name();

// Clear the state
state.clear();
```

**Key methods:**
- `State::new(name)` - Create a new state object
- `save(props)` - Persist properties to disk
- `load()` - Load persisted properties
- `location()` - Get storage path
- `name()` - Get state name
- `clear()` - Clear persisted state

The state system stores `Properties` (key-value dictionaries) persistently, typically in XDG-compliant locations for user configuration data.

### Properties as Storage Format

The `Properties` type at [src/pw/properties.rs](.test-agent/wireplumber.rs/src/pw/properties.rs) provides the key-value storage format:

```rust
use wireplumber::pw::Properties;

let props = Properties::new();

// Insert values
props.insert("key.name", "my-value");

// Iterate over all properties
for (key, value) in &props {
    println!("{} = {}", key, value);
}

// Convert from iterator
let props: Properties = vec![
    ("key1".to_string(), "value1".to_string()),
    ("key2".to_string(), "value2".to_string()),
].into_iter().collect();
```

**Features:**
- Key-value storage with string keys and values
- Iterator support for accessing all entries
- Debug formatting for inspection
- Conversion from/to iterators

### Metadata System

In addition to persistent state, WirePlumber uses a metadata system for runtime shared state:

- `Metadata` - Global metadata accessible to all clients
- `ImplMetadata` - Create custom metadata sources
- Properties on PipeWire objects - Runtime object configuration

This allows dynamic policy communication between different components.

## Features and Capabilities

### Core Features

Based on [Cargo.toml:55](.test-agent/wireplumber.rs/Cargo.toml):

```toml
[features]
default = []
lua = []
experimental = []
glib-signal = ["dep:glib-signal"]
futures = ["glib-signal?/futures", "dep:futures-channel"]
libspa = ["dep:libspa"]
serde = ["dep:serde"]
v0_4_2 = ["ffi/v0_4_2"]
v0_4_3 = ["ffi/v0_4_3", "v0_4_2"]
...
v0_4_16 = ["ffi/v0_4_16", "v0_4_15"]
```

- **Lua** - Enable Lua scripting integration
- **Experimental** - Enable experimental SPA features
- **glib-signal** - Signal stream support for reactive programming
- **futures** - Async/await support with futures
- **libspa** - Direct access to libspa types
- **serde** - Serde serialization support for Lua variants
- **v0_4_X** - Version-specific API additions

### Async Operations

WirePlumber provides extensive async support:

```rust
use wireplumber::Core;

Core::run(Some(props), |context, mainloop, core| {
    context.spawn_local(async move {
        // Connect to PipeWire
        core.connect_future().await?;

        // Activate a plugin
        plugin.activate_future(PluginFeatures::ENABLED).await?;

        // Wait for object manager installation
        om.installed_future().await?;

        mainloop.quit();
    });
});
```

### Signal Streams

With the `glib-signal` feature:

```rust
use futures_util::StreamExt;

let mut added_stream = om.signal_stream(ObjectManager::SIGNAL_OBJECT_ADDED);

while let Some((obj,)) = added_stream.next().await {
    println!("Object added: {:?}", obj);
}
```

## Usage Examples

### Basic Connection Setup

From [examples/src/bin/exec.rs:75](.test-agent/wireplumber.rs/examples/src/bin/exec.rs):

```rust
use wireplumber::{
    Core,
    pw::Properties,
    plugin::*,
    prelude::*,
};

fn main() -> Result<()> {
    wireplumber::Log::set_default_level("3");
    Core::init();

    let props = Properties::new();
    props.insert(pw::PW_KEY_APP_NAME, "my-app");

    Core::run(Some(props), |context, mainloop, core| {
        context.spawn_local(async move {
            match core.connect_future().await {
                Ok(()) => println!("Connected to PipeWire!"),
                Err(e) => println!("Failed: {e:?}"),
            }
            mainloop.quit();
        });
    })
}
```

### Monitoring Nodes

From [examples/src/static-link.rs](.test-agent/wireplumber.rs/examples/src/static-link.rs):

```rust
use wireplumber::{
    registry::{ObjectManager, Interest, Constraint, ConstraintType},
    pw::Node,
    prelude::*,
};

let om = ObjectManager::new();
om.add_interest([
    Constraint::compare(ConstraintType::PwProperty, "media.class", "Audio/Sink", true),
].iter().collect::<Interest<Node>>());

let mut objects = om.signal_stream(ObjectManager::SIGNAL_OBJECT_ADDED);

om.request_object_features(Node::static_type(), ObjectFeatures::ALL);
core.install_object_manager(&om);

while let Some((obj,)) = objects.next().await {
    let node = obj.dynamic_cast_ref::<Node>().expect("Node expected");
    println!("New node: {:?}", node);
}
```

### Creating Links

```rust
use wireplumber::pw::{Link, Properties, ProxyFeatures};

let link_props = Properties::new();
link_props.insert(pw::PW_KEY_LINK_PASSIVE, true);
link_props.insert(pw::PW_KEY_OBJECT_LINGER, true);

let link = Link::new(&core, &output_port, &input_port, &link_props)?;
link.activate_future(ProxyFeatures::MINIMAL).await?;
```

## Comparison with pipewire-native-rs

WirePlumber.rs provides a **higher-level** API compared to [pipewire-native-rs](.test-agent/pipewire-native-rs):

### pipewire-native-rs

Located at `.test-agent/pipewire-native-rs/`, this is a **native Rust implementation** of the PipeWire protocol:

- **Direct protocol implementation** - Native implementation of PipeWire native protocol
- **Lower-level** - Closer to PipeWire's wire protocol
- **No GObject dependencies** - Pure Rust
- **Focus** - Safe, idiomatic Rust client library for PipeWire
- **Current status** - Supports connecting, enumerating objects, creating server-side objects

From [pipewire/src/lib.rs:7](.test-agent/pipewire-native-rs/pipewire/src/lib.rs):
```rust
//! A typical client would use the following steps:
//!
//!   * Create a MainLoop, and run it
//!   * Configure and create a Context
//!   * Connect to the server, which provides a Core
//!   * Request a Registry via the core
//!   * Listen for global events
//!   * Bind to the global objects you wish to interact with
//!   * For each object you bind to, you will get a proxy object
```

### WirePlumber.rs

- **Policy and session management** - Orchestrates PipeWire objects
- **Higher-level abstractions** - Session items, endpoints, adapters
- **GObject-based** - Uses GLib/GObject from libwireplumber
- **Lua integration** - Built-in scripting support
- **State persistence** - Configuration and state management
- **Dynamic modules** - Extensible plugin system
- **Focus** - Session management, policy enforcement, configuration

### When to Use Which

| Use Case | Recommended Library |
|-----------|-------------------|
| Simple PipeWire client (play/record) | pipewire-native-rs |
| Creating custom audio processing nodes | pipewire-native-rs |
| Building a full audio session manager | WirePlumber.rs |
| Implementing routing policies | WirePlumber.rs |
| Integrating with existing WirePlumber config | WirePlumber.rs |
| Writing WirePlumber modules/plugins | WirePlumber.rs |
| PipeWire protocol research/debugging | pipewire-native-rs |

## Canonical Repositories

- **WirePlumber C library**: https://gitlab.freedesktop.org/pipewire/wireplumber
  - Official C implementation
  - C API documentation: https://pipewire.pages.freedesktop.org/wireplumber/c_api.html

- **PipeWire**: https://github.com/PipeWire/pipewire
  - Mirror of official repository at https://gitlab.freedesktop.org/pipewire/pipewire
  - Low-level audio/video server
  - Documentation: https://docs.pipewire.org

- **wireplumber.rs**: https://github.com/arcnmx/wireplumber.rs
  - Rust bindings for WirePlumber
  - API documentation: https://arcnmx.github.io/wireplumber.rs/main/wireplumber/
  - Crate: `wireplumber` on crates.io

- **pipewire-native-rs**: https://gitlab.freedesktop.org/pipewire/pipewire-native-rs
  - Native Rust PipeWire implementation
  - Alternative to official bindings
  - Goal: Official PipeWire Rust API

## Further Reading

### Core API Documentation

- [Core API](https://pipewire.pages.freedesktop.org/wireplumber/c_api/core_api.html) - Core initialization and connection
- [Object Manager API](https://pipewire.pages.freedesktop.org/wireplumber/c_api/obj_manager_api.html) - Object watching and filtering
- [Plugin API](https://pipewire.pages.freedesktop.org/wireplumber/c_api/plugin_api.html) - Module loading and plugin development
- [Properties API](https://pipewire.pages.freedesktop.org/wireplumber/c_api/properties_api.html) - Key-value storage

### PipeWire Concepts

- [Native Protocol](https://docs.pipewire.org/devel/page_native_protocol.html) - Wire protocol details
- [SPA Plugins](https://docs.pipewire.org/page_spa_plugins.html) - Simple Plugin API
- [SPA POD](https://docs.pipewire.org/page_spa_pod.html) - Plain Old Data format

### Session Management

- [Session Item API](https://pipewire.pages.freedesktop.org/wireplumber/c_api/session_item_api.html) - Session objects
- [SI Interfaces](https://pipewire.pages.freedesktop.org/wireplumber/c_api/si_interfaces_api.html) - Session item interfaces

### Lua Scripting

- [Lua Introduction](https://pipewire.pages.freedesktop.org/wireplumber/lua_api/lua_introduction.html) - Using Lua with WirePlumber
- [Configuration](https://pipewire.pages.freedesktop.org/wireplumber/configuration/config_lua.html) - Lua-based configuration

## Implementation Details

### GIR-Based Binding Generation

Most of the WirePlumber Rust bindings are automatically generated from GObject Introspection (GIR) data:

- GIR configuration: [Gir.toml](.test-agent/wireplumber.rs/Gir.toml)
- Generated code: [src/auto/](.test-agent/wireplumber.rs/src/auto/)
- Manual overrides in module-specific files

This ensures bindings stay in sync with the C API while allowing Rust-specific improvements.

### Thread Safety

WirePlumber.rs is built on GLib which has its own thread model:
- Objects are generally not thread-safe for sharing between threads
- Use GLib's async mechanisms (MainContext, MainLoop) for cross-thread communication
- Signals and futures provide safe concurrency patterns

### Error Handling

```rust
use wireplumber::Error;

Result<T> // Alias for std::result::Result<T, Error>
```

Errors typically come from:
- Library errors (PipeWire/WirePlumber failures)
- GLib/GIO errors (I/O, D-Bus)
- State errors (invalid operations)

### Memory Management

As a GObject-based library:
- Reference counting via GLib's object system
- Rust's RAII integrates with GObject's lifecycle
- Explicit unrefing handled by bindings

## Key Properties and Constants

### Well-Known Property Keys

From [src/pw/keys.rs](.test-agent/wireplumber.rs/src/pw/keys.rs):

```rust
// Application identification
PW_KEY_APP_NAME = "application.name"
PW_KEY_APP_LANGUAGE = "application.language"
PW_KEY_APP_PROCESS_ID = "application.process.id"

// Audio properties
PW_KEY_AUDIO_RATE = "audio.rate"
PW_KEY_AUDIO_CHANNELS = "audio.channels"

// Port/Link properties
PW_KEY_PORT_DIRECTION = "port.direction"
PW_KEY_LINK_PASSIVE = "link.passive"
PW_KEY_OBJECT_LINGER = "object.linger"
```

### Node States

```rust
use wireplumber::pw::NodeState;

NodeState::Creating
NodeState::Suspended
NodeState::Idle
NodeState::Running
NodeState::Error
```

### Link States

```rust
use wireplumber::pw::LinkState;

LinkState::Unlinked
LinkState::Init
LinkState::Negotiating
LinkState::Allocating
LinkState::Paused
LinkState::Active
LinkState::Error
```

## Conclusion

WirePlumber.rs provides a comprehensive Rust interface to the WirePlumber session and policy manager. It excels at:

1. **High-level orchestration** of PipeWire audio/video graphs
2. **Policy implementation** through session items and Lua scripting
3. **State persistence** for configuration and routing
4. **Extensibility** through a dynamic plugin system
5. **Async/await** support for modern Rust applications

For lower-level PipeWire interaction or custom node implementation, consider [pipewire-native-rs](.test-agent/pipewire-native-rs/) instead.

The storage subsystem combines persistent `State` objects with runtime `Metadata` and object `Properties` to provide both configuration persistence and dynamic policy communication.

To explore further, study the examples in [examples/](.test-agent/wireplumber.rs/examples/) and the API documentation at https://arcnmx.github.io/wireplumber.rs/main/wireplumber/.
