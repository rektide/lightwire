# Lightwire Design Document

> Control smart-bulb brightness as virtual PipeWire node's volume

## Overview

Lightwire creates a virtual PipeWire audio sink node. When applications or the user adjusts the volume of this virtual node, Lightwire translates those volume changes to brightness commands for LIFX smart bulbs on the local network.

This enables controlling light brightness through any PipeWire-compatible volume interface (desktop mixers, media keys, application settings).

---

## Architecture

```
┌─────────────────┐     ┌────────────────────┐     ┌───────────────┐
│  PipeWire       │────▶│    Lightwire       │────▶│  LIFX Bulbs   │
│  Volume Control │     │  (virtual node)    │     │  (LAN UDP)    │
└─────────────────┘     └────────────────────┘     └───────────────┘
        │                        │                        │
   User adjusts          Monitors Props           Sends brightness
   volume 0-100%         parameter changes        commands 0-100%
```

### Components

1. **PipeWire Virtual Node** - A sink node registered with PipeWire that appears in volume mixers
2. **Volume Monitor** - Listens for Props parameter changes on the node
3. **LIFX Bridge** - Translates volume (0.0-1.0) to brightness and sends to bulbs
4. **Device Discovery** - Finds LIFX bulbs on the local network

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
- `Registry` for object enumeration
- `Node` proxy for subscribing to parameter changes (Props)

**Limitation:** pipewire-native-rs does not yet support creating server-side objects (nodes). We may need to:
1. Use `pw-loopback` or `pw-link` externally to create the virtual node
2. Wait for object creation support in pipewire-native-rs
3. Fall back to pipewire-rs (FFI bindings) for node creation only

### LIFX Control: `lifx-core`

**Crate:** `lifx-core` v0.4

Rationale:
- Local/LAN protocol only (no cloud dependency)
- Minimal dependencies (`byteorder`, `thiserror`)
- Full protocol coverage including brightness control
- No external server process required

Trade-off: We must implement UDP I/O ourselves, but this gives full control over discovery and command timing.

---

## Core Data Flow

### Startup Sequence

```
1. Initialize pipewire::init()
2. Create MainLoop with app properties
3. Connect to PipeWire server via Context
4. Get Registry, find or create virtual sink node
5. Subscribe to node's Props parameters
6. Start LIFX discovery (UDP broadcast to 255.255.255.255:56700)
7. Cache discovered bulbs
8. Run main loop
```

### Volume Change Handling

```
1. PipeWire emits NodeEvents::param with Props
2. Extract volume from Props POD structure
3. Clamp volume to 0.0..1.0 range
4. Convert to LIFX brightness (0-65535 u16)
5. Send SetColor/SetLightPower to all discovered bulbs
```

---

## Module Structure

```
lightwire/
├── Cargo.toml
├── src/
│   ├── main.rs           # CLI entry, config parsing
│   ├── lib.rs            # Public API
│   ├── pipewire/
│   │   ├── mod.rs
│   │   ├── node.rs       # Virtual node management
│   │   └── volume.rs     # Props parsing, volume extraction
│   ├── lifx/
│   │   ├── mod.rs
│   │   ├── discovery.rs  # UDP discovery protocol
│   │   ├── bulb.rs       # Bulb state and commands
│   │   └── protocol.rs   # Message building via lifx-core
│   └── bridge.rs         # Volume → Brightness mapping
└── lightwire-lifx/       # Provider crate (as mentioned in README)
    ├── Cargo.toml
    └── src/
        └── lib.rs
```

---

## Configuration

```toml
# ~/.config/lightwire/config.toml

[pipewire]
node_name = "Lightwire"           # Name shown in mixers
node_description = "Light Control"

[lifx]
discovery_timeout_ms = 5000       # How long to wait for bulb discovery
broadcast_address = "255.255.255.255"
port = 56700

[[targets]]
selector = "all"                  # or specific bulb labels/IDs

# Optional: per-bulb overrides
[[targets]]
label = "Bedroom"
min_brightness = 0.1              # Never go fully dark
max_brightness = 1.0
```

