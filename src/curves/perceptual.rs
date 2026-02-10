use super::Curve;

pub struct PerceptualCurve;

impl Curve for PerceptualCurve {
    fn apply(&self, volume: f32) -> f32 {
        if volume <= 0.08 {
            volume / 9.033
        } else {
            ((volume + 0.16) / 1.16).powf(3.0)
        }
        .clamp(0.0, 1.0)
    }

    fn inverse(&self, brightness: f32) -> f32 {
        if brightness <= 0.008856 {
            brightness * 9.033
        } else {
            1.16 * brightness.powf(1.0 / 3.0) - 0.16
        }
        .clamp(0.0, 1.0)
    }

    fn name(&self) -> &'static str {
        "perceptual"
    }
}
