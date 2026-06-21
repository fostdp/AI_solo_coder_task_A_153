use crate::calibration::{CalibrationSample, OnlineCalibrator};
use rand::Rng;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Serialize, Clone)]
pub struct YarnQualityResult {
    pub predicted_uniformity: f64,
    pub predicted_strength: f64,
    pub twist_variance: f64,
    pub vibration_impact_factor: f64,
    pub wear_coefficient: f64,
    pub calibration_error: f64,
    pub sample_count: u64,
    pub beta0: f64,
    pub beta1: f64,
    pub alpha0: f64,
    pub alpha1: f64,
}

pub struct YarnPredictor {
    calibrators: Mutex<HashMap<String, OnlineCalibrator>>,
}

impl YarnPredictor {
    pub fn new() -> Self {
        Self {
            calibrators: Mutex::new(HashMap::new()),
        }
    }

    fn get_calibrator(&self, spindle_id: &str) -> OnlineCalibrator {
        let mut map = self.calibrators.lock().unwrap();
        map.entry(spindle_id.to_string())
            .or_insert_with(OnlineCalibrator::new)
            .clone()
    }

    fn update_calibrator(&self, spindle_id: &str, calibrator: OnlineCalibrator) {
        let mut map = self.calibrators.lock().unwrap();
        map.insert(spindle_id.to_string(), calibrator);
    }

    pub fn predict(
        &self,
        spindle_id: &str,
        vibration_amplitude: f64,
        twist_per_meter: f64,
        timestamp_seconds: f64,
    ) -> YarnQualityResult {
        let mut calibrator = self.get_calibrator(spindle_id);

        let (pred_uniformity_base, pred_strength_base, twist_variance, wear_coeff) =
            calibrator.predict(vibration_amplitude, twist_per_meter);

        let state = calibrator.state().clone();

        let mut rng = rand::thread_rng();
        let noise: f64 = rng.gen_range(-1.0..1.0) * 0.5;
        let (measured_uniformity, measured_strength) = crate::calibration::simulate_measured_values(
            vibration_amplitude,
            twist_per_meter,
            state.wear_coefficient,
            noise,
        );

        let sample = CalibrationSample {
            vibration_amplitude,
            twist_per_meter,
            measured_uniformity,
            measured_strength,
            timestamp_seconds,
        };
        calibrator.add_sample(sample);

        self.update_calibrator(spindle_id, calibrator);

        let state = self.get_calibrator(spindle_id).state().clone();

        let (predicted_uniformity, predicted_strength, twist_variance, wear_coefficient) =
            self.get_calibrator(spindle_id)
                .predict(vibration_amplitude, twist_per_meter);

        let lambda = 2.0;
        let vibration_impact_factor = 1.0 - (-lambda * vibration_amplitude).exp();

        YarnQualityResult {
            predicted_uniformity: predicted_uniformity.max(0.0),
            predicted_strength: predicted_strength.max(0.0),
            twist_variance,
            vibration_impact_factor,
            wear_coefficient,
            calibration_error: state.last_prediction_error.abs(),
            sample_count: state.sample_count,
            beta0: state.beta0,
            beta1: state.beta1,
            alpha0: state.alpha0,
            alpha1: state.alpha1,
        }
    }
}

impl Default for YarnPredictor {
    fn default() -> Self {
        Self::new()
    }
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
        predicted_uniformity: predicted_uniformity.max(0.0),
        predicted_strength: predicted_strength.max(0.0),
        twist_variance,
        vibration_impact_factor,
        wear_coefficient: 0.0,
        calibration_error: 0.0,
        sample_count: 0,
        beta0,
        beta1,
        alpha0,
        alpha1,
    }
}
