# PipeWire Native Rust Bindings Discovery

## Technology Stack Overview

**PipeWire** is a low-latency, graph-based audio and video processing server that aims to replace PulseAudio and JACK. It provides a modern infrastructure for handling multimedia streams on Linux systems.

The `pipewire-native-rs` crate is a native Rust implementation of the PipeWire client library, providing:

### Core Technologies

- **PipeWire Native Protocol** - A custom binary protocol for client-server communication over Unix domain sockets
- **SPA (Simple Plugin Architecture)** - PipeWire's plugin system providing low-level primitives for logging, event loops, system calls, CPU detection, and data serialization
- **POD (Plain Old Data)** - Binary serialization format used for structured data exchange

### Repository
- **Canonical GitLab**: https://gitlab.freedesktop.org/pipewire/pipewire-native-rs
- **Documentation**: https://docs.rs/pipewire-native/latest/pipewire_native/

---

## Core Architecture

### Initialization

The library must be initialized before use to set up logging and support libraries:

```rust
pipewire::init();
```

This loads SPA plugins for:
- Logging (controlled via `PIPEWIRE_DEBUG` and `PIPEWIRE_LOG*` environment variables)
- CPU feature detection (via `PIPEWIRE_CPU` and `PIPEWIRE_VM`)
- System interfaces (system calls, file descriptors)

Source: `pipewire/src/lib.rs:109-189`

### MainLoop

The `MainLoop` provides an event loop for asynchronous communication with the PipeWire server.

**Source**: `pipewire/src/main_loop.rs:64-82`

Key features:
- Event-driven I/O handling via Unix socket
- Timer events
- Signal handling
- Idle callbacks
- Thread-safe locking primitives

```rust
let mut props = Properties::new();
props.set(keys::APP_NAME, "my-app".to_string());
let main_loop = MainLoop::new(&props)?;
```

### ThreadLoop

A `ThreadLoop` variant that runs the main loop in a separate thread, suitable for GUI applications or other scenarios where the main thread shouldn't block.

**Source**: `pipewire/src/thread_loop.rs:18-25`

```rust
let thread_loop = ThreadLoop::new(&props)?;
// Access underlying main_loop for setup
let main_loop_ref = thread_loop.main_loop();
// Run in background thread
thread_loop.run();
```

---

## Context and Core

### Context

The `Context` is the top-level entry point containing client configuration and the main event loop.

**Source**: `pipewire/src/context.rs:26-42`

```rust
let context = Context::new(main_loop, props)?;
let core = context.connect(None)?;
```

The context loads client configuration from standard PipeWire config directories and provides:
- Properties (application name, process ID, etc.)
- Main loop reference
- Protocol handlers

### Core

The `Core` represents the connection to the PipeWire server and is a singleton for each client.

**Source**: `pipewire/src/core.rs:44-55`

Key responsibilities:
- Managing the connection lifecycle
- Tracking local and global object IDs
- Providing access to the Registry for object enumeration
- Creating new server-side objects (future feature)

---

## Proxy System

The proxy system is the primary API for interacting with PipeWire objects. All server-side objects (Nodes, Ports, Links, Devices, etc.) are represented as proxies.

### Proxy Concept

**Source**: `pipewire/src/proxy/mod.rs:34-71`

Each proxy has:
- A **local ID** - client's view of the object
- A **global ID** - server's view of the object
- Methods that can be invoked
- Events that can be subscribed to

### Proxy Types

#### Core (`PipeWire:Interface:Core`)
**Source**: `pipewire/src/core.rs:44-55`

Top-level singleton representing the server connection.

#### Registry (`PipeWire:Interface:Registry`)
**Source**: `pipewire/src/proxy/registry.rs:18-27`

Allows enumeration and binding to global objects.

```rust
let registry = core.registry()?;

registry.add_listener(RegistryEvents {
    global: some_closure!([registry] id, perms, type_, version, props, {
        // New global object appeared
        let object = registry.bind(id, type_, version)?;
        // Handle the object...
    }),
    global_remove: some_closure!([registry] id, {
        // Object was removed
    }),
    ..Default::default()
});
```

