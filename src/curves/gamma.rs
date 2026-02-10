use super::Curve;

pub struct GammaCurve {
    pub gamma: f32,
}

impl Default for GammaCurve {
    fn default() -> Self {
        Self { gamma: 2.2 }
    }
}

impl Curve for GammaCurve {
    fn apply(&self, volume: f32) -> f32 {
        volume.powf(self.gamma).clamp(0.0, 1.0)
    }

    fn inverse(&self, brightness: f32) -> f32 {
        brightness.powf(1.0 / self.gamma).clamp(0.0, 1.0)
    }

    fn name(&self) -> &'static str {
        "gamma"
    }
}
