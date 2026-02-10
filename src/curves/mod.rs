pub mod gamma;
pub mod linear;
pub mod logarithmic;
pub mod perceptual;

pub trait Curve: Send + Sync {
    fn apply(&self, volume: f32) -> f32;
    fn inverse(&self, brightness: f32) -> f32;
    fn name(&self) -> &'static str;
}

pub use gamma::GammaCurve;
pub use linear::LinearCurve;
pub use logarithmic::LogarithmicCurve;
pub use perceptual::PerceptualCurve;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum CurveConfig {
    Linear,
    Logarithmic { base: Option<f32> },
    Gamma { gamma: Option<f32> },
    Perceptual,
}

impl CurveConfig {
    pub fn into_curve(self) -> Box<dyn Curve> {
        match self {
            CurveConfig::Linear => Box::new(LinearCurve),
            CurveConfig::Logarithmic { base } => Box::new(LogarithmicCurve {
                base: base.unwrap_or(10.0),
            }),
            CurveConfig::Gamma { gamma } => Box::new(GammaCurve {
                gamma: gamma.unwrap_or(2.2),
            }),
            CurveConfig::Perceptual => Box::new(PerceptualCurve),
        }
    }
}