#### Client (`PipeWire:Interface:Client`)
**Source**: `pipewire/src/proxy/client.rs`

Represents other clients connected to the PipeWire server.

#### Node (`PipeWire:Interface:Node`)
**Source**: `pipewire/src/proxy/node.rs:22-29`

Represents processing nodes in the audio/video graph.

Key features:
- State management (Error, Creating, Suspended, Idle, Running)
- Port management (input/output)
- Parameter subscription and enumeration
- Command sending

```rust
node.add_listener(NodeEvents {
    info: some_closure!([^mut_app] info, {
        match info.state {
            NodeState::Running => println!("Node is running"),
            NodeState::Error => eprintln!("Node error: {:?}", info.error),
            _ => {}
        }
    }),
    param: some_closure!([^mut_app] seq, id, index, next, pod, {
        // Parameter changed
    }),
    ..Default::default()
});
```

Node states: `pipewire/src/proxy/node.rs:82-94`

#### Port (`PipeWire:Interface:Port`)
**Source**: `pipewire/src/proxy/port.rs:22-29`

Represents input or output ports on Nodes.

Port directions: `pipewire/src/proxy/port.rs:62-67`
- `Input` - Receives data
- `Output` - Sends data

#### Link (`PipeWire:Interface:Link`)
**Source**: `pipewire/src/proxy/link.rs:19-25`

Represents connections between ports.

Link states: `pipewire/src/proxy/link.rs:39-55`
- `Error`, `Unlinked`, `Init`, `Negotiation`, `Allocating`, `Paused`, `Active`

#### Device (`PipeWire:Interface:Device`)
**Source**: `pipewire/src/proxy/device.rs`

Represents hardware devices.

#### Module (`PipeWire:Interface:Module`)
**Source**: `pipewire/src/proxy/module.rs`

Represents loaded modules on the server.

#### Factory (`PipeWire:Interface:Factory`)
**Source**: `pipewire/src/proxy/factory.rs`

Used to create new objects on the server.

#### Metadata (`PipeWire:Interface:Metadata`)
**Source**: `pipewire/src/proxy/metadata.rs`

Provides metadata properties about objects.

---

## Protocol Layer

### Native Protocol Implementation

The library implements the PipeWire native protocol in pure Rust, providing:

**Source**: `pipewire/src/protocol/mod.rs`

- Message marshalling and unmarshalling
- Connection management over Unix domain sockets
- Asynchronous operation sequencing
- File descriptor passing

### Connection

**Source**: `pipewire/src/protocol/connection.rs`

Handles the socket connection with:
- Bidirectional message buffering
- Sequence tracking for async operations
- Generation tracking for message ordering

### Message Types

**Source**: `pipewire/src/protocol/marshal/`

The protocol supports messages for each interface type:
- Core (client methods like hello, sync, create_object)
- Registry (bind, destroy globals)
- Node (subscribe_params, enum_params, set_param, send_command)
- Port (subscribe_params, enum_params)
- Link, Device, Module, Factory, Client, Metadata

---

## SPA Layer

### POD (Plain Old Data)

Binary serialization format for structured data.

**Source**: `spa/src/pod/mod.rs:27-43`

The `Pod` trait defines encode/decode operations:

```rust
pub trait Pod {
    type DecodesTo;
    fn encode(&self, data: &mut [u8]) -> Result<usize, Error>;
    fn decode(data: &[u8]) -> Result<(Self::DecodesTo, usize), Error>;
}
```

POD types support:
- Primitive types (bool, int, float, string, fd, etc.)
- Arrays
- Structs (via derive macro)
- Choice values (ranges, enums, flags)

### Parameters

Parameter objects describe configurable properties of nodes and ports.

**Source**: `spa/src/param/mod.rs`

Common parameter types:
- `PropInfo` - Property information
- `Props` - Property values
- `Format` - Audio/video format
- `Buffers` - Buffer configuration
- `Latency` - Latency settings
- `Rate` - Sample rate
- `Profile` - Device profiles

