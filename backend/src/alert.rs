use chrono::Utc;
use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct AlertRecord {
    pub timestamp: String,
    pub spindle_id: String,
    pub alert_type: String,
    pub severity: String,
    pub message: String,
    pub value: f64,
    pub threshold: f64,
}

pub fn check_alerts(
    spindle_id: &str,
    rpm: f64,
    vibration_amplitude: f64,
    temperature: f64,
    twist_per_meter: f64,
    critical_rpm: f64,
) -> Vec<AlertRecord> {
    let mut alerts = Vec::new();

    if vibration_amplitude > 1.0 {
        alerts.push(AlertRecord {
            timestamp: Utc::now().to_rfc3339(),
            spindle_id: spindle_id.to_string(),
            alert_type: "vibration_overload".to_string(),
            severity: "critical".to_string(),
            message: format!("Vibration amplitude {:.3} mm exceeds critical threshold 1.0 mm", vibration_amplitude),
            value: vibration_amplitude,
            threshold: 1.0,
        });
    } else if vibration_amplitude > 0.5 {
        alerts.push(AlertRecord {
            timestamp: Utc::now().to_rfc3339(),
            spindle_id: spindle_id.to_string(),
            alert_type: "vibration_overload".to_string(),
            severity: "warning".to_string(),
            message: format!("Vibration amplitude {:.3} mm exceeds warning threshold 0.5 mm", vibration_amplitude),
            value: vibration_amplitude,
            threshold: 0.5,
        });
    }

    let twist_variance = (twist_per_meter - 800.0).abs() / 800.0;

    if twist_variance > 0.2 {
        alerts.push(AlertRecord {
            timestamp: Utc::now().to_rfc3339(),
            spindle_id: spindle_id.to_string(),
            alert_type: "twist_uneven".to_string(),
            severity: "critical".to_string(),
            message: format!("Twist variance {:.3} exceeds critical threshold 0.2", twist_variance),
            value: twist_variance,
            threshold: 0.2,
        });
    } else if twist_variance > 0.1 {
        alerts.push(AlertRecord {
            timestamp: Utc::now().to_rfc3339(),
            spindle_id: spindle_id.to_string(),
            alert_type: "twist_uneven".to_string(),
            severity: "warning".to_string(),
            message: format!("Twist variance {:.3} exceeds warning threshold 0.1", twist_variance),
            value: twist_variance,
            threshold: 0.1,
        });
    }

    if critical_rpm > 0.0 && (rpm - critical_rpm).abs() / critical_rpm <= 0.1 {
        alerts.push(AlertRecord {
            timestamp: Utc::now().to_rfc3339(),
            spindle_id: spindle_id.to_string(),
            alert_type: "critical_speed".to_string(),
            severity: "critical".to_string(),
            message: format!("RPM {:.1} is within 10% of critical RPM {:.1}", rpm, critical_rpm),
            value: rpm,
            threshold: critical_rpm,
        });
    }

    if temperature > 80.0 {
        alerts.push(AlertRecord {
            timestamp: Utc::now().to_rfc3339(),
            spindle_id: spindle_id.to_string(),
            alert_type: "temperature_high".to_string(),
            severity: "critical".to_string(),
            message: format!("Temperature {:.1}°C exceeds critical threshold 80°C", temperature),
            value: temperature,
            threshold: 80.0,
        });
    } else if temperature > 60.0 {
        alerts.push(AlertRecord {
            timestamp: Utc::now().to_rfc3339(),
            spindle_id: spindle_id.to_string(),
            alert_type: "temperature_high".to_string(),
            severity: "warning".to_string(),
            message: format!("Temperature {:.1}°C exceeds warning threshold 60°C", temperature),
            value: temperature,
            threshold: 60.0,
        });
    }

    alerts
}

pub fn alert_to_mqtt_payload(alert: &AlertRecord) -> String {
    serde_json::to_string(alert).unwrap()
}
