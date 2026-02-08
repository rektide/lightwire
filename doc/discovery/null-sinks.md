# PipeWire Null Nodes Investigation

## Findings

### Available Null Sinks

**module-null-sink** creates virtual audio sinks that:
- Accept audio input without outputting to hardware
- Provide volume control properties
- Expose monitor ports for observing the audio
- Can be monitored for volume changes

### Creating a Null Sink

```bash
pactl load-module module-null-sink \
  sink_name=lightwire \
  sink_properties="node.description='Lightwire Virtual Sink'"
```

**Output**: Returns module ID that can be used to unload later

**Key Properties**:
- `node.name = "lightwire"` - Node identifier
- `node.description = "Lightwire"` - Display name
- `media.class = "Audio/Sink"` - Node type
- `factory.name = "support.null-audio-sink"` - Factory type
- `monitor.channel-volumes = "true"` - Enables volume monitoring
- `monitor.passthrough = "true"` - Passes audio through to monitor

### Node Structure

```
Node ID: 243
  Ports:
    - playback_FL (input)
    - playback_FR (input)
    - monitor_FL (output)
    - monitor_FR (output)
  Properties:
    - audio.channels = "2"
    - audio.position = "[ FL, FR ]"
```

### Volume Control

Volume is controlled via standard PipeWire commands:
```bash
# Set volume
pactl set-sink-volume lightwire 50%

# Get current volume
pactl list sinks | grep -A15 "Name: lightwire"
```

Volume is represented as:
- Linear: 0 to 65536 (0-100%)
- Decibels: -∞ to 0 dB
- Percentage: 0% to 100%

### Using Existing Combine Sink

The system already has a virtual sink called **combine_sink**:

**Properties**:
- `node.name = "combine_sink"`
- `node.description = "Combine Sink"`
- `node.virtual = "true"` - Indicates virtual sink
- `node.group = "combine-sink-1289-30"` - Group identifier
- `node.link-group = "combine-sink-1289-30"` - Links nodes in group

This appears to be created by `module-combine-sink` or similar, and could potentially be used instead of creating a new null sink.

### Monitoring Volume Changes via pipewire-native-rs

To monitor volume changes:

```rust
use pipewire_native as pipewire;

// Bind to the node
let registry = core.registry()?;
let node = registry.bind(node_id, "PipeWire:Interface:Node", 3)?;

// Subscribe to param changes
node.subscribe_params(vec!["Props", "Route"])?;

// Listen for param events
node.add_listener(NodeEvents {
    param: some_closure!([node] seq, id, index, next, pod, {
        // Parse Props param to get volume
        if let Ok(props) = pod.parse::<Props>() {
            if let Some(volume) = props.get("channelVolumes") {
                let volumes: Vec<f32> = volume.try_into()?;
                // volumes[0] = left channel, volumes[1] = right channel
            }
        }
    }),
    ..Default::default()
});
```

### Alternative: Monitor via Audio Data

Instead of monitoring volume properties, we can:
1. Link to the monitor ports (monitor_FL, monitor_FR)
2. Create a stream that captures audio data
3. Calculate RMS or peak volume from audio samples

This gives real-time audio visualization but requires more processing.

### Recommendations for Lightwire

**Option 1: Create Dedicated Null Sink**
```bash
pactl load-module module-null-sink sink_name=lightwire
```
- **Pros**: Clean separation, dedicated to lightwire
- **Cons**: Requires module loading at startup

**Option 2: Use Existing combine_sink**
- **Pros**: Already exists, no setup needed
- **Cons**: Shared with other purposes, may have conflicting uses

**Option 3: Create Sink on Demand**
- Check if lightwire sink exists
- Create if missing
- Use `sink_name` config option for flexibility

### Managing the Sink Module

**Loading**:
```rust
use std::process::Command;

fn create_lightwire_sink() -> Result<u32, Error> {
    let output = Command::new("pactl")
        .args(["load-module", "module-null-sink", 
              "sink_name=lightwire",
              "sink_properties=device.description='Lightwire'"])
        .output()?;
    
    let module_id = String::from_utf8(output.stdout)?
        .trim()
        .parse::<u32>()?;
    
    Ok(module_id)
}
```

**Unloading**:
```bash
pactl unload-module <module_id>
```

### Configuring Lightwire to Use the Sink

Add to config:
```toml
[lightwire.pipewire]
sink_name = "lightwire"           # or "combine_sink" to use existing
auto_create_sink = true           # Create if missing
```

### Testing Volume Monitoring

```rust
// Quick test: monitor volume changes
pactl set-sink-volume lightwire 0%   # bulbs off
pactl set-sink-volume lightwire 50%  # bulbs at 50%
pactl set-sink-volume lightwire 100% # bulbs at 100%
```

### Potential Issues

1. **Module ID Persistence**: Module IDs change on restart, need to track by name
2. **Auto-start**: Need to create sink at application startup
3. **Cleanup**: Should unload module when application exits
4. **Permissions**: May need audio group membership
5. **PipeWire Daemon**: Requires PipeWire to be running

### Integration with pipewire-native-rs

Since pipewire-native-rs doesn't support creating nodes yet, we:
1. Use `pactl` to create the null sink
2. Use pipewire-native-rs to connect and monitor
3. Monitor the sink's Props parameters for volume changes

Future enhancement: When pipewire-native-rs supports node creation, we can:
- Use the factory system directly
- Avoid spawning external processes
- Have tighter integration

---

## Summary

The recommended approach for lightwire:

1. **Use module-null-sink** to create a dedicated virtual sink named "lightwire"
2. **Monitor Props parameters** via pipewire-native-rs for volume changes
3. **Map volume to brightness** using the configured mapping mode
4. **Implement hysteresis** to prevent flickering from small changes
5. **Auto-create sink** at startup if missing
6. **Clean up** by unloading the module on exit

Example sink creation command:
```bash
pactl load-module module-null-sink \
  sink_name=lightwire \
  sink_properties="device.description='Lightwire',monitor.channel-volumes=true"
```
