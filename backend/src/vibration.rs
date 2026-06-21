use serde::Serialize;
use std::f64::consts::PI;

#[derive(Serialize, Clone)]
pub struct VibrationResult {
    pub critical_rpm: f64,
    pub unbalance_response: f64,
    pub oil_film_stiffness_x: f64,
    pub oil_film_stiffness_y: f64,
    pub oil_film_damping_x: f64,
    pub oil_film_damping_y: f64,
    pub whirl_ratio: f64,
    pub eccentricity_ratio: f64,
    pub vibration_x: f64,
    pub vibration_y: f64,
    pub total_displacement: f64,
    pub phase_angle: f64,
    pub nonlinear_force_x: f64,
    pub nonlinear_force_y: f64,
    pub whirl_instability: bool,
    pub nonlinear_damping_factor: f64,
    pub oil_film_pressure_peak: f64,
}

struct SpindleParams {
    m: f64,
    shaft_length: f64,
    shaft_diameter: f64,
    e_unbalance: f64,
    zeta: f64,
    youngs_modulus: f64,
    mu: f64,
    bearing_length: f64,
    bearing_diameter: f64,
    bearing_radius: f64,
    radial_clearance: f64,
    g: f64,
    alpha_nonlinear: f64,
    whirl_threshold: f64,
}

impl Default for SpindleParams {
    fn default() -> Self {
        Self {
            m: 0.5,
            shaft_length: 0.3,
            shaft_diameter: 0.008,
            e_unbalance: 0.0001,
            zeta: 0.02,
            youngs_modulus: 210e9,
            mu: 0.01,
            bearing_length: 0.02,
            bearing_diameter: 0.016,
            bearing_radius: 0.008,
            radial_clearance: 0.00005,
            g: 9.81,
            alpha_nonlinear: 5e6,
            whirl_threshold: 0.55,
        }
    }
}

fn reynolds_short_bearing_force(
    eccentricity: f64,
    epsilon: f64,
    theta: f64,
    omega: f64,
    p: &SpindleParams,
) -> (f64, f64, f64) {
    let c = p.radial_clearance;
    let R = p.bearing_radius;
    let L = p.bearing_length;
    let mu = p.mu;

    let eps = epsilon.min(0.95).max(0.01);
    let denom = 1.0 + eps * theta.cos();

    let pressure_coeff = mu * omega * R * R / (c * c);
    let z_factor = 2.0 / 3.0;

    let pressure_peak = pressure_coeff * eps * theta.sin() * z_factor / (denom * denom).max(1e-12);

    let k_pi = PI * (1.0 - eps * eps).powf(-1.5);
    let fx = -mu * omega * L.powi(3) * R / (c * c) * eps * (2.0 + eps * eps) * k_pi / (4.0 * (1.0 - eps * eps).powi(2));
    let fy = mu * omega * L.powi(3) * R / (c * c) * PI * eps / (2.0 * (1.0 - eps * eps).powi(2));

    let theta_rot = (omega * 0.5) * 0.01;
    let fx_rot = fx * theta_rot.cos() - fy * theta_rot.sin();
    let fy_rot = fx * theta_rot.sin() + fy * theta_rot.cos();

    (fx_rot, fy_rot, pressure_peak.abs())
}

fn nonlinear_damping(c_linear: f64, displacement: f64, alpha: f64) -> f64 {
    let disp = displacement.abs().min(0.001);
    c_linear * (1.0 + alpha * disp * disp)
}

fn detect_whirl_instability(omega: f64, omega_cr: f64, epsilon: f64) -> (bool, f64) {
    let r = omega / omega_cr;
    let threshold = if epsilon < 0.3 { 0.45 } else if epsilon < 0.6 { 0.5 } else { 0.55 };

    let mut whirl_ratio = 0.5;
    let unstable = r > threshold && epsilon > 0.2;

    if unstable {
        let factor = 1.0 + 0.3 * (r - threshold) / (1.0 - threshold).max(0.01);
        whirl_ratio = 0.5 * factor;
    }

    (unstable, whirl_ratio)
}

fn oil_whirl_amplitude_growth(base_amp: f64, omega: f64, omega_cr: f64, epsilon: f64) -> f64 {
    let (unstable, _) = detect_whirl_instability(omega, omega_cr, epsilon);
    if !unstable {
        return base_amp;
    }

    let r = omega / omega_cr;
    let growth = 1.0 + 2.5 * (r - 0.55).max(0.0) * (epsilon - 0.2).max(0.0) * 10.0;
    base_amp * growth.min(8.0)
}

