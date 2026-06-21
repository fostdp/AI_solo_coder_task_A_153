use serde::Serialize;

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
}

pub fn analyze_vibration(rpm: f64, _vibration_amplitude: f64) -> VibrationResult {
    let m: f64 = 0.5;
    let shaft_length: f64 = 0.3;
    let shaft_diameter: f64 = 0.008;
    let e: f64 = 0.0001;
    let zeta: f64 = 0.02;
    let youngs_modulus: f64 = 210e9;
    let mu: f64 = 0.01;
    let bearing_length: f64 = 0.02;
    let bearing_diameter: f64 = 0.016;
    let bearing_radius: f64 = 0.008;
    let radial_clearance: f64 = 0.00005;
    let g: f64 = 9.81;

    let i_shaft = std::f64::consts::PI * shaft_diameter.powi(4) / 64.0;
    let k_shaft = 48.0 * youngs_modulus * i_shaft / shaft_length.powi(3);
    let omega_cr = (k_shaft / m).sqrt();
    let critical_rpm = omega_cr * 60.0 / (2.0 * std::f64::consts::PI);

    let omega = rpm * 2.0 * std::f64::consts::PI / 60.0;
    let r = omega / omega_cr;
    let unbalance_response = e * r.powi(2) / ((1.0 - r.powi(2)).powi(2) + (2.0 * zeta * r).powi(2)).sqrt();

    let n_rps = rpm / 60.0;
    let w = m * g;
    let sommerfeld = (mu * n_rps * bearing_length * bearing_diameter) / w * (bearing_radius / radial_clearance).powi(2);
    let eccentricity_ratio = 1.0 - 1.0 / (2.0 * sommerfeld + 1.0);

    let k0 = mu * omega * bearing_length * (bearing_radius / radial_clearance).powi(3) / (2.0 * std::f64::consts::PI);
    let oil_film_stiffness_x = k0 * (1.0 + 2.0 * eccentricity_ratio.powi(2));
    let oil_film_stiffness_y = k0 * (1.0 - 2.0 * eccentricity_ratio.powi(2));

    let c0 = mu * bearing_length * (bearing_radius / radial_clearance).powi(3) / (2.0 * std::f64::consts::PI);
    let oil_film_damping_x = c0 * (1.0 + eccentricity_ratio.powi(2));
    let oil_film_damping_y = c0 * (1.0 - eccentricity_ratio.powi(2));

    let f0 = m * e * omega.powi(2);
    let vibration_x = f0 / ((oil_film_stiffness_x - m * omega.powi(2)).powi(2) + (oil_film_damping_x * omega).powi(2)).sqrt();
    let vibration_y = f0 / ((oil_film_stiffness_y - m * omega.powi(2)).powi(2) + (oil_film_damping_y * omega).powi(2)).sqrt();

    let total_displacement = (vibration_x.powi(2) + vibration_y.powi(2)).sqrt();
    let phase_angle = (vibration_y / vibration_x).atan();

    let whirl_ratio = 0.5;

    VibrationResult {
        critical_rpm,
        unbalance_response,
        oil_film_stiffness_x,
        oil_film_stiffness_y,
        oil_film_damping_x,
        oil_film_damping_y,
        whirl_ratio,
        eccentricity_ratio,
        vibration_x,
        vibration_y,
        total_displacement,
        phase_angle,
    }
}
