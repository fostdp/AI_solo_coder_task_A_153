use rumqttc::{AsyncClient, MqttOptions, QoS};
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Deserialize, Clone)]
pub struct SensorData {
    pub spindle_id: String,
    pub rpm: f64,
    pub vibration_amplitude: f64,
    pub temperature: f64,
    pub twist_per_meter: f64,
}

pub async fn start_mqtt_subscriber<F>(topic: &str, on_message: F) -> anyhow::Result<()>
where
    F: Fn(SensorData) + Send + Sync + 'static,
{
    let mut mqttoptions = MqttOptions::new("spindle-backend", "localhost", 1883);
    mqttoptions.set_keep_alive(Duration::from_secs(5));

    let (client, mut eventloop) = AsyncClient::new(mqttoptions, 10);
    client.subscribe(topic, QoS::AtLeastOnce).await?;

    let on_message = std::sync::Arc::new(on_message);

    loop {
        match eventloop.poll().await {
            Ok(notification) => {
                if let rumqttc::Event::Incoming(rumqttc::Incoming::Publish(publish)) = notification {
                    if let Ok(data) = serde_json::from_slice::<SensorData>(&publish.payload) {
                        on_message(data);
                    }
                }
            }
            Err(e) => {
                tracing::warn!("MQTT error: {:?}", e);
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
}
