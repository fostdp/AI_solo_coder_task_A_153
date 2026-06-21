use crate::alert::AlertRecord;
use crate::vibration::VibrationResult;
use crate::yarn_predict::YarnQualityResult;
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::mpsc;

pub struct ClickHouseWriter {
    client: Client,
    base_url: String,
}

impl ClickHouseWriter {
    pub fn new(base_url: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.to_string(),
        }
    }

    pub async fn insert_sensor_data(
        &self,
        timestamp: &str,
        spindle_id: &str,
        rpm: f64,
        vibration_amplitude: f64,
        temperature: f64,
        twist_per_meter: f64,
    ) -> anyhow::Result<()> {
        let query = format!(
            "INSERT INTO spindle_system.spindle_sensor_data (timestamp, spindle_id, rpm, vibration_amplitude, temperature, twist_per_meter) VALUES ('{}', '{}', {}, {}, {}, {})",
            timestamp, spindle_id, rpm, vibration_amplitude, temperature, twist_per_meter
        );
        self.execute(&query).await
    }

    pub async fn insert_vibration_analysis(
        &self,
        timestamp: &str,
        spindle_id: &str,
        result: &VibrationResult,
    ) -> anyhow::Result<()> {
        let query = format!(
            "INSERT INTO spindle_system.vibration_analysis (timestamp, spindle_id, critical_rpm, unbalance_response, oil_film_stiffness_x, oil_film_stiffness_y, oil_film_damping_x, oil_film_damping_y, whirl_ratio, eccentricity_ratio, vibration_x, vibration_y, total_displacement, phase_angle) VALUES ('{}', '{}', {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {})",
            timestamp,
            spindle_id,
            result.critical_rpm,
            result.unbalance_response,
            result.oil_film_stiffness_x,
            result.oil_film_stiffness_y,
            result.oil_film_damping_x,
            result.oil_film_damping_y,
            result.whirl_ratio,
            result.eccentricity_ratio,
            result.vibration_x,
            result.vibration_y,
            result.total_displacement,
            result.phase_angle
        );
        self.execute(&query).await
    }

    pub async fn insert_yarn_quality(
        &self,
        timestamp: &str,
        spindle_id: &str,
        result: &YarnQualityResult,
    ) -> anyhow::Result<()> {
        let query = format!(
            "INSERT INTO spindle_system.yarn_quality (timestamp, spindle_id, predicted_uniformity, predicted_strength, twist_variance, vibration_impact_factor) VALUES ('{}', '{}', {}, {}, {}, {})",
            timestamp,
            spindle_id,
            result.predicted_uniformity,
            result.predicted_strength,
            result.twist_variance,
            result.vibration_impact_factor
        );
        self.execute(&query).await
    }

    pub async fn insert_alert(&self, alert: &AlertRecord) -> anyhow::Result<()> {
        let query = format!(
            "INSERT INTO spindle_system.alerts (timestamp, spindle_id, alert_type, severity, message, value, threshold) VALUES ('{}', '{}', '{}', '{}', '{}', {}, {})",
            alert.timestamp,
            alert.spindle_id,
            alert.alert_type,
            alert.severity,
            alert.message.replace('\'', "\\'"),
            alert.value,
            alert.threshold
        );
        self.execute(&query).await
    }

    pub async fn query(&self, sql: &str) -> anyhow::Result<String> {
        let url = format!("{}/?query={}", self.base_url, urlencoding(&sql));
        let resp = self.client.get(&url).send().await?;
        let body = resp.text().await?;
        Ok(body)
    }

    async fn execute(&self, sql: &str) -> anyhow::Result<()> {
        let url = format!("{}/?query={}", self.base_url, urlencoding(sql));
        let resp = self.client.post(&url).send().await?;
        if !resp.status().is_success() {
            let body = resp.text().await?;
            anyhow::bail!("ClickHouse error: {}", body);
        }
        Ok(())
    }
}

fn urlencoding(s: &str) -> String {
    s.replace(' ', "+")
        .replace('\'', "%27")
        .replace('\n', "%0A")
}

pub async fn writer_loop(
    mut rx: mpsc::UnboundedReceiver<WriteCommand>,
    writer: Arc<ClickHouseWriter>,
) {
    while let Some(cmd) = rx.recv().await {
        match cmd {
            WriteCommand::SensorData {
                timestamp,
                spindle_id,
                rpm,
                vibration_amplitude,
                temperature,
                twist_per_meter,
            } => {
                if let Err(e) = writer
                    .insert_sensor_data(
                        &timestamp,
                        &spindle_id,
                        rpm,
                        vibration_amplitude,
                        temperature,
                        twist_per_meter,
                    )
                    .await
                {
                    tracing::error!("Failed to write sensor data: {}", e);
                }
            }
            WriteCommand::VibrationAnalysis {
                timestamp,
                spindle_id,
                result,
            } => {
                if let Err(e) = writer
                    .insert_vibration_analysis(&timestamp, &spindle_id, &result)
                    .await
                {
                    tracing::error!("Failed to write vibration analysis: {}", e);
                }
            }
            WriteCommand::YarnQuality {
                timestamp,
                spindle_id,
                result,
            } => {
                if let Err(e) = writer
                    .insert_yarn_quality(&timestamp, &spindle_id, &result)
                    .await
                {
                    tracing::error!("Failed to write yarn quality: {}", e);
                }
            }
            WriteCommand::Alert { alert } => {
                if let Err(e) = writer.insert_alert(&alert).await {
                    tracing::error!("Failed to write alert: {}", e);
                }
            }
        }
    }
}

pub enum WriteCommand {
    SensorData {
        timestamp: String,
        spindle_id: String,
        rpm: f64,
        vibration_amplitude: f64,
        temperature: f64,
        twist_per_meter: f64,
    },
    VibrationAnalysis {
        timestamp: String,
        spindle_id: String,
        result: VibrationResult,
    },
    YarnQuality {
        timestamp: String,
        spindle_id: String,
        result: YarnQualityResult,
    },
    Alert {
        alert: AlertRecord,
    },
}
