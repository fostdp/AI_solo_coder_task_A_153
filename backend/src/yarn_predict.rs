use rand::Rng;
use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct YarnQualityResult {
    pub predicted_uniformity: f64,
    pub predicted_strength: f64,
    pub twist_variance: f64,
    pub vibration_impact_factor: f64,
}

pub fn predict_yarn_quality(vibration_amplitude: f64, twist_per_meter: f64) -> YarnQualityResult {
    let target_twist = 800.0;
    let twist_variance = (twist_per_meter - target_twist).abs() / target_twist;

    let mut rng = rand::thread_rng();
    let noise: f64 = rng.gen_range(-1.0..1.0) * 0.5;

    let beta0 = 95.0;
    let beta1 = -0.8;
    let beta2 = -0.3;
    let beta3 = -0.05;
    let predicted_uniformity = beta0
        + beta1 * vibration_amplitude
        + beta2 * twist_variance
        + beta3 * vibration_amplitude * twist_variance
        + noise;

    let twist_factor = twist_per_meter / 100.0;
    let alpha0 = 15.0;
    let alpha1 = 0.02;
    let alpha2 = -1.5;
    let alpha3 = -0.00001;
    let strength_noise: f64 = rng.gen_range(-1.0..1.0) * 0.5;
    let predicted_strength = alpha0
        + alpha1 * twist_factor
        + alpha2 * vibration_amplitude
        + alpha3 * twist_factor * twist_factor
        + strength_noise;

    let lambda = 2.0;
    let vibration_impact_factor = 1.0 - (-lambda * vibration_amplitude).exp();

    YarnQualityResult {
        predicted_uniformity,
        predicted_strength,
        twist_variance,
        vibration_impact_factor,
    }
}
