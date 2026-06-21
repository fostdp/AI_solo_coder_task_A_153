mod alert;
mod api;
mod ch_writer;
mod mqtt_sub;
mod vibration;
mod yarn_predict;
mod calibration;

use ch_writer::{ClickHouseWriter, WriteCommand};
use yarn_predict::YarnPredictor;
use std::sync::Arc;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let ch_writer = Arc::new(ClickHouseWriter::new("http://localhost:8123"));
    let yarn_predictor = Arc::new(YarnPredictor::new());
    let (write_tx, write_rx) = mpsc::unbounded_channel::<WriteCommand>();
    let (alert_tx, alert_rx) = mpsc::unbounded_channel::<alert::AlertRecord>();

    let writer_clone = Arc::clone(&ch_writer);
    tokio::spawn(async move {
        ch_writer::writer_loop(write_rx, writer_clone).await;
    });

    let alert_mqtt_options = rumqttc::MqttOptions::new("spindle-alert-pub", "localhost", 1883);
    let (alert_client, alert_eventloop) =
        rumqttc::AsyncClient::new(alert_mqtt_options, 10);
    tokio::spawn(alert_publisher(alert_client, alert_eventloop, alert_rx));

    let alert_tx_clone = alert_tx.clone();
    let write_tx_clone = write_tx.clone();
    let yarn_predictor_clone = Arc::clone(&yarn_predictor);

    let on_message = move |data: mqtt_sub::SensorData| {
        let timestamp = chrono::Utc::now().to_rfc3339();
        let spindle_id = data.spindle_id.clone();

        let _ = write_tx_clone.send(WriteCommand::SensorData {
            timestamp: timestamp.clone(),
            spindle_id: spindle_id.clone(),
            rpm: data.rpm,
            vibration_amplitude: data.vibration_amplitude,
            temperature: data.temperature,
            twist_per_meter: data.twist_per_meter,
        });

        let vib_result = vibration::analyze_vibration(data.rpm, data.vibration_amplitude);

        let _ = write_tx_clone.send(WriteCommand::VibrationAnalysis {
            timestamp: timestamp.clone(),
            spindle_id: spindle_id.clone(),
            result: vib_result.clone(),
        });

        let yarn_result =
            yarn_predictor_clone.predict(&spindle_id, data.vibration_amplitude, data.twist_per_meter, chrono::Utc::now().timestamp_millis() as f64 / 1000.0);

        let _ = write_tx_clone.send(WriteCommand::YarnQuality {
            timestamp: timestamp.clone(),
            spindle_id: spindle_id.clone(),
            result: yarn_result,
        });

        let alerts = alert::check_alerts(
            &spindle_id,
            data.rpm,
            data.vibration_amplitude,
            data.temperature,
            data.twist_per_meter,
            vib_result.critical_rpm,
            vib_result.whirl_instability,
            vib_result.whirl_ratio,
        );

        for a in alerts {
            let _ = write_tx_clone.send(WriteCommand::Alert {
                alert: a.clone(),
            });
            let _ = alert_tx_clone.send(a);
        }
    };

    let mqtt_topic = "spindle/sensor_data";
    tokio::spawn(async move {
        if let Err(e) = mqtt_sub::start_mqtt_subscriber(mqtt_topic, on_message).await {
            tracing::error!("MQTT subscriber error: {}", e);
        }
    });

    let app_state = Arc::new(api::AppState {
        ch_writer: Arc::clone(&ch_writer),
        yarn_predictor: Arc::clone(&yarn_predictor),
    });
    let app = api::create_router(Arc::clone(&app_state));
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("Server listening on {}", addr);

    let server = axum::Server::bind(&addr).serve(app.into_make_service());
    server.await?;

    Ok(())
}

async fn alert_publisher(
    client: rumqttc::AsyncClient,
    mut eventloop: rumqttc::EventLoop,
    mut rx: mpsc::UnboundedReceiver<alert::AlertRecord>,
) {
    loop {
        tokio::select! {
            Some(alert) = rx.recv() => {
                let payload = alert::alert_to_mqtt_payload(&alert);
                if let Err(e) = client
                    .publish(
                        "spindle/alerts",
                        rumqttc::QoS::AtLeastOnce,
                        false,
                        payload.as_bytes(),
                    )
                    .await
                {
                    tracing::error!("MQTT alert publish error: {}", e);
                }
            }
            notification = eventloop.poll() => {
                if let Err(e) = notification {
                    tracing::warn!("MQTT alert connection error: {:?}", e);
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            }
        }
    }
}
