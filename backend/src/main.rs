mod alarm_mqtt;
mod api;
mod ch_writer;
mod config;
mod mqtt_receiver;
mod quality_predictor;
mod vibration_simulator;

use alarm_mqtt::AlertRecord;
use ch_writer::{ClickHouseWriter, WriteCommand};
use config::AppConfig;
use mqtt_receiver::ValidatedSensorData;
use quality_predictor::QualityPredictor;
use vibration_simulator::VibrationSimulator;

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let cfg = Arc::new(AppConfig::load_default()?);
    tracing::info!("Configuration loaded");

    let ch_writer = Arc::new(ClickHouseWriter::new(&cfg.clickhouse.base_url));

    let vibration_sim = VibrationSimulator::new(
        cfg.rotor_dynamics.clone(),
        cfg.oil_film_bearing.clone(),
    );

    let quality_predictor = Arc::new(QualityPredictor::new(cfg.regression_model.clone()));

    let (write_tx, write_rx) = mpsc::unbounded_channel::<WriteCommand>();
    let (vib_tx, vib_rx) = mpsc::unbounded_channel::<(String, f64)>();
    let (vib_out_tx, vib_out_rx) =
        mpsc::unbounded_channel::<(String, vibration_simulator::VibrationResult)>();
    let (qual_tx, qual_rx) = mpsc::unbounded_channel::<(String, f64, f64, f64)>();
    let (qual_out_tx, qual_out_rx) =
        mpsc::unbounded_channel::<(String, quality_predictor::YarnQualityResult)>();
    let (alert_tx, alert_rx) = mpsc::unbounded_channel::<AlertRecord>();
    let (sensor_tx, sensor_rx) = mpsc::unbounded_channel::<ValidatedSensorData>();

    let writer_clone = Arc::clone(&ch_writer);
    tokio::spawn(async move {
        ch_writer::writer_loop(write_rx, writer_clone).await;
    });

    let cfg_clone = Arc::clone(&cfg);
    let sensor_tx_clone = sensor_tx.clone();
    tokio::spawn(async move {
        if let Err(e) =
            mqtt_receiver::run_receiver_service((*cfg_clone).clone(), sensor_tx_clone).await
        {
            tracing::error!("MQTT receiver service error: {}", e);
        }
    });

    let sim_clone = vibration_sim.clone();
    tokio::spawn(async move {
        vibration_simulator::run_vibration_service(sim_clone, vib_rx, vib_out_tx).await;
    });

    let predictor_clone = Arc::clone(&quality_predictor);
    tokio::spawn(async move {
        quality_predictor::run_quality_service(predictor_clone, qual_rx, qual_out_tx).await;
    });

    let mqtt_cfg = cfg.mqtt.clone();
    tokio::spawn(async move {
        if let Err(e) = alarm_mqtt::run_alert_mqtt_service(mqtt_cfg, alert_rx).await {
            tracing::error!("Alert MQTT service error: {}", e);
        }
    });

    let write_tx_dispatch = write_tx.clone();
    let vib_tx_dispatch = vib_tx.clone();
    let qual_tx_dispatch = qual_tx.clone();
    let alert_tx_dispatch = alert_tx.clone();
    let cfg_dispatch = Arc::clone(&cfg);
    tokio::spawn(async move {
        dispatch_loop(
            sensor_rx,
            vib_out_rx,
            qual_out_rx,
            write_tx_dispatch,
            vib_tx_dispatch,
            qual_tx_dispatch,
            alert_tx_dispatch,
            cfg_dispatch,
        )
        .await;
    });

    let app_state = Arc::new(api::AppState {
        ch_writer: Arc::clone(&ch_writer),
        quality_predictor: Arc::clone(&quality_predictor),
        vibration_simulator: vibration_sim.clone(),
        config: Arc::clone(&cfg),
    });
    let app = api::create_router(Arc::clone(&app_state));
    let addr_str = format!("{}:{}", cfg.api.bind_address, cfg.api.bind_port);
    let addr: SocketAddr = addr_str.parse()?;
    tracing::info!("Server listening on {}", addr);
    let server = axum::Server::bind(&addr).serve(app.into_make_service());
    server.await?;

    Ok(())
}

