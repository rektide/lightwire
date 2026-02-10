use anyhow::Result;
use tokio::sync::mpsc;

#[derive(Clone, Debug)]
pub struct VolumeEvent {
    pub node_name: String,
    pub volume: f32,
    pub muted: bool,
}

#[allow(dead_code)]
pub struct VolumeMonitor {
    node_names: Vec<String>,
    event_tx: mpsc::UnboundedSender<VolumeEvent>,
}

impl VolumeMonitor {
    pub fn new(node_names: Vec<String>) -> (Self, mpsc::UnboundedReceiver<VolumeEvent>) {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        (
            Self { node_names, event_tx },
            event_rx,
        )
    }

    pub async fn run(self) -> Result<()> {
        Ok(())
    }
}
