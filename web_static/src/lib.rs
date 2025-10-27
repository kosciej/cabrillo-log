use chrono::Timelike;
use js_sys::Promise;
use serde::{Deserialize, Serialize};
use stats::{QSOByBand, QsoStats};
use std::collections::HashMap;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{File, FileReader};
use yew::prelude::*;

#[derive(Serialize, Deserialize, Clone, PartialEq)]
struct MapMarker {
    country: String,
    latitude: f64,
    longitude: f64,
    cq_zone: u32,
    itu_zone: u32,
    dxcc: u32,
    callsigns: Vec<String>,
}

#[derive(Clone)]
struct EnrichedQSO {
    qso: cabrillo_log::QSO,
    rcvd_entity: Option<enricher::Entity>,
}

#[derive(Clone, PartialEq)]
struct StatsData {
    qso_per_country_band: Vec<QSOByBand>,
    qso_per_continent: Vec<(String, u32)>,
    qso_per_hour_band: HashMap<String, HashMap<String, u32>>,
}

#[function_component(App)]
fn app() -> Html {
    let markers = use_state(Vec::<MapMarker>::new);
    let qsos = use_state(Vec::<EnrichedQSO>::new);
    let stats = use_state(|| None::<StatsData>);
    let loading = use_state(|| false);
    let error = use_state(String::new);
    let map_initialized = use_state(|| false);
    let tooltip = use_state(|| None::<(i32, i32, String)>);

    let on_file_change = {
        let markers = markers.clone();
        let qsos = qsos.clone();
        let stats = stats.clone();
        let loading = loading.clone();
        let error = error.clone();

        Callback::from(move |e: Event| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            let file = input.files().and_then(|files| files.get(0));

            if let Some(file) = file {
                let markers = markers.clone();
                let qsos = qsos.clone();
                let stats = stats.clone();
                let loading = loading.clone();
                let error = error.clone();

                wasm_bindgen_futures::spawn_local(async move {
                    loading.set(true);
                    error.set(String::new());

                    match process_file(file).await {
                        Ok((processed_markers, processed_qsos, processed_stats)) => {
                            markers.set(processed_markers);
                            qsos.set(processed_qsos);
                            stats.set(Some(processed_stats));
                        }
                        Err(err) => {
                            error.set(format!("Error processing file: {:?}", err));
                        }
                    }

                    loading.set(false);
                });
            }
        })
    };

    let on_process_click = {
        let markers = markers.clone();
        let qsos = qsos.clone();
        let stats = stats.clone();
        let loading = loading.clone();
        let error = error.clone();

        Callback::from(move |_| {
            let window = web_sys::window().unwrap();
            let document = window.document().unwrap();
            let input = document.get_element_by_id("file-input").unwrap();
            let input: web_sys::HtmlInputElement = input.dyn_into().unwrap();
            let file = input.files().and_then(|files| files.get(0));

            if let Some(file) = file {
                let markers = markers.clone();
                let qsos = qsos.clone();
                let stats = stats.clone();
                let loading = loading.clone();
                let error = error.clone();

                wasm_bindgen_futures::spawn_local(async move {
                    loading.set(true);
                    error.set(String::new());

                    match process_file(file).await {
                        Ok((processed_markers, processed_qsos, processed_stats)) => {
                            markers.set(processed_markers);
                            qsos.set(processed_qsos);
                            stats.set(Some(processed_stats));
                            web_sys::console::log_1(&"Markers processed successfully".into());
                        }
                        Err(err) => {
                            error.set(format!("Error processing file: {:?}", err));
                            web_sys::console::log_1(&format!("Error: {:?}", err).into());
                        }
                    }

                    loading.set(false);
                });
            } else {
                error.set("Please select a file first".to_string());
            }
        })
    };

    // Initialize map when component mounts
    {
        let markers = markers.clone();
        let map_initialized = map_initialized.clone();

        use_effect(move || {
            if !*map_initialized {
                init_map();
                map_initialized.set(true);
            }

            // Update map when markers change
            update_map(&markers);

            || ()
        });
    }

    let tooltip_clone = tooltip.clone();
    let on_mouse_move = Callback::from(move |e: MouseEvent| {
        if let Some((_, _, ref text)) = *tooltip_clone {
            tooltip_clone.set(Some((e.client_x(), e.client_y(), text.clone())));
        }
    });

    let tooltip_clone = tooltip.clone();
    let on_mouse_leave = Callback::from(move |_| {
        tooltip_clone.set(None);
    });

    html! {
        <div class="container">
            <h1>{"Cabrillo Log Geographic Map"}</h1>

            <div class="upload-section">
                <form class="upload-form">
                    <input
                        id="file-input"
                        type="file"
                        accept=".log,.txt"
                        onchange={on_file_change}
                        required=true
                    />
                    <button type="button" onclick={on_process_click}>{"Process File"}</button>
                </form>
                if !(*error).is_empty() {
                    <div class="error">{ &*error }</div>
                }
                if *loading {
                    <div class="loading">{"Processing log file..."}</div>
                }
            </div>

            <div id="map"></div>

            if let Some(stats_data) = &*stats {
                <div class="stats-section">
                    <h2>{"Statistics"}</h2>

                    <div class="stats-tables">
                        <div class="stats-table">
                            <h3>{"QSOs per Continent"}</h3>
                            <table class="stats-table-content">
                                <thead>
                                    <tr>
                                        <th>{"Continent"}</th>
                                        <th>{"Count"}</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    { for stats_data.qso_per_continent.iter().map(|(continent, count)| {
                                        html! {
                                            <tr>
                                                <td>{continent.clone()}</td>
                                                <td>{count}</td>
                                            </tr>
                                        }
                                    }) }
                                </tbody>
                            </table>
                        </div>
                        <div class="stats-table">
                            <h3>{"QSOs by Country and Band"}</h3>
                            <table class="stats-table-content">
                                <thead>
                                    <tr>
                                        <th>{"Country"}</th>
                                        <th>{"160m"}</th>
                                        <th>{"80m"}</th>
                                        <th>{"40m"}</th>
                                        <th>{"20m"}</th>
                                        <th>{"15m"}</th>
                                        <th>{"10m"}</th>
                                        <th>{"6m"}</th>
                                        <th>{"Total"}</th>
                                     </tr>
                                </thead>
                                <tbody>
                                    { for stats_data.qso_per_country_band.iter().map(|x| {
                                        html! {
                                            <tr>
                                                <td>{x.item.clone()}</td>
                                                <td>{x.count160m}</td>
                                                <td>{x.count80m}</td>
                                                <td>{x.count40m}</td>
                                                <td>{x.count20m}</td>
                                                <td>{x.count15m}</td>
                                                <td>{x.count10m}</td>
                                                <td>{x.count6m}</td>
                                                <td class="total-cell">{x.total}</td>
                                            </tr>
                                        }
                                    }) }
                                </tbody>
                            </table>
                        </div>

                        // <div class="stats-table">
                        //     <h3>{"QSOs per Hour and Band"}</h3>
                        //     <table class="stats-table-content">
                        //         <thead>
                        //             <tr>
                        //                 <th>{"Hour"}</th>
                        //                 { for stats_data.qso_per_hour_band.keys().map(|band| html! { <th>{band.clone()}</th> }) }
                        //             </tr>
                        //         </thead>
                        //         <tbody>
                        //             { for (0..24).map(|hour| {
                        //                 html! {
                        //                     <tr>
                        //                         <td>{format!("{:02}:00", hour)}</td>
                        //                         { for stats_data.qso_per_hour_band.keys().map(|band| {
                        //                             let hour_key = format!("{:02}", hour);
                        //                             let count = stats_data.qso_per_hour_band.get(&hour_key).and_then(|hours| hours.get(band)).unwrap_or(&0);
                        //                             let max_count = stats_data.qso_per_hour_band.values().flat_map(|hours| hours.values()).max().unwrap_or(&1);
                        //                             let intensity = if *max_count > 0 { (*count as f32 / *max_count as f32) * 255.0 } else { 0.0 };
                        //                             let bg_color = format!("rgba(255, 0, 0, {:.2})", intensity / 255.0);
                        //                             html! { <td style={format!("background-color: {}", bg_color)}>{count}</td> }
                        //                         }) }
                        //                     </tr>
                        //                 }
                        //             }) }
                        //         </tbody>
                        //     </table>
                        // </div>
                    </div>
                </div>
            }

            if !qsos.is_empty() {
                <div class="qso-table-section" onmousemove={on_mouse_move.clone()} onmouseleave={on_mouse_leave.clone()}>
                    <h2>{"QSOs"}</h2>
                    <table class="qso-table">
                        <thead>
                             <tr>
                                 <th>{"Date"}</th>
                                 <th>{"Time"}</th>
                                 <th>{"Frequency"}</th>
                                 <th>{"Mode"}</th>
                                 <th>{"Rcvd Callsign Country"}</th>
                                 <th>{"Sent Callsign"}</th>
                                 <th>{"Exchange"}</th>
                                 <th>{"Trx"}</th>
                                 <th>{"Rcvd Callsign"}</th>
                                 <th>{"Exchange"}</th>
                             </tr>
                         </thead>
                        <tbody>
                            { for qsos.iter().map(|enriched_qso| {
                                let tooltip_clone = tooltip.clone();
                                let rcvd_country = enriched_qso.rcvd_entity.as_ref().map(|e| e.country).unwrap_or("Unknown");
                                let cq_zone = enriched_qso.rcvd_entity.as_ref().map(|e| e.cq_zone).unwrap_or(0);
                                let itu_zone = enriched_qso.rcvd_entity.as_ref().map(|e| e.itu_zone).unwrap_or(0);
                                let dxcc = enriched_qso.rcvd_entity.as_ref().map(|e| e.dxcc).unwrap_or(0);
                                let tooltip_text = format!("CQ Zone: {}, ITU Zone: {}, DXCC: {}", cq_zone, itu_zone, dxcc);
                                let on_mouse_enter = Callback::from(move |e: MouseEvent| {
                                    tooltip_clone.set(Some((e.client_x(), e.client_y(), tooltip_text.clone())));
                                });
                                html! {
                                    <tr onmouseenter={on_mouse_enter} onmouseleave={on_mouse_leave.clone()}>
                                        <td>{enriched_qso.qso.date.format("%Y-%m-%d").to_string()}</td>
                                        <td>{enriched_qso.qso.time.format("%H%M").to_string()}</td>
                                        <td>{&enriched_qso.qso.freq}</td>
                                        <td>{&enriched_qso.qso.mode}</td>
                                        <td>{rcvd_country}</td>
                                        <td>{&enriched_qso.qso.sent_call}</td>
                                        <td>{&enriched_qso.qso.sent_rst_exch}</td>
                                        <td>{enriched_qso.qso.tx.as_ref().unwrap_or(&"".to_string())}</td>
                                        <td>{&enriched_qso.qso.rcvd_call}</td>
                                        <td>{&enriched_qso.qso.rcvd_rst_exch}</td>
                                    </tr>
                                }
                            })}
                        </tbody>
                    </table>
                    if let Some((x, y, text)) = &*tooltip {
                        <div class="tooltip" style={format!("left: {}px; top: {}px;", x + 10, y + 10)}>
                            {text.clone()}
                        </div>
                    }
                </div>
            }
        </div>
    }
}