async fn dispatch_loop(
    mut sensor_rx: mpsc::UnboundedReceiver<ValidatedSensorData>,
    mut vib_out_rx: mpsc::UnboundedReceiver<(
        String,
        vibration_simulator::VibrationResult,
    )>,
    mut qual_out_rx: mpsc::UnboundedReceiver<(
        String,
        quality_predictor::YarnQualityResult,
    )>,
    write_tx: mpsc::UnboundedSender<WriteCommand>,
    vib_tx: mpsc::UnboundedSender<(String, f64)>,
    qual_tx: mpsc::UnboundedSender<(String, f64, f64, f64)>,
    alert_tx: mpsc::UnboundedSender<AlertRecord>,
    cfg: Arc<AppConfig>,
) {
    use std::collections::HashMap;

    let mut pending: HashMap<String, PendingState> = HashMap::new();

    loop {
        tokio::select! {
            Some(valid) = sensor_rx.recv() => {
                let spindle_id = valid.data.spindle_id.clone();
                let timestamp = valid.received_at.to_rfc3339();
                let sid = spindle_id.clone();

                let _ = write_tx.send(WriteCommand::SensorData {
                    timestamp: timestamp.clone(),
                    spindle_id: sid.clone(),
                    rpm: valid.data.rpm,
                    vibration_amplitude: valid.data.vibration_amplitude,
                    temperature: valid.data.temperature,
                    twist_per_meter: valid.data.twist_per_meter,
                });

                let entry = pending.entry(spindle_id.clone()).or_insert_with(PendingState::new);
                entry.timestamp = timestamp.clone();
                entry.raw_data = Some((
                    valid.data.rpm,
                    valid.data.vibration_amplitude,
                    valid.data.temperature,
                    valid.data.twist_per_meter,
                ));

                let _ = vib_tx.send((spindle_id.clone(), valid.data.rpm));
                let ts_secs = valid.received_at.timestamp_millis() as f64 / 1000.0;
                let _ = qual_tx.send((
                    spindle_id,
                    valid.data.vibration_amplitude,
                    valid.data.twist_per_meter,
                    ts_secs,
                ));
            }

            Some((sid, vib_result)) = vib_out_rx.recv() => {
                let entry = pending.entry(sid.clone()).or_insert_with(PendingState::new);
                entry.vibration = Some(vib_result.clone());

                let ts = entry.timestamp.clone();
                let _ = write_tx.send(WriteCommand::VibrationAnalysis {
                    timestamp: ts,
                    spindle_id: sid.clone(),
                    result: vib_result.clone(),
                });

                if let Some((rpm, vib_amp, temp, twist)) = entry.raw_data {
                    let alerts = alarm_mqtt::check_alerts(
                        &sid,
                        rpm,
                        vib_amp,
                        temp,
                        twist,
                        vib_result.critical_rpm,
                        vib_result.whirl_instability,
                        vib_result.whirl_ratio,
                        &cfg.alert_thresholds,
                        cfg.regression_model.target_twist_per_meter,
                    );
                    for a in alerts {
                        let _ = write_tx.send(WriteCommand::Alert { alert: a.clone() });
                        let _ = alert_tx.send(a);
                    }
                }
            }

            Some((sid, qual_result)) = qual_out_rx.recv() => {
                let entry = pending.entry(sid.clone()).or_insert_with(PendingState::new);
                let ts = entry.timestamp.clone();
                let _ = write_tx.send(WriteCommand::YarnQuality {
                    timestamp: ts,
                    spindle_id: sid,
                    result: qual_result,
                });
            }
        }
    }
}

struct PendingState {
    timestamp: String,
    raw_data: Option<(f64, f64, f64, f64)>,
    vibration: Option<vibration_simulator::VibrationResult>,
}

impl PendingState {
    fn new() -> Self {
        Self {
            timestamp: String::new(),
            raw_data: None,
            vibration: None,
        }
    }
}
