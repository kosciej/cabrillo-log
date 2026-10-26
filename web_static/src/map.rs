//! Bridge to the Leaflet helper defined in index.html (`window.cabrilloMap`).

use crate::common::Data;
use serde::Serialize;
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = cabrilloMap, js_name = mount)]
    fn js_mount(id: &str);
    #[wasm_bindgen(js_namespace = cabrilloMap, js_name = show)]
    fn js_show(json: &str);
}

#[derive(Serialize)]
struct Marker {
    country: &'static str,
    lat: f64,
    lon: f64,
    zone: u32,
    qsos: u32,
    calls: Vec<String>,
}

#[derive(Serialize)]
struct MapData {
    markers: Vec<Marker>,
    station: Option<(f64, f64)>,
}

/// Create the map in the element with the given id, replacing any earlier map.
pub fn mount(id: &str) {
    js_mount(id);
}

/// Plot one circle per country worked in the current report.
pub fn show(data: &Data) {
    let mut markers: HashMap<&'static str, Marker> = HashMap::new();
    for (_, q) in data.qsos() {
        let Some(e) = q.entity else { continue };
        let m = markers.entry(e.country).or_insert_with(|| Marker {
            country: e.country,
            lat: e.latitude,
            // cty.csv longitudes are positive to the west
            lon: -e.longitude,
            zone: e.cq_zone,
            qsos: 0,
            calls: Vec::new(),
        });
        m.qsos += 1;
        if !m.calls.contains(&q.call) {
            m.calls.push(q.call.clone());
        }
    }
    let payload = MapData {
        markers: markers.into_values().collect(),
        station: data.log.station.as_ref().map(|s| (s.lat, s.lon)),
    };
    if let Ok(json) = serde_json::to_string(&payload) {
        js_show(&json);
    }
}
