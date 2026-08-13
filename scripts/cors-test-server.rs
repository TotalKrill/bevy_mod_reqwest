#!/usr/bin/env -S cargo +nightly -Zscript
---cargo
[package]
edition = "2021"

[dependencies]
axum = "0.8"
serde_json = "1"
tokio = { version = "1", features = ["full"] }
tower-http = { version = "0.6", features = ["cors"] }
---

use axum::{
    extract::Json,
    routing::{get, post},
    Router,
};
use serde_json::{json, Value};
use tower_http::cors::CorsLayer;

#[tokio::main]
async fn main() {
    let addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8090".to_string());

    let app = Router::new()
        .route("/random", get(random))
        .route("/todos/1", get(random))
        .route("/get", get(random))
        .route("/posts", post(posts))
        .route("/post", post(posts))
        .layer(CorsLayer::permissive());

    println!("CORS test server listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn random() -> Json<Value> {
    Json(json!({
        "activity": "Use the local bevy_mod_reqwest CORS test server",
        "availability": 1.0,
        "type": "education",
        "participants": 1,
        "price": 0.0,
        "accessibility": "Few to no challenges",
        "duration": "minutes",
        "kidFriendly": true,
        "link": "",
        "key": "local-test"
    }))
}

async fn posts(Json(mut payload): Json<Value>) -> Json<Value> {
    match &mut payload {
        Value::Object(object) => {
            object.insert("id".to_string(), json!(101));
            Json(payload)
        }
        _ => Json(json!({ "body": payload, "id": 101 })),
    }
}