---

## Key Implementation Details

### Volume Extraction from Props

PipeWire Props parameters contain volume as `channelVolumes` array. Using `pipewire-native`:

```rust
node.add_listener(NodeEvents {
    param: some_closure!([bridge] seq, id, index, next, pod, {
        if id == spa::param::ParamType::Props {
            // Parse POD to extract channelVolumes
            // Average channels or use first channel
            let volume = extract_volume_from_pod(pod);
            bridge.set_brightness(volume);
        }
    }),
    ..Default::default()
});
```

### LIFX Brightness Command

Using `lifx-core` to build the message:

```rust
use lifx_core::Message;

fn set_brightness(bulb_addr: SocketAddr, brightness: f32) {
    let level = (brightness * 65535.0) as u16;
    let msg = Message::LightSetColor {
        reserved: 0,
        color: HSBK {
            hue: 0,           // Preserve existing
            saturation: 0,    // Preserve existing
            brightness: level,
            kelvin: 3500,     // Preserve existing
        },
        duration: 100,        // 100ms transition
    };
    // Send via UDP socket
}
```

### Discovery Protocol

```rust
// Broadcast GetService to find all bulbs
let msg = Message::GetService;
socket.send_to(&msg.pack(), "255.255.255.255:56700")?;

// Listen for StateService responses
loop {
    let (size, addr) = socket.recv_from(&mut buf)?;
    if let Ok(Message::StateService { port, .. }) = Message::unpack(&buf) {
        discovered_bulbs.insert(addr, BulbInfo { port, .. });
    }
}
```

---

## Open Questions

1. **Node Creation** - How to create virtual sink without object creation support in pipewire-native-rs?
   - Option A: Shell out to `pw-loopback --capture-props='...'`
   - Option B: Use pipewire-rs FFI for node creation, pipewire-native for monitoring
   - Option C: Contribute node creation to pipewire-native-rs

2. **Multiple Bulb Sync** - Should brightness commands be sent sequentially or in parallel?
   - Parallel is faster but may cause visible flicker if bulbs respond at different speeds

3. **Color Preservation** - When adjusting brightness, should we query current color first?
   - Simpler: Send SetLightPower (on/off) + brightness via SetWaveform
   - Better: Query GetColor first, modify only brightness field

4. **Mute Handling** - What happens when node is muted?
   - Option A: Set brightness to 0 (lights off)
   - Option B: Ignore mute, only respond to volume
   - Option C: Configurable behavior

---

## Implementation Phases

### Phase 1: LIFX Bridge (lightwire-lifx)
- Implement discovery using lifx-core
- Implement brightness control
- CLI tool for testing: `lightwire-lifx set-brightness 0.5`

### Phase 2: PipeWire Integration
- Connect to PipeWire, enumerate nodes
- Subscribe to Props changes on existing nodes
- Map volume changes to LIFX brightness

### Phase 3: Virtual Node
- Create virtual sink node (method TBD based on pipewire-native-rs capabilities)
- Register with descriptive name for mixer display

### Phase 4: Configuration & Polish
- TOML configuration file support
- Multiple bulb targeting
- Systemd service file
- Error recovery (reconnect on PipeWire restart, re-discover bulbs)

---

## Dependencies

```toml
[dependencies]
pipewire-native = "0.1"      # PipeWire client
lifx-core = "0.4"            # LIFX protocol
tokio = { version = "1", features = ["net", "rt-multi-thread"] }  # Async UDP
figment2 = "0.4"             # Config
clap = { version = "4", features = ["derive"] }  # CLI
jiff = "0.1"                 # Time handling
tracing = "0.1"              # Logging
```

---

## Success Criteria

1. Virtual node appears in `pavucontrol`, GNOME Settings, etc.
2. Adjusting volume slider changes LIFX bulb brightness smoothly
3. Works entirely on LAN (no cloud/internet required)
4. Survives PipeWire restarts via reconnection logic
5. Sub-100ms latency from volume change to brightness change
