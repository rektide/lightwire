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

## CLI Design

```
lightwire <command>

Commands:
  scan        Discover lights on the network
  sync        Create/update PipeWire configs for discovered lights
  remove      Remove a light's PipeWire config
  list        List managed lights and their node status
  daemon      Run the volume→brightness bridge daemon
  reload      Trigger PipeWire to reload configuration

Options:
  --provider <name>   Light provider (default: lifx)
  --config-dir <path> PipeWire config directory
                      (default: ~/.config/pipewire/pipewire.conf.d)
```

### Example Workflow

```bash
# Discover bulbs
$ lightwire scan
Found 3 LIFX bulbs:
  - Bedroom (d073d5xxxxxx)
  - Living Room (d073d5yyyyyy)
  - Desk Lamp (d073d5zzzzzz)

# Generate PipeWire configs
$ lightwire sync
Created: ~/.config/pipewire/pipewire.conf.d/lightwire-lifx-bedroom.conf
Created: ~/.config/pipewire/pipewire.conf.d/lightwire-lifx-living-room.conf
Created: ~/.config/pipewire/pipewire.conf.d/lightwire-lifx-desk-lamp.conf

# Reload PipeWire
$ lightwire reload
Reloading PipeWire...

# Run daemon (or as systemd service)
$ lightwire daemon
Monitoring nodes: lightwire.lifx.bedroom, lightwire.lifx.living-room, lightwire.lifx.desk-lamp
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
│   ├── main.rs           # CLI entry (clap)
│   ├── lib.rs            # Public API
│   ├── cli/
│   │   ├── mod.rs
│   │   ├── scan.rs       # Discovery command
│   │   ├── sync.rs       # Config generation
│   │   ├── daemon.rs     # Bridge daemon
│   │   └── reload.rs     # PipeWire reload
│   ├── config/
│   │   ├── mod.rs
│   │   ├── dropin.rs     # Drop-in file generation/parsing
│   │   └── settings.rs   # User config (figment)
│   ├── pipewire/
│   │   ├── mod.rs
│   │   ├── monitor.rs    # Node enumeration and binding
│   │   └── volume.rs     # Props parsing, volume extraction
│   ├── providers/
│   │   ├── mod.rs        # Provider trait
│   │   └── lifx/
│   │       ├── mod.rs
│   │       ├── discovery.rs
│   │       ├── bulb.rs
│   │       └── protocol.rs
│   └── bridge.rs         # Volume → Brightness mapping
└── lightwire-lifx/       # Standalone LIFX crate (optional)
    ├── Cargo.toml
    └── src/
        └── lib.rs
```

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

### Phase 1: LIFX Discovery & Control
- Implement discovery using lifx-core
- Implement brightness control
- CLI: `lightwire scan`, basic brightness test

### Phase 2: Config Generation
- Drop-in file generation/removal
- CLI: `lightwire sync`, `lightwire remove`, `lightwire list`
- PipeWire reload helper

### Phase 3: PipeWire Monitoring
- Connect to PipeWire via pipewire-native
- Enumerate nodes, bind to managed ones
- Subscribe to Props changes
- CLI: `lightwire daemon`

### Phase 4: Bridge & Polish
- Volume→brightness mapping with per-light config
- Systemd service file for daemon
- Error recovery (reconnect on PipeWire restart)
- Documentation

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
