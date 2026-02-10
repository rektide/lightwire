use super::Curve;

pub struct LinearCurve;

impl Curve for LinearCurve {
    fn apply(&self, volume: f32) -> f32 {
        volume.clamp(0.0, 1.0)
    }

    fn inverse(&self, brightness: f32) -> f32 {
        brightness.clamp(0.0, 1.0)
    }

    fn name(&self) -> &'static str {
        "linear"
    }
}