**Source**: `spa/src/param/`

### Interfaces

The SPA interface layer wraps C plugins for:

**Source**: `spa/src/interface/`

- **Loop** (`loop.rs`) - Event loop implementation
- **System** (`system.rs`) - System call abstraction
- **CPU** (`cpu.rs`) - CPU feature detection
- **Log** (`log.rs`) - Logging infrastructure
- **Plugin** (`plugin.rs`) - Plugin loading

---

## Properties and Keys

### Properties

Key-value pairs used throughout the API for object configuration.

**Source**: `pipewire/src/properties.rs`

```rust
let mut props = Properties::new();
props.set(keys::APP_NAME, "My App".to_string());
props.set(keys::REMOTE_NAME, "pipewire-0".to_string());

let value: String = props.lookup(keys::APP_NAME).to_string();
```

### Well-Known Keys

**Source**: `pipewire/src/keys.rs`

Application keys:
- `application.name` - Application name
- `application.id` - Application ID (e.g., org.gnome.Rhythmbox)
- `application.version` - Application version
- `application.icon` - Base64 encoded icon
- `application.language` - Locale
- `application.process.id` - Process ID
- `application.process.binary` - Binary name
- `application.process.user` - Username
- `application.process.host` - Hostname

Connection keys:
- `remote.name` - PipeWire remote to connect to (default: `pipewire-0`)
- `remote.intention` - Connection intent (generic, screencast, internal)

Object keys:
- `core.name`, `core.version`, `core.daemon`, `core.id`
- `link.input.node`, `link.output.node`, `link.input.port`, `link.output.port`
- `node.name`, `node.description`, `node.latency`
- `port.name`, `port.direction`, `port.physical`, `port.terminal`

---

## Event Handling and Closures

### The `closure!` Macro

Reduces boilerplate for event handlers by managing `Refcounted` and `Clone` captures automatically.

**Source**: `pipewire/src/lib.rs:191-252`

```rust
// Capture with Clone (^ marker)
registry.add_listener(RegistryEvents {
    global: some_closure!([registry ^(app)] id, type_, version, props, {
        // app is cloned, registry is weak reference
        // ...
    }),
    ..Default::default()
});

// Capture with Clone and mutable (^mut marker)
node.add_listener(NodeEvents {
    info: some_closure!([^mut_app_state] info, {
        // app_state is available as mut
        app_state.update(info);
    }),
    ..Default::default()
});

// Capture with weak reference (no marker)
node.add_listener(NodeEvents {
    info: some_closure!([registry] info, {
        // registry is automatically weak-referenced
        if let Some(registry) = registry.upgrade() {
            // ...
        }
    }),
    ..Default::default()
});
```

### Hook System

Event listeners are managed via the hook system from SPA.

**Source**: `spa/src/hook.rs`

```rust
let hook_id = object.add_listener(Events { ... });
// Later:
object.remove_listener(hook_id);
```

---

## Logging

The library integrates with PipeWire's logging system.

**Source**: `pipewire/src/log.rs`

Environment variables:
- `PIPEWIRE_DEBUG` - Log level (e.g., "3", or "module.name=5")
- `PIPEWIRE_LOG` - Log file path
- `PIPEWIRE_LOG_COLOR` - Enable/disable colored output
- `PIPEWIRE_LOG_TIMESTAMP` - Enable/disable timestamps
- `PIPEWIRE_LOG_LINE` - Enable/disable line numbers

Log topics defined: `pipewire/src/lib.rs` uses `default_topic!` macro

---

## Example: Simple Client

This example shows the basic lifecycle:

