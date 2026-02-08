# Rust Libraries for LIFX Control

This document provides an overview of available Rust libraries for controlling LIFX smart lights, with guidance on local vs remote control and recommendations for selecting the right library.

## Overview

LIFX devices can be controlled in two ways:
1. **Remote/Cloud API** - Via LIFX's official HTTP API (requires internet access and API token)
2. **Local/LAN Protocol** - Direct communication with lights on your local network (UDP packets, no internet required)

Most Rust libraries fall into these categories:
- High-level API wrappers (remote only)
- Low-level protocol libraries (local only)
- Hybrid solutions (both)

---

## Available Libraries

### 1. lifx-rs

**Crate:** `lifx-rs`  
**Repository:** https://github.com/PixelCoda/lifx-rs  
**Documentation:** https://docs.rs/lifx-rs  
**Version:** 0.1.30

**Communication Type:** Hybrid (supports both)

**Features:**
- Synchronous and asynchronous APIs
- **Remote API Support:** Full support for official LIFX HTTP API
  - List Lights, Set State, Set States, State Delta, Toggle Power
  - Effects: Breathe, Move, Morph, Flame, Pulse, Effects Off
  - Clean (HEV), List Scenes, Validate Color
- **Local API Support:** Basic offline API (via lifx-api-server)
  - List Lights, Set State, Set States
- Falls back from primary to secondary API endpoints

**Dependencies:**
- `reqwest` (HTTP client)
- `serde`, `serde_json`
- `trust-dns-resolver`

**Pros:**
- Most feature-rich option
- Dual communication modes (can fall back to local if cloud is down)
- Well-documented
- Both sync and async support

**Cons:**
- Larger dependency footprint (includes HTTP client)
- Local control requires running separate lifx-api-server process

**Best For:**
- Applications that need maximum flexibility
- Projects that need cloud features (scenes, effects) but want offline fallback
- Complex integrations

**Example:**
```rust
use lifx_rs as lifx;

fn main() {
    let config = lifx::LifxConfig {
        access_token: "your-token".to_string(),
        api_endpoints: vec![
            "https://api.lifx.com".to_string(),     // Cloud
            "http://localhost:8089".to_string(),    // Local fallback
        ],
    };

    let lights = lifx::Light::list_all(config).unwrap();
    for light in lights {
        println!("Found light: {}", light.label);
    }
}
```

---

### 2. lifx-core

**Crate:** `lifx-core`  
**Repository:** https://github.com/eminence/lifx  
**Documentation:** https://docs.rs/lifx-core  
**Version:** 0.4.0

**Communication Type:** Local/LAN only

**Features:**
- Low-level LIFX LAN protocol implementation
- Full message types and structures for protocol packets
- Supports all LIFX product types:
  - Standard light bulbs
  - Multi-zone devices (LIFX Z, Beam)
  - Relay devices (LIFX Switch)
  - Tile devices
- Discovery protocol implementation
- Product info lookups

**What it does NOT do:**
- Network I/O (you must handle UDP communication)
- State caching
- Request/response handling
- Any HTTP/cloud functionality

**Dependencies:**
- `byteorder`
- `thiserror` (minimal)

**Pros:**
- Minimal dependencies
- Full protocol coverage
- No external dependencies
- Zero overhead
- Actively maintained

**Cons:**
- Requires you to implement network communication
- More boilerplate code required
- Steeper learning curve
- No async support built-in

**Best For:**
- Embedded systems or constrained environments
- Applications that need direct UDP control
- Custom protocol implementations
- Learning the LIFX protocol

**Example:**
```rust
use lifx_core::{Message, FrameAddress, Frame};

// Build a GetService message for discovery
let message = Message::GetService;
let raw = message.build();

// You must send this via UDP to 255.255.255.255:56700
// and handle responses yourself
```

---

### 3. lifxi

**Crate:** `lifxi`  
**Repository:** https://github.com/Aehmlo/lifxi  
**Documentation:** https://docs.rs/lifxi

**Communication Type:** Remote/Cloud only

**Features:**
- Clean, idiomatic Rust API
- Builder pattern for state changes
- Currently only supports HTTP API (local support planned but not implemented)
- Connection pooling via Client

**Dependencies:**
- HTTP client (likely `reqwest` or similar)

**Pros:**
- Clean API design
- Good documentation
- Type-safe state changes

**Cons:**
- Local/LAN support not yet implemented (as of documentation)
- Less active than other libraries (fewer stars)
- No async support shown in docs

**Best For:**
- Projects that only need cloud API access
- Simple, clean API wrappers are preferred
- Waiting for future LAN support

**Example:**
```rust
use lifxi::http::*;

fn main() {
    let client = Client::new("your-secret");
    let _result = client
        .select(Selector::All)
        .set_state()
        .power(true)
        .color(Color::Red)
        .brightness(0.4)
        .send();
}
```

---

### 4. lifx-api-server

**Crate:** `lifx-api-server`  
**Repository:** https://github.com/ktheindifferent/lifx-api-server  
**Documentation:** https://docs.rs/lifx-api-server  
**Version:** 0.1.15

