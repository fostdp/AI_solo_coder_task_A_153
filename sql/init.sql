CREATE DATABASE IF NOT EXISTS spindle_system;

USE spindle_system;

CREATE TABLE IF NOT EXISTS spindle_sensor_data
(
    timestamp DateTime64(3),
    spindle_id String,
    rpm Float64,
    vibration_amplitude Float64,
    temperature Float64,
    twist_per_meter Float64
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (spindle_id, timestamp);

CREATE TABLE IF NOT EXISTS vibration_analysis
(
    timestamp DateTime64(3),
    spindle_id String,
    critical_rpm Float64,
    unbalance_response Float64,
    oil_film_stiffness_x Float64,
    oil_film_stiffness_y Float64,
    oil_film_damping_x Float64,
    oil_film_damping_y Float64,
    whirl_ratio Float64,
    eccentricity_ratio Float64,
    vibration_x Float64,
    vibration_y Float64,
    total_displacement Float64,
    phase_angle Float64,
    nonlinear_force_x Float64,
    nonlinear_force_y Float64,
    whirl_instability UInt8,
    nonlinear_damping_factor Float64,
    oil_film_pressure_peak Float64
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (spindle_id, timestamp);

CREATE TABLE IF NOT EXISTS yarn_quality
(
    timestamp DateTime64(3),
    spindle_id String,
    predicted_uniformity Float64,
    predicted_strength Float64,
    twist_variance Float64,
    vibration_impact_factor Float64,
    wear_coefficient Float64,
    calibration_error Float64,
    sample_count Int64,
    beta0 Float64,
    beta1 Float64,
    alpha0 Float64,
    alpha1 Float64
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (spindle_id, timestamp);

CREATE TABLE IF NOT EXISTS alerts
(
    timestamp DateTime64(3),
    spindle_id String,
    alert_type Enum8('vibration_overload' = 1, 'twist_uneven' = 2, 'critical_speed' = 3, 'temperature_high' = 4, 'oil_whirl' = 5),
    severity Enum8('warning' = 1, 'critical' = 2),
    message String,
    value Float64,
    threshold Float64
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (spindle_id, timestamp);
