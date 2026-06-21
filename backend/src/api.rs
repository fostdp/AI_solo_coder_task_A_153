use crate::ch_writer::ClickHouseWriter;
use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

#[derive(Serialize, Deserialize)]
pub struct SimulationRequest {
    pub spindle_id: String,
    pub rpm: f64,
    pub vibration_amplitude: f64,
    pub temperature: f64,
    pub twist_per_meter: f64,
}

#[derive(Serialize)]
pub struct SimulationResponse {
    pub vibration: crate::vibration::VibrationResult,
    pub yarn_quality: crate::yarn_predict::YarnQualityResult,
    pub alerts: Vec<crate::alert::AlertRecord>,
}

pub fn create_router(ch_writer: Arc<ClickHouseWriter>) -> Router {
    Router::new()
        .route("/api/sensor-data", get(get_sensor_data))
        .route("/api/vibration-analysis", get(get_vibration_analysis))
        .route("/api/yarn-quality", get(get_yarn_quality))
        .route("/api/alerts", get(get_alerts))
        .route("/api/simulate", post(run_simulation))
        .route("/api/spindle-list", get(get_spindle_list))
        .route("/api/latest/:spindle_id", get(get_latest))
        .with_state(ch_writer)
}

async fn get_sensor_data(
    State(writer): State<Arc<ClickHouseWriter>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Value>, StatusCode> {
    let limit = params.get("limit").map(|s| s.as_str()).unwrap_or("100");
    let spindle_id = params.get("spindle_id").map(|s| s.as_str()).unwrap_or("");
    let sql = if spindle_id.is_empty() {
        format!(
            "SELECT * FROM spindle_system.spindle_sensor_data ORDER BY timestamp DESC LIMIT {} FORMAT JSON",
            limit
        )
    } else {
        format!(
            "SELECT * FROM spindle_system.spindle_sensor_data WHERE spindle_id = '{}' ORDER BY timestamp DESC LIMIT {} FORMAT JSON",
            spindle_id, limit
        )
    };
    match writer.query(&sql).await {
        Ok(body) => {
            let parsed: Value = serde_json::from_str(&body).unwrap_or(json!({"data": body}));
            Ok(Json(parsed))
        }
        Err(e) => {
            tracing::error!("Query error: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn get_vibration_analysis(
    State(writer): State<Arc<ClickHouseWriter>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Value>, StatusCode> {
    let limit = params.get("limit").map(|s| s.as_str()).unwrap_or("100");
    let spindle_id = params.get("spindle_id").map(|s| s.as_str()).unwrap_or("");
    let sql = if spindle_id.is_empty() {
        format!(
            "SELECT * FROM spindle_system.vibration_analysis ORDER BY timestamp DESC LIMIT {} FORMAT JSON",
            limit
        )
    } else {
        format!(
            "SELECT * FROM spindle_system.vibration_analysis WHERE spindle_id = '{}' ORDER BY timestamp DESC LIMIT {} FORMAT JSON",
            spindle_id, limit
        )
    };
    match writer.query(&sql).await {
        Ok(body) => {
            let parsed: Value = serde_json::from_str(&body).unwrap_or(json!({"data": body}));
            Ok(Json(parsed))
        }
        Err(e) => {
            tracing::error!("Query error: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn get_yarn_quality(
    State(writer): State<Arc<ClickHouseWriter>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Value>, StatusCode> {
    let limit = params.get("limit").map(|s| s.as_str()).unwrap_or("100");
    let spindle_id = params.get("spindle_id").map(|s| s.as_str()).unwrap_or("");
    let sql = if spindle_id.is_empty() {
        format!(
            "SELECT * FROM spindle_system.yarn_quality ORDER BY timestamp DESC LIMIT {} FORMAT JSON",
            limit
        )
    } else {
        format!(
            "SELECT * FROM spindle_system.yarn_quality WHERE spindle_id = '{}' ORDER BY timestamp DESC LIMIT {} FORMAT JSON",
            spindle_id, limit
        )
    };
    match writer.query(&sql).await {
        Ok(body) => {
            let parsed: Value = serde_json::from_str(&body).unwrap_or(json!({"data": body}));
            Ok(Json(parsed))
        }
        Err(e) => {
            tracing::error!("Query error: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn get_alerts(
    State(writer): State<Arc<ClickHouseWriter>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Value>, StatusCode> {
    let limit = params.get("limit").map(|s| s.as_str()).unwrap_or("100");
    let sql = format!(
        "SELECT * FROM spindle_system.alerts ORDER BY timestamp DESC LIMIT {} FORMAT JSON",
        limit
    );
    match writer.query(&sql).await {
        Ok(body) => {
            let parsed: Value = serde_json::from_str(&body).unwrap_or(json!({"data": body}));
            Ok(Json(parsed))
        }
        Err(e) => {
            tracing::error!("Query error: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn run_simulation(
    State(_writer): State<Arc<ClickHouseWriter>>,
    Json(req): Json<SimulationRequest>,
) -> Json<SimulationResponse> {
    let vibration = crate::vibration::analyze_vibration(req.rpm, req.vibration_amplitude);
    let yarn_quality = crate::yarn_predict::predict_yarn_quality(req.vibration_amplitude, req.twist_per_meter);
    let alerts = crate::alert::check_alerts(
        &req.spindle_id,
        req.rpm,
        req.vibration_amplitude,
        req.temperature,
        req.twist_per_meter,
        vibration.critical_rpm,
    );
    Json(SimulationResponse {
        vibration,
        yarn_quality,
        alerts,
    })
}

async fn get_spindle_list(
    State(writer): State<Arc<ClickHouseWriter>>,
) -> Result<Json<Value>, StatusCode> {
    let sql = "SELECT DISTINCT spindle_id FROM spindle_system.spindle_sensor_data ORDER BY spindle_id FORMAT JSON".to_string();
    match writer.query(&sql).await {
        Ok(body) => {
            let parsed: Value = serde_json::from_str(&body).unwrap_or(json!({"data": body}));
            Ok(Json(parsed))
        }
        Err(e) => {
            tracing::error!("Query error: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn get_latest(
    State(writer): State<Arc<ClickHouseWriter>>,
    axum::extract::Path(spindle_id): axum::extract::Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let sql = format!(
        "SELECT s.*, v.*, y.* FROM spindle_system.spindle_sensor_data s LEFT JOIN spindle_system.vibration_analysis v ON s.spindle_id = v.spindle_id AND s.timestamp = v.timestamp LEFT JOIN spindle_system.yarn_quality y ON s.spindle_id = y.spindle_id AND s.timestamp = y.timestamp WHERE s.spindle_id = '{}' ORDER BY s.timestamp DESC LIMIT 1 FORMAT JSON",
        spindle_id
    );
    match writer.query(&sql).await {
        Ok(body) => {
            let parsed: Value = serde_json::from_str(&body).unwrap_or(json!({"data": body}));
            Ok(Json(parsed))
        }
        Err(e) => {
            tracing::error!("Query error: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