**Communication Type:** Local/LAN only (exposes HTTP server)

**Features:**
- Standalone HTTP server that mimics official LIFX API
- Uses LAN protocol to communicate with lights
- Exposes compatible HTTP endpoints locally
- **Supported Methods:** List Lights, Set State, Set States
- Optional authentication
- Docker support

**Usage Pattern:**
This is typically used alongside `lifx-rs` to provide offline/local HTTP endpoints that mirror the official cloud API.

**Pros:**
- Enables local-only control with HTTP simplicity
- Works with existing LIFX HTTP API clients
- Self-contained server
- No internet required

**Cons:**
- Must run as separate process/service
- Limited functionality compared to cloud API
- Only supports basic operations (no scenes, effects, etc.)

**Best For:**
- Home automation servers
- Local-only setups
- Bridge between HTTP clients and local protocol
- Using `lifx-rs` in offline mode

**Example:**
```rust
use lifx_api_server;

fn main() {
    let config = lifx_api_server::Config {
        secret_key: Some("your-secret".to_string()),
        port: 8089,
        auth_required: true,
    };

    lifx_api_server::start(config);
    // Server is now running at http://localhost:8089
}
```

---

## Decision Guide

### Choose lifx-rs if:

1. **You need maximum flexibility** - Supports both cloud and local communication
2. **You want cloud features** - Effects, scenes, and other advanced LIFX features
3. **You want a fallback strategy** - Can fall back to local if cloud is unavailable
4. **You want both sync and async** - Provides both APIs
5. **You're building a general-purpose application**

### Choose lifx-core if:

1. **You need local/LAN control only** - Direct UDP protocol implementation
2. **You want minimal dependencies** - Very small footprint
3. **You're building an embedded system** - Or have constrained resources
4. **You want full control over network I/O** - Implement your own UDP handling
5. **You're learning or debugging the LIFX protocol**

### Choose lifxi if:

1. **You only need cloud API access** - Don't care about local control
2. **You prefer a clean, builder-style API** - Very idiomatic Rust
3. **You can wait for LAN support** - Planned for future releases
4. **You want a simple HTTP wrapper** - No need for low-level protocol access

### Choose lifx-api-server if:

1. **You need a local HTTP bridge** - Expose local protocol as HTTP
2. **You're using lifx-rs offline** - This is the companion server
3. **You have HTTP-only clients** - Need to communicate with local lights
4. **You're building a home automation hub** - Run one server for all clients

---

## Quick Reference Table

| Library | Remote/Cloud | Local/LAN | Protocol Level | Async | Dependencies | Stars |
|---------|-------------|-----------|----------------|-------|-------------|-------|
| lifx-rs | ✅ | ✅ (via server) | High | ✅ | Moderate (reqwest) | - |
| lifx-core | ❌ | ✅ | Low | ❌ | Minimal | 22 |
| lifxi | ✅ | ❌ | High | ❌ | Moderate | 3 |
| lifx-api-server | ❌ | ✅ | High (HTTP) | ❌ | Moderate | 2 |

---

## Network Requirements

### Remote/Cloud Control
- **Requires:** Internet connection, LIFX account, API token
- **Get token:** https://cloud.lifx.com/settings
- **Endpoint:** https://api.lifx.com
- **Advantages:** Full feature set, works from anywhere
- **Disadvantages:** Dependent on cloud service, requires auth

### Local/LAN Control
- **Requires:** Devices on same network, UDP broadcast capability
- **Port:** 56700 (default LIFX protocol port)
- **Discovery:** Send GetService to 255.255.255.255:56700
- **Advantages:** No internet required, faster, no auth
- **Disadvantages:** Limited features, more complex implementation

---

## Getting Started Checklist

### For Cloud Control with lifx-rs:
1. Create LIFX account at cloud.lifx.com
2. Generate API token in settings
3. Add `lifx-rs = "0.1"` to Cargo.toml
4. Use the token in `LifxConfig`

### For Local Control with lifx-core:
1. Ensure LIFX devices are on your network
2. Add `lifx-core = "0.4"` to Cargo.toml
3. Implement UDP discovery on port 56700
4. Send `Message::GetService` for discovery
5. Parse responses and send control messages

### For Hybrid Control:
1. Set up `lifx-api-server` locally (or on network)
2. Configure `lifx-rs` with both endpoints
3. Primary: `https://api.lifx.com`
4. Fallback: `http://localhost:8089` (or your server address)

---

## Additional Resources

- [LIFX HTTP API Documentation](https://api.developer.lifx.com)
- [LIFX LAN Protocol Documentation](https://lan.developer.lifx.com/)
- [LIFX Developer Terms](https://www.lifx.com/pages/developer-terms-of-use)

---

## Notes

- All libraries are dual-licensed under MIT or Apache-2.0
- Commercial use may be subject to LIFX Developer Terms
- Check library repository for latest versions and maintenance status
- Some libraries may have untested device types (report issues to maintainers)