```rust
use pipewire_native as pipewire;
use pipewire_native::properties::Properties;
use pipewire_native::keys;

fn main() -> std::io::Result<()> {
    // Initialize library
    pipewire::init();

    // Create properties
    let mut props = Properties::new();
    props.set(keys::APP_NAME, "simple-client".to_string());

    // Create main loop
    let main_loop = pipewire::MainLoop::new(&props)
        .expect("Failed to create main loop");

    // Create context
    let context = pipewire::Context::new(&main_loop, props)?;

    // Connect to server
    let core = context.connect(None)?;

    // Get registry
    let registry = core.registry()?;

    // Listen for objects
    use pipewire::some_closure;
    registry.add_listener(pipewire::proxy::registry::RegistryEvents {
        global: some_closure!([registry] id, _perms, type_, _version, _props, {
            println!("New object: {} ({})", id, type_);
        }),
        ..Default::default()
    });

    // Run main loop
    main_loop.run();

    Ok(())
}
```

---

## Example: Threaded Client (from pw-browse)

The `pw-browse` tool demonstrates a more complete usage pattern.

**Source**: `tools/browse/main.rs`, `tools/browse/pw.rs`

Key patterns:
- Using `ThreadLoop` to run PipeWire in background thread
- Maintaining application state with `Arc<Mutex<>>`
- Using `ThreadLoop::lock()` for safe cross-thread access
- Binding to multiple object types and handling their events

```rust
let thread_loop = ThreadLoop::new(&props)?;
let context = Context::new(thread_loop.main_loop(), props)?;
let core = context.connect(None)?;
let registry = core.registry()?;

// Store state in Arc<Mutex<>> for shared access
let state = Arc::new(Mutex::new(AppState::new()));

// Run in background
thread_loop.run();

// From UI thread, safely access PipeWire objects:
{
    let lock = thread_loop.lock();
    // Safe to access proxies while locked
    let objects = state.lock().unwrap().get_objects();
}
// Lock automatically released
```

**Source**: `tools/browse/pw.rs:163-194`

---

## Current Limitations

As stated in the README (`README.md:8-10`), the library is a work-in-progress:

- **Audio/video streaming** - Not yet implemented
- **Buffer management** - Not yet implemented
- **Object creation** - Core methods for creating server objects are not yet implemented
- **API stability** - The API is expected to change as the project matures

---

## Code Organization

### Workspace Structure

**Source**: `Cargo.toml:1-4`

```toml
[workspace]
members = ["macros", "pipewire", "spa", "tools"]
```

- `macros/` - Procedural macros (EnumU32, PodStruct derive)
- `pipewire/` - Main library crate (Context, Core, Proxies)
- `spa/` - SPA low-level primitives (POD, interfaces, params)
- `tools/` - Example tools (pw-browse TUI)

### Key Modules

**pipewire/src/**:
- `lib.rs` - Public API entry point
- `context.rs` - Context implementation
- `core.rs` - Core implementation
- `main_loop.rs` - Event loop
- `thread_loop.rs` - Threaded event loop
- `proxy/mod.rs` - Proxy system and trait
- `proxy/*.rs` - Individual proxy implementations
- `properties.rs` - Property key-value store
- `keys.rs` - Well-known property keys
- `protocol/mod.rs` - Protocol handling
- `protocol/connection.rs` - Socket connection
- `protocol/marshal/*.rs` - Message marshalling
- `log.rs` - Logging integration

**spa/src/**:
- `pod/` - POD serialization (builder, parser, types)
- `param/` - Parameter types (buffers, format, props, etc.)
- `interface/` - SPA interface wrappers (loop, system, cpu, log)
- `dict.rs` - Dictionary type
- `hook.rs` - Hook/listener system
- `flags.rs` - Bitflag utilities

---

## Related Projects

- **PipeWire**: https://gitlab.freedesktop.org/pipewire/pipewire
- **PipeWire Rust Bindings (FFI)**: https://gitlab.freedesktop.org/pipewire/pipewire-rs
- **PipeWire Native Protocol (reference)**: https://docs.pipewire.org/devel/page_native_protocol.html

---

## Closing

The `pipewire-native-rs` library provides a safe, idiomatic Rust API for interacting with PipeWire, with a native implementation of the protocol and hybrid SPA layer. While still a work-in-progress (particularly for audio/video streaming), it already provides a robust foundation for client applications that need to enumerate and monitor PipeWire objects.

The proxy system and event-driven architecture make it straightforward to build reactive applications that respond to changes in the audio/video graph in real-time.