async fn process_file(
    file: File,
) -> Result<(Vec<MapMarker>, Vec<EnrichedQSO>, StatsData), JsValue> {
    let file_reader = FileReader::new()?;
    let promise = Promise::new(&mut |resolve, reject| {
        file_reader.read_as_text(&file).unwrap();
        file_reader.set_onload(Some(&resolve));
        file_reader.set_onerror(Some(&reject));
    });

    let result = JsFuture::from(promise).await?;
    let content = js_sys::Reflect::get(&result, &"target".into())
        .ok()
        .and_then(|target| js_sys::Reflect::get(&target, &"result".into()).ok())
        .and_then(|result| result.as_string())
        .unwrap_or_default();

    web_sys::console::log_1(&format!("File content length: {}", content.len()).into());
    web_sys::console::log_1(
        &format!("First 200 chars: {}", &content[..content.len().min(200)]).into(),
    );

    if content.is_empty() {
        return Err(JsValue::from_str("File content is empty"));
    }

    // Parse the Cabrillo log
    let log = cabrillo_log::CabrilloLog::parse(&content)
        .map_err(|e| JsValue::from_str(&format!("Failed to parse log: {:?}", e)))?;
    // Process QSOs and collect unique countries with their callsigns
    let mut country_contacts: HashMap<String, (enricher::Entity, Vec<String>)> = HashMap::new();
    let mut enriched_qsos: Vec<EnrichedQSO> = Vec::new();

    for qso in &log.qsos {
        // Try to enrich both sent and received callsigns
        let sent_entity = enricher::enrich_callsign(&qso.sent_call);
        let rcvd_entity = enricher::enrich_callsign(&qso.rcvd_call);

        // Add enriched QSO
        enriched_qsos.push(EnrichedQSO {
            qso: qso.clone(),
            rcvd_entity: rcvd_entity.cloned(),
        });

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

    // Generate statistics
    let mut stats_analyzer = QsoStats::new(log.qsos.clone())
        .map_err(|e| JsValue::from_str(&format!("Failed to create stats: {:?}", e)))?;

    let _qso_per_country = stats_analyzer
        .qso_per_country(None)
        .map_err(|e| JsValue::from_str(&format!("Failed to get country stats: {:?}", e)))?;

    let qso_per_band = stats_analyzer
        .qso_per_band(None)
        .map_err(|e| JsValue::from_str(&format!("Failed to get band stats: {:?}", e)))?;

    let qso_per_country_band = stats_analyzer
        .qso_per_country_band(None)
        .map_err(|e| JsValue::from_str(&format!("Failed to get country-band stats: {:?}", e)))?;

    let qso_per_continent = stats_analyzer
        .qso_per_continent(None)
        .map_err(|e| JsValue::from_str(&format!("Failed to get continent stats: {:?}", e)))?;

    // QSOs per hour and band
    let mut qso_per_hour_band: HashMap<String, HashMap<String, u32>> = HashMap::new();
    for (band, _) in &qso_per_band {
        let mut hours = HashMap::new();
        for hour in 0..24 {
            // This is a simplified approach - we need to filter by hour
            // For now, we'll assume we need to implement hour filtering
            // Since QsoStats doesn't have hour filtering, we'll use a placeholder
            let count = 0; // Placeholder - need to implement proper hour filtering
            if count > 0 {
                hours.insert(format!("{:02}", hour), count);
            }
        }
        if !hours.is_empty() {
            qso_per_hour_band.insert(band.clone(), hours);
        }
    }

    // For QSOs per hour and band, we need to extract hour from timestamp
    // Since QsoStats doesn't expose this directly, we'll need to process the QSOs manually
    let mut qso_per_hour_band_real: HashMap<String, HashMap<String, u32>> = HashMap::new();
    for qso in &log.qsos {
        let hour = format!("{:02}", qso.time.hour());
        let band = stats::frequency_to_band(&qso.freq);
        *qso_per_hour_band_real
            .entry(band)
            .or_default()
            .entry(hour)
            .or_insert(0) += 1;
    }

    let stats_data = StatsData {
        qso_per_country_band,
        qso_per_continent,
        qso_per_hour_band: qso_per_hour_band_real,
    };

    Ok((markers, enriched_qsos, stats_data))
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

static mut MAP: Option<JsValue> = None;

fn init_map() {
    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();

    if let Some(_map_element) = document.get_element_by_id("map") {
        // Initialize Leaflet map
        let leaflet = js_sys::Reflect::get(&window, &"L".into()).unwrap();
        let map_constructor = js_sys::Reflect::get(&leaflet, &"map".into()).unwrap();

        let map = js_sys::Reflect::apply(
            &map_constructor.into(),
            &leaflet,
            &js_sys::Array::of1(&"map".into()),
        )
        .unwrap();

        // Set initial view
        let set_view = js_sys::Reflect::get(&map, &"setView".into()).unwrap();
        js_sys::Reflect::apply(
            &set_view.into(),
            &map,
            &js_sys::Array::of2(&js_sys::Array::of2(&20.into(), &0.into()).into(), &2.into()),
        )
        .unwrap();

        let tile_layer_constructor = js_sys::Reflect::get(&leaflet, &"tileLayer".into()).unwrap();
        let tile_layer = js_sys::Reflect::apply(
            &tile_layer_constructor.into(),
            &leaflet,
            &js_sys::Array::of2(
                &"https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png".into(),
                &{
                    let options = js_sys::Object::new();
                    js_sys::Reflect::set(
                        &options,
                        &"attribution".into(),
                        &"© OpenStreetMap contributors".into(),
                    )
                    .unwrap();
                    options
                }
                .into(),
            ),
        )
        .unwrap();

        js_sys::Reflect::apply(
            &js_sys::Reflect::get(&tile_layer, &"addTo".into())
                .unwrap()
                .into(),
            &tile_layer,
            &js_sys::Array::of1(&map),
        )
        .unwrap();

        unsafe {
            MAP = Some(map);
        }
    }
}

fn update_map(markers: &[MapMarker]) {
    unsafe {
        if let Some(ref map) = MAP {
            let clear_func = js_sys::Function::new_no_args(
                "
                var layers = [];
                this.eachLayer(function(layer) {
                    if (layer instanceof L.Marker) {
                        layers.push(layer);
                    }
                });
                layers.forEach(function(layer) {
                    this.removeLayer(layer);
                }, this);
            ",
            );
            js_sys::Reflect::apply(&clear_func, map, &js_sys::Array::new()).unwrap();

            // Add new markers
            for marker in markers {
                let window = web_sys::window().unwrap();
                let leaflet = js_sys::Reflect::get(&window, &"L".into()).unwrap();

                let marker_constructor = js_sys::Reflect::get(&leaflet, &"marker".into()).unwrap();
                let new_marker = js_sys::Reflect::apply(
                    &marker_constructor.into(),
                    &leaflet,
                    &js_sys::Array::of1(
                        &js_sys::Array::of2(&marker.latitude.into(), &(-marker.longitude).into())
                            .into(),
                    ),
                )
                .unwrap();

                let callsigns_list = marker
                    .callsigns
                    .iter()
                    .map(|call| format!("• {}", call))
                    .collect::<Vec<_>>()
                    .join("<br>");
                let popup_content = format!(
                    "<strong>{}</strong><br>CQ Zone: {}<br>ITU Zone: {}<br>DXCC: {}<br><br><strong>Callsigns contacted:</strong><br>{}",
                    marker.country, marker.cq_zone, marker.itu_zone, marker.dxcc, callsigns_list
                );

                let bind_popup = js_sys::Reflect::get(&new_marker, &"bindPopup".into()).unwrap();
                js_sys::Reflect::apply(
                    &bind_popup.into(),
                    &new_marker,
                    &js_sys::Array::of1(&popup_content.into()),
                )
                .unwrap();

                let add_to = js_sys::Reflect::get(&new_marker, &"addTo".into()).unwrap();
                js_sys::Reflect::apply(&add_to.into(), &new_marker, &js_sys::Array::of1(map))
                    .unwrap();
            }

            // Fit map to show all markers
            if !markers.is_empty() {
                let window = web_sys::window().unwrap();
                let leaflet = js_sys::Reflect::get(&window, &"L".into()).unwrap();
                let feature_group_constructor =
                    js_sys::Reflect::get(&leaflet, &"featureGroup".into()).unwrap();

                let marker_objects = markers
                    .iter()
                    .map(|m| {
                        let marker_constructor =
                            js_sys::Reflect::get(&leaflet, &"marker".into()).unwrap();
                        js_sys::Reflect::apply(
                            &marker_constructor.into(),
                            &leaflet,
                            &js_sys::Array::of1(
                                &js_sys::Array::of2(&m.latitude.into(), &(-m.longitude).into())
                                    .into(),
                            ),
                        )
                        .unwrap()
                    })
                    .collect::<js_sys::Array>();

                let group = js_sys::Reflect::apply(
                    &feature_group_constructor.into(),
                    &leaflet,
                    &js_sys::Array::of1(&marker_objects),
                )
                .unwrap();
                let bounds = js_sys::Reflect::get(&group, &"getBounds".into()).unwrap();
                let bounds_result =
                    js_sys::Reflect::apply(&bounds.into(), &group, &js_sys::Array::new()).unwrap();

                let fit_bounds = js_sys::Reflect::get(map, &"fitBounds".into()).unwrap();
                let pad = js_sys::Reflect::get(&bounds_result, &"pad".into()).unwrap();
                let padded_bounds = js_sys::Reflect::apply(
                    &pad.into(),
                    &bounds_result,
                    &js_sys::Array::of1(&0.1.into()),
                )
                .unwrap();
                js_sys::Reflect::apply(
                    &fit_bounds.into(),
                    map,
                    &js_sys::Array::of1(&padded_bounds),
                )
                .unwrap();
            }
        }
    }
}

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    yew::Renderer::<App>::new().render();
}
