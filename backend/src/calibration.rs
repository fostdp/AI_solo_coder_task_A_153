use serde::Serialize;
use std::collections::VecDeque;

const WINDOW_SIZE: usize = 200;
const LEARNING_RATE: f64 = 0.01;
const WEAR_ENERGY_COEFF: f64 = 1e-9;
const WEAR_TIME_COEFF: f64 = 2e-10;

#[derive(Serialize, Clone, Debug)]
pub struct CalibrationState {
    pub wear_coefficient: f64,
    pub beta0: f64,
    pub beta1: f64,
    pub beta2: f64,
    pub beta3: f64,
    pub alpha0: f64,
    pub alpha1: f64,
    pub alpha2: f64,
    pub alpha3: f64,
    pub cumulative_vibration_energy: f64,
    pub total_runtime_seconds: f64,
    pub sample_count: u64,
    pub last_prediction_error: f64,
}

impl Default for CalibrationState {
    fn default() -> Self {
        Self {
            wear_coefficient: 0.0,
            beta0: 95.0,
            beta1: -0.8,
            beta2: -0.3,
            beta3: -0.05,
            alpha0: 15.0,
            alpha1: 0.02,
            alpha2: -1.5,
            alpha3: -0.00001,
            cumulative_vibration_energy: 0.0,
            total_runtime_seconds: 0.0,
            sample_count: 0,
            last_prediction_error: 0.0,
        }
    }
}

#[derive(Clone)]
pub struct CalibrationSample {
    pub vibration_amplitude: f64,
    pub twist_per_meter: f64,
    pub measured_uniformity: f64,
    pub measured_strength: f64,
    pub timestamp_seconds: f64,
}

#[derive(Clone)]
pub struct OnlineCalibrator {
    state: CalibrationState,
    window: VecDeque<CalibrationSample>,
    last_timestamp: f64,
}

impl OnlineCalibrator {
    pub fn new() -> Self {
        Self {
            state: CalibrationState::default(),
            window: VecDeque::with_capacity(WINDOW_SIZE),
            last_timestamp: 0.0,
        }
    }

    pub fn state(&self) -> &CalibrationState {
        &self.state
    }

    pub fn add_sample(&mut self, sample: CalibrationSample) {
        if self.last_timestamp > 0.0 {
            let dt = (sample.timestamp_seconds - self.last_timestamp).max(0.0);
            self.state.total_runtime_seconds += dt;
            self.state.cumulative_vibration_energy += sample.vibration_amplitude.powi(2) * dt;
        }
        self.last_timestamp = sample.timestamp_seconds;

        self.window.push_back(sample);
        if self.window.len() > WINDOW_SIZE {
            self.window.pop_front();
        }

        self.state.sample_count += 1;
        self.update_wear_coefficient();
        self.lms_update();
    }

    fn update_wear_coefficient(&mut self) {
        let energy_term = WEAR_ENERGY_COEFF * self.state.cumulative_vibration_energy.sqrt();
        let time_term = WEAR_TIME_COEFF * self.state.total_runtime_seconds;
        self.state.wear_coefficient = (energy_term + time_term).min(0.3);
    }

    fn lms_update(&mut self) {
        if self.window.len() < 10 {
            return;
        }

        let target_twist = 800.0;
        let lr = LEARNING_RATE;

        let recent: Vec<_> = self.window.iter().rev().take(50).collect();

        for sample in &recent {
            let twist_var = (sample.twist_per_meter - target_twist).abs() / target_twist;

            let pred_uniformity = self.state.beta0
                + self.state.beta1 * sample.vibration_amplitude
                + self.state.beta2 * twist_var
                + self.state.beta3 * sample.vibration_amplitude * twist_var;

            let error_u = sample.measured_uniformity - pred_uniformity;
            self.state.last_prediction_error = error_u;

            let wear_factor = 1.0 + self.state.wear_coefficient;

            self.state.beta0 += lr * error_u;
            self.state.beta1 += lr * error_u * sample.vibration_amplitude * wear_factor;
            self.state.beta2 += lr * error_u * twist_var;
            self.state.beta3 += lr * error_u * sample.vibration_amplitude * twist_var;

            let twist_factor = sample.twist_per_meter / 100.0;
            let pred_strength = self.state.alpha0
                + self.state.alpha1 * twist_factor
                + self.state.alpha2 * sample.vibration_amplitude
                + self.state.alpha3 * twist_factor * twist_factor;

            let error_s = sample.measured_strength - pred_strength;

            self.state.alpha0 += lr * error_s;
            self.state.alpha1 += lr * error_s * twist_factor;
            self.state.alpha2 += lr * error_s * sample.vibration_amplitude * wear_factor;
            self.state.alpha3 += lr * error_s * twist_factor * twist_factor;
        }

        self.state.beta1 = self.state.beta1.clamp(-5.0, 0.0);
        self.state.beta2 = self.state.beta2.clamp(-2.0, 0.0);
        self.state.beta3 = self.state.beta3.clamp(-0.5, 0.0);
        self.state.alpha1 = self.state.alpha1.clamp(0.0, 0.2);
        self.state.alpha2 = self.state.alpha2.clamp(-5.0, 0.0);
        self.state.alpha3 = self.state.alpha3.clamp(-0.0001, 0.0);
    }

    pub fn predict(
        &self,
        vibration_amplitude: f64,
        twist_per_meter: f64,
    ) -> (f64, f64, f64, f64) {
        let target_twist = 800.0;
        let twist_var = (twist_per_meter - target_twist).abs() / target_twist;

        let wear_penalty = self.state.wear_coefficient * 3.0;

        let predicted_uniformity = self.state.beta0
            + self.state.beta1 * vibration_amplitude
            + self.state.beta2 * twist_var
            + self.state.beta3 * vibration_amplitude * twist_var
            - wear_penalty;

        let twist_factor = twist_per_meter / 100.0;
        let predicted_strength = self.state.alpha0
            + self.state.alpha1 * twist_factor
            + self.state.alpha2 * vibration_amplitude
            + self.state.alpha3 * twist_factor * twist_factor
            - wear_penalty * 0.5;

        (
            predicted_uniformity,
            predicted_strength,
            twist_var,
            self.state.wear_coefficient,
        )
    }
}

impl Default for OnlineCalibrator {
    fn default() -> Self {
        Self::new()
    }
}

pub fn simulate_measured_values(
    vibration_amplitude: f64,
    twist_per_meter: f64,
    wear_coeff: f64,
    noise: f64,
) -> (f64, f64) {
    let target_twist = 800.0;
    let twist_var = (twist_per_meter - target_twist).abs() / target_twist;

    let wear_penalty = wear_coeff * 5.0;

    let true_uniformity = 95.0
        - 0.9 * vibration_amplitude
        - 0.35 * twist_var
        - 0.06 * vibration_amplitude * twist_var
        - wear_penalty
        + noise;

    let twist_factor = twist_per_meter / 100.0;
    let true_strength = 15.0
        + 0.025 * twist_factor
        - 1.6 * vibration_amplitude
        - 0.000015 * twist_factor * twist_factor
        - wear_penalty * 0.5
        + noise * 0.3;

    (true_uniformity.max(0.0), true_strength.max(0.0))
}
