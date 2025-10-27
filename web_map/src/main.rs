use axum::{
    Router,
    extract::Multipart,
    http::StatusCode,
    response::{Html, Json},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tower::ServiceBuilder;
use tower_http::{cors::CorsLayer, services::ServeDir};

#[derive(Serialize, Deserialize)]
struct MapMarker {
    country: String,
    latitude: f64,
    longitude: f64,
    cq_zone: u32,
    itu_zone: u32,
    dxcc: u32,
    callsigns: Vec<String>,
}

#[tokio::main]
async fn main() {
    // Build the application with routes
    let app = Router::new()
        .route("/", get(index))
        .route("/upload", post(upload_log))
        .nest_service("/static", ServeDir::new("static"))
        .layer(ServiceBuilder::new().layer(CorsLayer::permissive()));

    // Run the server
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("Server running at http://{}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn index() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
}

async fn upload_log(mut multipart: Multipart) -> Result<Json<Vec<MapMarker>>, StatusCode> {
    let mut file_content = None;

    while let Some(field) = multipart.next_field().await.unwrap() {
        if field.name() == Some("logfile") {
            file_content = Some(field.bytes().await.unwrap());
            break;
        }
    }

    let content = match file_content {
        Some(bytes) => String::from_utf8(bytes.to_vec()).map_err(|_| StatusCode::BAD_REQUEST)?,
        None => return Err(StatusCode::BAD_REQUEST),
    };

    // Parse the Cabrillo log
    let log = cabrillo_log::CabrilloLog::parse(&content).map_err(|_| StatusCode::BAD_REQUEST)?;

    // Process QSOs and collect unique countries with their callsigns
    let mut country_contacts: std::collections::HashMap<String, (enricher::Entity, Vec<String>)> =
        std::collections::HashMap::new();

    for qso in &log.qsos {
        // Try to enrich both sent and received callsigns
        let sent_entity = enricher::enrich_callsign(&qso.sent_call);
        let rcvd_entity = enricher::enrich_callsign(&qso.rcvd_call);

        // Add to country contacts for sent callsign
        if let Some(entity) = sent_entity {
            let entry = country_contacts
                .entry(entity.country.to_string())
                .or_insert((entity.clone(), Vec::new()));
            if !entry.1.contains(&qso.sent_call) {
                entry.1.push(qso.sent_call.clone());
            }
        }

        // Add to country contacts for received callsign
        if let Some(entity) = rcvd_entity {
            let entry = country_contacts
                .entry(entity.country.to_string())
                .or_insert((entity.clone(), Vec::new()));
            if !entry.1.contains(&qso.rcvd_call) {
                entry.1.push(qso.rcvd_call.clone());
            }
        }
    }

    // Convert to markers
    let markers: Vec<MapMarker> = country_contacts
        .into_iter()
        .map(|(_, (entity, callsigns))| MapMarker {
            country: entity.country.to_string(),
            latitude: entity.latitude,
            longitude: entity.longitude,
            cq_zone: entity.cq_zone,
            itu_zone: entity.itu_zone,
            dxcc: entity.dxcc,
            callsigns,
        })
        .collect();

    Ok(Json(markers))
}
