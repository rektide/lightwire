use anyhow::Result;

#[derive(Clone, Debug)]
pub struct Volume {
    pub value: f32,
    pub muted: bool,
}

impl Volume {
    pub fn new(value: f32) -> Self {
        Self { value: value.clamp(0.0, 1.0), muted: false }
    }

    pub fn muted(value: f32) -> Self {
        Self { value: value.clamp(0.0, 1.0), muted: true }
    }

    pub fn is_muted(&self) -> bool {
        self.muted
    }

    pub fn as_f32(&self) -> f32 {
        self.value
    }
}

#[allow(dead_code)]
pub struct VolumeController {
    node_name: String,
}

impl VolumeController {
    pub fn new(node_name: String) -> Self {
        Self { node_name }
    }

    pub async fn get_volume(&self) -> Result<Volume> {
        Ok(Volume::new(1.0))
    }

    pub async fn set_volume(&self, _volume: f32) -> Result<()> {
        Ok(())
    }

    pub async fn set_muted(&self, _muted: bool) -> Result<()> {
        Ok(())
    }
}
