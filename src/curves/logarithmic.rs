use super::Curve;

pub struct LogarithmicCurve {
    pub base: f32,
}

impl Default for LogarithmicCurve {
    fn default() -> Self {
        Self { base: 10.0 }
    }
}

impl Curve for LogarithmicCurve {
    fn apply(&self, volume: f32) -> f32 {
        if volume <= 0.0 {
            return 0.0;
        }
        (volume.powf(1.0 / self.base.log10())).clamp(0.0, 1.0)
    }

    fn inverse(&self, brightness: f32) -> f32 {
        brightness.powf(self.base.log10()).clamp(0.0, 1.0)
    }

    fn name(&self) -> &'static str {
        "logarithmic"
    }
}