fn compute_linear_coeffs(epsilon: f64, omega: f64, p: &SpindleParams) -> (f64, f64, f64, f64) {
    let k0 = p.mu * omega * p.bearing_length * (p.bearing_radius / p.radial_clearance).powi(3) / (2.0 * PI);
    let c0 = p.mu * p.bearing_length * (p.bearing_radius / p.radial_clearance).powi(3) / (2.0 * PI);

    let k_xx = k0 * (1.0 + 2.0 * epsilon * epsilon);
    let k_yy = k0 * (1.0 - 2.0 * epsilon * epsilon);
    let c_xx = c0 * (1.0 + epsilon * epsilon);
    let c_yy = c0 * (1.0 - epsilon * epsilon);

    (k_xx, k_yy, c_xx, c_yy)
}

pub fn analyze_vibration(rpm: f64, _vibration_amplitude: f64) -> VibrationResult {
    let p = SpindleParams::default();

    let i_shaft = PI * p.shaft_diameter.powi(4) / 64.0;
    let k_shaft = 48.0 * p.youngs_modulus * i_shaft / p.shaft_length.powi(3);
    let omega_cr = (k_shaft / p.m).sqrt();
    let critical_rpm = omega_cr * 60.0 / (2.0 * PI);

    let omega = rpm * 2.0 * PI / 60.0;
    let r = omega / omega_cr;
    let unbalance_response = p.e_unbalance * r.powi(2)
        / ((1.0 - r.powi(2)).powi(2) + (2.0 * p.zeta * r).powi(2)).sqrt();

    let n_rps = rpm / 60.0;
    let w = p.m * p.g;
    let sommerfeld = (p.mu * n_rps * p.bearing_length * p.bearing_diameter)
        / w * (p.bearing_radius / p.radial_clearance).powi(2);
    let eccentricity_ratio = 1.0 - 1.0 / (2.0 * sommerfeld + 1.0);

    let eccentricity = eccentricity_ratio * p.radial_clearance;

    let (k_xx, k_yy, c_xx_linear, c_yy_linear) = compute_linear_coeffs(eccentricity_ratio, omega, &p);

    let theta = omega * 0.1;
    let (nl_fx, nl_fy, pressure_peak) =
        reynolds_short_bearing_force(eccentricity, eccentricity_ratio, theta, omega, &p);

    let f0 = p.m * p.e_unbalance * omega.powi(2);

    let vib_x_linear = f0
        / ((k_xx - p.m * omega.powi(2)).powi(2) + (c_xx_linear * omega).powi(2)).sqrt();
    let vib_y_linear = f0
        / ((k_yy - p.m * omega.powi(2)).powi(2) + (c_yy_linear * omega).powi(2)).sqrt();

    let c_xx_nonlinear = nonlinear_damping(c_xx_linear, vib_x_linear, p.alpha_nonlinear);
    let c_yy_nonlinear = nonlinear_damping(c_yy_linear, vib_y_linear, p.alpha_nonlinear);

    let vib_x = f0
        / ((k_xx - p.m * omega.powi(2)).powi(2) + (c_xx_nonlinear * omega).powi(2)).sqrt();
    let vib_y = f0
        / ((k_yy - p.m * omega.powi(2)).powi(2) + (c_yy_nonlinear * omega).powi(2)).sqrt();

    let total_disp_linear = (vib_x_linear.powi(2) + vib_y_linear.powi(2)).sqrt();
    let (whirl_instability, whirl_ratio) = detect_whirl_instability(omega, omega_cr, eccentricity_ratio);

    let total_disp = oil_whirl_amplitude_growth(
        (vib_x.powi(2) + vib_y.powi(2)).sqrt(),
        omega,
        omega_cr,
        eccentricity_ratio,
    );

    let scale = if total_disp_linear > 1e-12 {
        total_disp / total_disp_linear
    } else {
        1.0
    };

    let vibration_x = vib_x * scale;
    let vibration_y = vib_y * scale;
    let phase_angle = (vibration_y / vibration_x).atan();

    let nonlinear_damping_factor = c_xx_nonlinear / c_xx_linear.max(1e-12);

    VibrationResult {
        critical_rpm,
        unbalance_response,
        oil_film_stiffness_x: k_xx,
        oil_film_stiffness_y: k_yy,
        oil_film_damping_x: c_xx_nonlinear,
        oil_film_damping_y: c_yy_nonlinear,
        whirl_ratio,
        eccentricity_ratio,
        vibration_x,
        vibration_y,
        total_displacement: total_disp,
        phase_angle,
        nonlinear_force_x: nl_fx,
        nonlinear_force_y: nl_fy,
        whirl_instability,
        nonlinear_damping_factor,
        oil_film_pressure_peak: pressure_peak,
    }
}
