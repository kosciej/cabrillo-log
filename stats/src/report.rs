//! # Contest report
//!
//! Computes a full contest-style report (rates, breaks, multipliers, geography,
//! callsign statistics) from a parsed Cabrillo log. Everything is computed in
//! memory in a few passes, so a report can be rebuilt cheaply for any
//! band/mode/transmitter filter.
//!
//! ## Example
//! ```rust
//! use cabrillo_log::CabrilloLog;
//! use stats::report::{ContestLog, ReportFilter};
//!
//! let log = CabrilloLog::parse("START-OF-LOG: 3.0\nCALLSIGN: SP5TLS\nQSO: 14000 CW 2023-10-01 1200 SP5TLS 599 15 W1AW 599 5\nEND-OF-LOG:").unwrap();
//! let contest = ContestLog::from_cabrillo(&log);
//! let report = contest.report(&ReportFilter::default());
//! assert_eq!(report.summary.qsos, 1);
//! ```

use crate::frequency_to_band;
use cabrillo_log::CabrilloLog;
use chrono::{Duration, NaiveDateTime, Timelike};
use enricher::{Entity, enrich_callsign};
use std::collections::{BTreeMap, HashMap, HashSet};

/// Gap between consecutive QSOs that counts as a break (CQ WW off-time rule).
pub const BREAK_MINUTES: i64 = 60;

/// Sliding windows used for "best rate" figures.
pub const RATE_WINDOWS: [i64; 3] = [10, 30, 60];

/// Bands in display order; anything else is sorted after these.
pub const BAND_ORDER: [&str; 15] = [
    "160m", "80m", "60m", "40m", "30m", "20m", "17m", "15m", "12m", "10m", "6m", "4m", "2m",
    "70cm", "Unknown",
];

fn band_rank(band: &str) -> usize {
    BAND_ORDER
        .iter()
        .position(|b| *b == band)
        .unwrap_or(BAND_ORDER.len())
}

/// Header information of the log.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LogInfo {
    pub callsign: String,
    pub contest: String,
    pub category: String,
    pub operators: String,
    pub claimed_score: String,
    pub club: String,
    pub location: String,
    pub locator: String,
    pub created_by: String,
    pub soapbox: String,
}

impl LogInfo {
    fn from_headers(headers: &HashMap<String, String>) -> Self {
        let get = |key: &str| headers.get(key).cloned().unwrap_or_default();
        let category = [
            "CATEGORY-OPERATOR",
            "CATEGORY-TRANSMITTER",
            "CATEGORY-BAND",
            "CATEGORY-MODE",
            "CATEGORY-POWER",
            "CATEGORY-ASSISTED",
            "CATEGORY-STATION",
            "CATEGORY-OVERLAY",
        ]
        .iter()
        .filter_map(|key| headers.get(*key))
        .filter(|v| !v.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join(" ");

        LogInfo {
            callsign: get("CALLSIGN"),
            contest: get("CONTEST"),
            category,
            operators: get("OPERATORS"),
            claimed_score: get("CLAIMED-SCORE"),
            club: get("CLUB"),
            location: get("LOCATION"),
            locator: get("GRID-LOCATOR"),
            created_by: get("CREATED-BY"),
            soapbox: get("SOAPBOX"),
        }
    }
}

/// Position of the operating station, used for distance and beam heading.
#[derive(Debug, Clone, PartialEq)]
pub struct Station {
    pub lat: f64,
    pub lon: f64,
    /// True when taken from GRID-LOCATOR, false when it is the country centroid.
    pub from_locator: bool,
}

/// A single QSO with enrichment and derived data.
#[derive(Debug, Clone)]
pub struct LogQso {
    /// 1-based position in the original log.
    pub number: usize,
    pub time: NaiveDateTime,
    pub freq: String,
    pub band: String,
    pub mode: String,
    pub call: String,
    pub sent_exch: String,
    pub rcvd_exch: String,
    pub tx: Option<String>,
    pub entity: Option<&'static Entity>,
    /// WPX-style prefix, e.g. "SP5" for SP5TLS.
    pub prefix: String,
    pub distance_km: Option<f64>,
    pub bearing: Option<f64>,
    /// Same call already worked on this band and mode.
    pub dupe: bool,
    /// CQ zone: from the received exchange in CQ WW logs, else from the country file.
    pub zone: u32,
    /// Estimated run QSO (true) or search & pounce (false); None when the log's
    /// frequencies are too coarse to tell.
    pub run: Option<bool>,
}

impl LogQso {
    pub fn country(&self) -> &'static str {
        self.entity.map(|e| e.country).unwrap_or("")
    }

    pub fn continent(&self) -> &'static str {
        self.entity.map(|e| e.continent).unwrap_or("")
    }

    pub fn cq_zone(&self) -> u32 {
        self.zone
    }
}

/// Which QSOs a report covers. `None` means "all".
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ReportFilter {
    pub band: Option<String>,
    pub mode: Option<String>,
    pub tx: Option<String>,
}

impl ReportFilter {
    pub fn matches(&self, qso: &LogQso) -> bool {
        self.band.as_ref().is_none_or(|b| *b == qso.band)
            && self.mode.as_ref().is_none_or(|m| *m == qso.mode)
            && self
                .tx
                .as_ref()
                .is_none_or(|t| qso.tx.as_ref() == Some(t))
    }
}

/// A parsed, enriched log ready for reporting.
#[derive(Debug, Clone)]
pub struct ContestLog {
    pub info: LogInfo,
    pub station: Option<Station>,
    /// QSOs sorted by time.
    pub qsos: Vec<LogQso>,
    /// QSO lines the parser had to skip, as "line N: text".
    pub skipped: Vec<String>,
    /// CQ zones were taken from the received exchange.
    pub zones_from_exchange: bool,
    /// Frequencies are precise enough to estimate run vs search & pounce.
    pub has_run_estimate: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Summary {
    pub qsos: u32,
    pub dupes: u32,
    pub unique_calls: u32,
    pub dxcc: u32,
    pub cq_zones: u32,
    pub itu_zones: u32,
    pub continents: u32,
    pub prefixes: u32,
    /// Sum over bands of (DXCC + CQ zones) - the CQ WW multiplier count.
    pub band_mults: u32,
    pub first: Option<NaiveDateTime>,
    pub last: Option<NaiveDateTime>,
    pub participation_min: i64,
    pub break_min: i64,
    pub operating_min: i64,
    /// QSOs per operating hour.
    pub avg_rate: f64,
    pub avg_km: Option<f64>,
    pub longest: Option<(String, f64)>,
    pub unknown_calls: u32,
    /// Estimated run QSOs; None when the log has no run estimate.
    pub run_qsos: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BandRow {
    pub band: String,
    pub qsos: u32,
    pub dupes: u32,
    pub unique_calls: u32,
    pub dxcc: u32,
    pub cq_zones: u32,
    pub best_hour: u32,
    pub run: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HourRow {
    pub start: NaiveDateTime,
    /// Aligned with `Report::bands`.
    pub per_band: Vec<u32>,
    pub total: u32,
    pub cumulative: u32,
    pub new_dxcc: Vec<&'static str>,
    pub new_zones: Vec<u32>,
    pub run: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Break {
    pub start: NaiveDateTime,
    pub end: NaiveDateTime,
    pub minutes: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RateWindow {
    pub minutes: i64,
    pub qsos: u32,
    pub start: NaiveDateTime,
}

impl RateWindow {
    pub fn per_hour(&self) -> f64 {
        self.qsos as f64 * 60.0 / self.minutes as f64
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CountryRow {
    pub country: &'static str,
    pub prefix: &'static str,
    pub continent: &'static str,
    pub cq_zone: u32,
    pub per_band: Vec<u32>,
    pub total: u32,
    pub unique_calls: u32,
    pub first: NaiveDateTime,
    pub distance_km: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContinentRow {
    pub continent: &'static str,
    pub per_band: Vec<u32>,
    pub total: u32,
    pub countries: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ZoneRow {
    pub zone: u32,
    pub per_band: Vec<u32>,
    pub total: u32,
    pub first: NaiveDateTime,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StationRow {
    pub call: String,
    pub country: &'static str,
    pub qsos: u32,
    pub bands: Vec<String>,
}

/// The complete report for one filter.
#[derive(Debug, Clone, Default)]
pub struct Report {
    /// Bands present in the report, in band order. Per-band vectors align with it.
    pub bands: Vec<String>,
    /// Indices into `ContestLog::qsos` of the QSOs in this report, in time order.
    pub qsos: Vec<usize>,
    pub summary: Summary,
    pub band_rows: Vec<BandRow>,
    pub hours: Vec<HourRow>,
    pub breaks: Vec<Break>,
    pub best_rates: Vec<RateWindow>,
    pub countries: Vec<CountryRow>,
    pub continents: Vec<ContinentRow>,
    pub zones: Vec<ZoneRow>,
    /// QSO count per 22.5° beam heading sector, starting at north.
    pub headings: [u32; 16],
    pub stations: Vec<StationRow>,
    /// (number of bands, stations worked on that many bands)
    pub bands_per_station: Vec<(usize, u32)>,
    /// (callsign length, unique calls)
    pub call_lengths: Vec<(usize, u32)>,
    /// (prefix, QSOs), most worked first
    pub prefixes: Vec<(String, u32)>,
    pub unknown_calls: Vec<String>,
    /// (transmitter, QSOs) when the log has a TX column.
    pub per_tx: Vec<(String, u32)>,
    /// Indices of QSOs that worked a new DXCC entity within this report.
    pub new_dxcc: HashSet<usize>,
    /// Indices of QSOs that worked a new CQ zone within this report.
    pub new_zone: HashSet<usize>,
    /// Longest runs, most QSOs first. Empty without a run estimate.
    pub runs: Vec<Run>,
}

/// A stretch of consecutive run QSOs on one frequency.
#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    pub band: String,
    pub freq: String,
    pub tx: Option<String>,
    pub start: NaiveDateTime,
    pub end: NaiveDateTime,
    pub qsos: u32,
}

impl Run {
    pub fn minutes(&self) -> i64 {
        minute_of(self.end) - minute_of(self.start) + 1
    }

    pub fn per_hour(&self) -> f64 {
        self.qsos as f64 * 60.0 / self.minutes() as f64
    }
}

impl ContestLog {
    pub fn from_cabrillo(log: &CabrilloLog) -> Self {
        let info = LogInfo::from_headers(&log.headers);
        let station = locator_to_lat_lon(&info.locator)
            .map(|(lat, lon)| Station {
                lat,
                lon,
                from_locator: true,
            })
            .or_else(|| {
                let call = if info.callsign.is_empty() {
                    log.qsos.first().map(|q| q.sent_call.as_str()).unwrap_or("")
                } else {
                    info.callsign.as_str()
                };
                enrich_callsign(&base_call(call)).map(|e| Station {
                    lat: e.latitude,
                    lon: -e.longitude,
                    from_locator: false,
                })
            });

        let mut qsos: Vec<LogQso> = log
            .qsos
            .iter()
            .enumerate()
            .map(|(i, q)| {
                let call = q.rcvd_call.to_uppercase();
                let entity = enrich_callsign(&call).or_else(|| enrich_callsign(&base_call(&call)));
                let (distance_km, bearing) = match (&station, entity) {
                    (Some(s), Some(e)) => {
                        let (d, b) = distance_and_bearing(s.lat, s.lon, e.latitude, -e.longitude);
                        (Some(d), Some(b))
                    }
                    _ => (None, None),
                };
                LogQso {
                    number: i + 1,
                    time: NaiveDateTime::new(q.date, q.time),
                    freq: q.freq.clone(),
                    band: frequency_to_band(&q.freq),
                    mode: q.mode.to_uppercase(),
                    prefix: wpx_prefix(&call),
                    call,
                    sent_exch: q.sent_rst_exch.clone(),
                    rcvd_exch: q.rcvd_rst_exch.clone(),
                    tx: q.tx.clone(),
                    zone: entity.map(|e| e.cq_zone).unwrap_or(0),
                    entity,
                    distance_km,
                    bearing,
                    dupe: false,
                    run: None,
                }
            })
            .collect();

        qsos.sort_by_key(|q| (q.time, q.number));

        let mut seen = HashSet::new();
        for q in &mut qsos {
            q.dupe = !seen.insert((q.call.clone(), q.band.clone(), q.mode.clone()));
        }

        let zones_from_exchange = info.contest.to_uppercase().starts_with("CQ-WW");
        if zones_from_exchange {
            for q in &mut qsos {
                if let Some(zone) = exchange_zone(&q.rcvd_exch) {
                    q.zone = zone;
                }
            }
        }
        let has_run_estimate = estimate_run(&mut qsos);

        let skipped = log
            .skipped
            .iter()
            .map(|s| format!("line {}: {}", s.line_number, s.text))
            .collect();

        ContestLog {
            info,
            station,
            qsos,
            skipped,
            zones_from_exchange,
            has_run_estimate,
        }
    }

    /// Bands present in the log, in band order.
    pub fn bands(&self) -> Vec<String> {
        sorted_bands(self.qsos.iter().map(|q| q.band.as_str()))
    }

    pub fn modes(&self) -> Vec<String> {
        let mut modes: Vec<String> = self
            .qsos
            .iter()
            .map(|q| q.mode.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        modes.sort();
        modes
    }

    pub fn transmitters(&self) -> Vec<String> {
        let mut txs: Vec<String> = self
            .qsos
            .iter()
            .filter_map(|q| q.tx.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        txs.sort();
        txs
    }

    pub fn report(&self, filter: &ReportFilter) -> Report {
        let indices: Vec<usize> = (0..self.qsos.len())
            .filter(|&i| filter.matches(&self.qsos[i]))
            .collect();
        let qsos: Vec<&LogQso> = indices.iter().map(|&i| &self.qsos[i]).collect();
        let bands = sorted_bands(qsos.iter().map(|q| q.band.as_str()));
        let band_index: HashMap<&str, usize> = bands
            .iter()
            .enumerate()
            .map(|(i, b)| (b.as_str(), i))
            .collect();

        let mut report = Report {
            bands: bands.clone(),
            qsos: indices.clone(),
            ..Default::default()
        };
        if qsos.is_empty() {
            return report;
        }

        // First occurrence of each multiplier, in time order.
        let mut dxcc_seen = HashSet::new();
        let mut zone_seen = HashSet::new();
        for (&idx, q) in indices.iter().zip(&qsos) {
            if q.entity.is_some_and(|e| dxcc_seen.insert(e.country)) {
                report.new_dxcc.insert(idx);
            }
            if q.zone > 0 && zone_seen.insert(q.zone) {
                report.new_zone.insert(idx);
            }
        }

        report.breaks = find_breaks(&qsos);
        report.best_rates = RATE_WINDOWS
            .iter()
            .filter_map(|&w| best_rate(&qsos, w))
            .collect();
        report.hours = hourly(&qsos, &indices, &band_index, &report);
        report.band_rows = band_rows(&qsos, &bands, &report.hours);
        report.countries = countries(&qsos, &band_index);
        report.continents = continents(&qsos, &band_index);
        report.zones = zones(&qsos, &band_index);

        for q in &qsos {
            if let Some(b) = q.bearing {
                let sector = (((b + 11.25) % 360.0) / 22.5) as usize;
                report.headings[sector.min(15)] += 1;
            }
        }

        callsign_stats(&qsos, &mut report);

        let mut per_tx: BTreeMap<String, u32> = BTreeMap::new();
        for q in &qsos {
            if let Some(tx) = &q.tx {
                *per_tx.entry(tx.clone()).or_default() += 1;
            }
        }
        report.per_tx = per_tx.into_iter().collect();

        report.runs = runs(&qsos);
        report.summary = summary(&qsos, &report);
        report
    }
}

fn sorted_bands<'a>(bands: impl Iterator<Item = &'a str>) -> Vec<String> {
    let mut list: Vec<String> = bands
        .collect::<HashSet<_>>()
        .into_iter()
        .map(String::from)
        .collect();
    list.sort_by_key(|b| (band_rank(b), b.clone()));
    list
}

fn minute_of(t: NaiveDateTime) -> i64 {
    t.and_utc().timestamp() / 60
}

fn hour_start(t: NaiveDateTime) -> NaiveDateTime {
    t.date().and_hms_opt(t.hour(), 0, 0).unwrap()
}

fn find_breaks(qsos: &[&LogQso]) -> Vec<Break> {
    qsos.windows(2)
        .filter_map(|w| {
            let minutes = minute_of(w[1].time) - minute_of(w[0].time);
            (minutes >= BREAK_MINUTES).then(|| Break {
                start: w[0].time,
                end: w[1].time,
                minutes,
            })
        })
        .collect()
}

/// Most QSOs within any `window` consecutive minutes.
fn best_rate(qsos: &[&LogQso], window: i64) -> Option<RateWindow> {
    let minutes: Vec<i64> = qsos.iter().map(|q| minute_of(q.time)).collect();
    let mut best: Option<(usize, usize)> = None;
    let mut start = 0;
    for end in 0..minutes.len() {
        while minutes[end] - minutes[start] >= window {
            start += 1;
        }
        if best.is_none_or(|(s, e)| end - start > e - s) {
            best = Some((start, end));
        }
    }
    best.map(|(s, e)| RateWindow {
        minutes: window,
        qsos: (e - s + 1) as u32,
        start: qsos[s].time,
    })
}

fn hourly(
    qsos: &[&LogQso],
    indices: &[usize],
    band_index: &HashMap<&str, usize>,
    report: &Report,
) -> Vec<HourRow> {
    let first = hour_start(qsos[0].time);
    let last = hour_start(qsos[qsos.len() - 1].time);
    let mut rows = Vec::new();
    let mut t = first;
    while t <= last {
        rows.push(HourRow {
            start: t,
            per_band: vec![0; band_index.len()],
            total: 0,
            cumulative: 0,
            new_dxcc: Vec::new(),
            new_zones: Vec::new(),
            run: 0,
        });
        t += Duration::hours(1);
    }

    for (&idx, q) in indices.iter().zip(qsos) {
        let row = &mut rows[((hour_start(q.time) - first).num_hours()) as usize];
        row.per_band[band_index[q.band.as_str()]] += 1;
        row.total += 1;
        if q.run == Some(true) {
            row.run += 1;
        }
        if report.new_dxcc.contains(&idx) {
            row.new_dxcc.push(q.country());
        }
        if report.new_zone.contains(&idx) {
            row.new_zones.push(q.cq_zone());
        }
    }

    let mut cumulative = 0;
    for row in &mut rows {
        cumulative += row.total;
        row.cumulative = cumulative;
    }
    rows
}

fn band_rows(qsos: &[&LogQso], bands: &[String], hours: &[HourRow]) -> Vec<BandRow> {
    bands
        .iter()
        .enumerate()
        .map(|(bi, band)| {
            let on_band: Vec<&&LogQso> = qsos.iter().filter(|q| q.band == *band).collect();
            BandRow {
                band: band.clone(),
                qsos: on_band.len() as u32,
                dupes: on_band.iter().filter(|q| q.dupe).count() as u32,
                unique_calls: on_band.iter().map(|q| &q.call).collect::<HashSet<_>>().len()
                    as u32,
                dxcc: on_band
                    .iter()
                    .filter_map(|q| q.entity.map(|e| e.country))
                    .collect::<HashSet<_>>()
                    .len() as u32,
                cq_zones: on_band
                    .iter()
                    .map(|q| q.cq_zone())
                    .filter(|z| *z > 0)
                    .collect::<HashSet<_>>()
                    .len() as u32,
                best_hour: hours.iter().map(|h| h.per_band[bi]).max().unwrap_or(0),
                run: on_band.iter().filter(|q| q.run == Some(true)).count() as u32,
            }
        })
        .collect()
}

fn countries(qsos: &[&LogQso], band_index: &HashMap<&str, usize>) -> Vec<CountryRow> {
    let mut rows: HashMap<&'static str, (CountryRow, HashSet<&str>, f64, u32)> = HashMap::new();
    for q in qsos {
        let Some(e) = q.entity else { continue };
        let (row, calls, km_sum, km_n) = rows.entry(e.country).or_insert_with(|| {
            (
                CountryRow {
                    country: e.country,
                    prefix: e.main_prefix.trim_start_matches('*'),
                    continent: e.continent,
                    cq_zone: e.cq_zone,
                    per_band: vec![0; band_index.len()],
                    total: 0,
                    unique_calls: 0,
                    first: q.time,
                    distance_km: None,
                },
                HashSet::new(),
                0.0,
                0,
            )
        });
        row.per_band[band_index[q.band.as_str()]] += 1;
        row.total += 1;
        calls.insert(q.call.as_str());
        if let Some(d) = q.distance_km {
            *km_sum += d;
            *km_n += 1;
        }
    }
    let mut list: Vec<CountryRow> = rows
        .into_values()
        .map(|(mut row, calls, km_sum, km_n)| {
            row.unique_calls = calls.len() as u32;
            row.distance_km = (km_n > 0).then(|| km_sum / km_n as f64);
            row
        })
        .collect();
    list.sort_by(|a, b| b.total.cmp(&a.total).then(a.country.cmp(b.country)));
    list
}

fn continents(qsos: &[&LogQso], band_index: &HashMap<&str, usize>) -> Vec<ContinentRow> {
    let mut rows: HashMap<&'static str, (ContinentRow, HashSet<&str>)> = HashMap::new();
    for q in qsos {
        let Some(e) = q.entity else { continue };
        let (row, countries) = rows.entry(e.continent).or_insert_with(|| {
            (
                ContinentRow {
                    continent: e.continent,
                    per_band: vec![0; band_index.len()],
                    total: 0,
                    countries: 0,
                },
                HashSet::new(),
            )
        });
        row.per_band[band_index[q.band.as_str()]] += 1;
        row.total += 1;
        countries.insert(e.country);
    }
    let mut list: Vec<ContinentRow> = rows
        .into_values()
        .map(|(mut row, countries)| {
            row.countries = countries.len() as u32;
            row
        })
        .collect();
    list.sort_by(|a, b| b.total.cmp(&a.total));
    list
}

fn zones(qsos: &[&LogQso], band_index: &HashMap<&str, usize>) -> Vec<ZoneRow> {
    let mut rows: BTreeMap<u32, ZoneRow> = BTreeMap::new();
    for q in qsos {
        let zone = q.cq_zone();
        if zone == 0 {
            continue;
        }
        let row = rows.entry(zone).or_insert_with(|| ZoneRow {
            zone,
            per_band: vec![0; band_index.len()],
            total: 0,
            first: q.time,
        });
        row.per_band[band_index[q.band.as_str()]] += 1;
        row.total += 1;
    }
    rows.into_values().collect()
}

fn callsign_stats(qsos: &[&LogQso], report: &mut Report) {
    let mut stations: HashMap<&str, StationRow> = HashMap::new();
    let mut prefixes: HashMap<&str, u32> = HashMap::new();
    let mut unknown = HashSet::new();
    for q in qsos {
        let row = stations.entry(q.call.as_str()).or_insert_with(|| StationRow {
            call: q.call.clone(),
            country: q.country(),
            qsos: 0,
            bands: Vec::new(),
        });
        row.qsos += 1;
        if !row.bands.contains(&q.band) {
            row.bands.push(q.band.clone());
        }
        *prefixes.entry(q.prefix.as_str()).or_default() += 1;
        if q.entity.is_none() {
            unknown.insert(q.call.clone());
        }
    }

    let mut list: Vec<StationRow> = stations.into_values().collect();
    for row in &mut list {
        row.bands.sort_by_key(|b| band_rank(b));
    }
    list.sort_by(|a, b| {
        b.bands
            .len()
            .cmp(&a.bands.len())
            .then(b.qsos.cmp(&a.qsos))
            .then(a.call.cmp(&b.call))
    });

    let mut per_bands: BTreeMap<usize, u32> = BTreeMap::new();
    let mut lengths: BTreeMap<usize, u32> = BTreeMap::new();
    for row in &list {
        *per_bands.entry(row.bands.len()).or_default() += 1;
        *lengths.entry(base_call(&row.call).len()).or_default() += 1;
    }

    let mut prefixes: Vec<(String, u32)> = prefixes
        .into_iter()
        .map(|(p, n)| (p.to_string(), n))
        .collect();
    prefixes.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));

    let mut unknown: Vec<String> = unknown.into_iter().collect();
    unknown.sort();

    report.stations = list;
    report.bands_per_station = per_bands.into_iter().collect();
    report.call_lengths = lengths.into_iter().collect();
    report.prefixes = prefixes;
    report.unknown_calls = unknown;
}

fn summary(qsos: &[&LogQso], report: &Report) -> Summary {
    let first = qsos[0].time;
    let last = qsos[qsos.len() - 1].time;
    let participation_min = minute_of(last) - minute_of(first) + 1;
    let break_min: i64 = report.breaks.iter().map(|b| b.minutes).sum();
    let operating_min = (participation_min - break_min).max(1);

    let distances: Vec<(&str, f64)> = qsos
        .iter()
        .filter_map(|q| q.distance_km.map(|d| (q.call.as_str(), d)))
        .collect();
    let avg_km = (!distances.is_empty())
        .then(|| distances.iter().map(|d| d.1).sum::<f64>() / distances.len() as f64);
    let longest = distances
        .iter()
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(c, d)| (c.to_string(), *d));

    Summary {
        qsos: qsos.len() as u32,
        dupes: qsos.iter().filter(|q| q.dupe).count() as u32,
        unique_calls: report.stations.len() as u32,
        dxcc: report.countries.len() as u32,
        cq_zones: report.zones.len() as u32,
        itu_zones: qsos
            .iter()
            .filter_map(|q| q.entity.map(|e| e.itu_zone))
            .collect::<HashSet<_>>()
            .len() as u32,
        continents: report.continents.len() as u32,
        prefixes: report.prefixes.len() as u32,
        band_mults: report.band_rows.iter().map(|b| b.dxcc + b.cq_zones).sum(),
        first: Some(first),
        last: Some(last),
        participation_min,
        break_min,
        operating_min,
        avg_rate: qsos.len() as f64 * 60.0 / operating_min as f64,
        avg_km,
        longest,
        unknown_calls: report.unknown_calls.len() as u32,
        run_qsos: qsos
            .iter()
            .any(|q| q.run.is_some())
            .then(|| qsos.iter().filter(|q| q.run == Some(true)).count() as u32),
    }
}

/// CQ zone from a CQ WW exchange: the token after the RST ("59 14", "599 05 MA").
fn exchange_zone(exch: &str) -> Option<u32> {
    exch.split_whitespace()
        .nth(1)
        .and_then(|z| z.parse::<u32>().ok())
        .filter(|z| (1..=40).contains(z))
}

/// Minimum share of QSOs with a frequency not on a round 100 kHz, below which
/// the log is assumed to record band edges only (e.g. "14000").
const PRECISE_FREQ_SHARE: f64 = 0.2;

/// Mark QSOs as run or search & pounce. A run QSO shares its exact frequency
/// with the previous or next QSO by the same transmitter on the same band;
/// in S&P the frequency changes with nearly every QSO. Returns false (and
/// leaves `run` unset) when frequencies are too coarse for this.
fn estimate_run(qsos: &mut [LogQso]) -> bool {
    let freqs: Vec<Option<f64>> = qsos.iter().map(|q| q.freq.parse::<f64>().ok()).collect();
    let precise = freqs
        .iter()
        .filter(|f| f.is_some_and(|f| f % 100.0 != 0.0))
        .count();
    if qsos.is_empty() || (precise as f64) < qsos.len() as f64 * PRECISE_FREQ_SHARE {
        return false;
    }

    let mut streams: HashMap<(String, Option<String>), Vec<usize>> = HashMap::new();
    for (i, q) in qsos.iter().enumerate() {
        streams
            .entry((q.band.clone(), q.tx.clone()))
            .or_default()
            .push(i);
    }
    for stream in streams.values() {
        for (pos, &i) in stream.iter().enumerate() {
            let same = |other: Option<&usize>| {
                other.is_some_and(|&j| freqs[j].is_some() && freqs[j] == freqs[i])
            };
            let prev = pos.checked_sub(1).and_then(|p| stream.get(p));
            qsos[i].run = Some(same(prev) || same(stream.get(pos + 1)));
        }
    }
    true
}

/// Minimum QSOs for a stretch on one frequency to be listed as a run.
pub const MIN_RUN_QSOS: u32 = 10;

fn runs(qsos: &[&LogQso]) -> Vec<Run> {
    let mut open: HashMap<(&str, Option<&str>), Run> = HashMap::new();
    let mut done = Vec::new();
    for q in qsos {
        let key = (q.band.as_str(), q.tx.as_deref());
        let continues = open.get(&key).is_some_and(|r| q.run == Some(true) && r.freq == q.freq);
        if continues {
            let r = open.get_mut(&key).unwrap();
            r.end = q.time;
            r.qsos += 1;
        } else {
            if let Some(r) = open.remove(&key) {
                done.push(r);
            }
            if q.run == Some(true) {
                open.insert(key, Run {
                    band: q.band.clone(),
                    freq: q.freq.clone(),
                    tx: q.tx.clone(),
                    start: q.time,
                    end: q.time,
                    qsos: 1,
                });
            }
        }
    }
    done.extend(open.into_values());
    done.retain(|r| r.qsos >= MIN_RUN_QSOS);
    done.sort_by(|a, b| b.qsos.cmp(&a.qsos).then(a.start.cmp(&b.start)));
    done
}

/// Strip portable designators: "SP5TLS/P" -> "SP5TLS", "VP2E/K1ABC" -> "K1ABC".
pub fn base_call(call: &str) -> String {
    call.split('/')
        .max_by_key(|part| part.len())
        .unwrap_or(call)
        .to_string()
}

/// WPX prefix of a callsign: letters and digits up to and including the last
/// digit of the base call, with the usual portable rules.
pub fn wpx_prefix(call: &str) -> String {
    let call = call.to_uppercase();
    let parts: Vec<&str> = call
        .split('/')
        .filter(|p| !matches!(*p, "P" | "M" | "MM" | "AM" | "QRP" | "A" | "LH" | ""))
        .collect();
    let base = parts.iter().max_by_key(|p| p.len()).copied().unwrap_or("");
    let base_prefix = prefix_of(base);

    match parts.iter().find(|p| **p != base) {
        // K1ABC/4 -> K4
        Some(p) if p.len() == 1 && p.chars().all(|c| c.is_ascii_digit()) => {
            let without_digits = base_prefix.trim_end_matches(|c: char| c.is_ascii_digit());
            format!("{}{}", without_digits, p)
        }
        // VP2E/K1ABC -> VP2E, F/K1ABC -> F0
        Some(p) => {
            if p.chars().any(|c| c.is_ascii_digit()) {
                p.to_string()
            } else {
                format!("{}0", p)
            }
        }
        None => base_prefix,
    }
}

fn prefix_of(call: &str) -> String {
    match call.rfind(|c: char| c.is_ascii_digit()) {
        Some(i) => call[..=i].to_string(),
        None => format!("{}0", call.get(..2).unwrap_or(call)),
    }
}

/// Centre of a Maidenhead locator (4 or 6 characters).
pub fn locator_to_lat_lon(locator: &str) -> Option<(f64, f64)> {
    let chars: Vec<char> = locator.trim().to_uppercase().chars().collect();
    if chars.len() < 4 {
        return None;
    }
    let field = |c: char| ('A'..='R').contains(&c).then(|| (c as u8 - b'A') as f64);
    let digit = |c: char| c.to_digit(10).map(f64::from);
    let mut lon = field(chars[0])? * 20.0 - 180.0 + digit(chars[2])? * 2.0;
    let mut lat = field(chars[1])? * 10.0 - 90.0 + digit(chars[3])?;
    if chars.len() >= 6 {
        let sub = |c: char| ('A'..='X').contains(&c).then(|| (c as u8 - b'A') as f64);
        lon += sub(chars[4])? * 5.0 / 60.0 + 2.5 / 60.0;
        lat += sub(chars[5])? * 2.5 / 60.0 + 1.25 / 60.0;
    } else {
        lon += 1.0;
        lat += 0.5;
    }
    Some((lat, lon))
}

/// Great-circle distance (km) and initial bearing (degrees) between two points.
pub fn distance_and_bearing(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> (f64, f64) {
    let (p1, p2) = (lat1.to_radians(), lat2.to_radians());
    let dl = (lon2 - lon1).to_radians();
    let a = ((p2 - p1) / 2.0).sin().powi(2) + p1.cos() * p2.cos() * (dl / 2.0).sin().powi(2);
    let distance = 2.0 * 6371.0 * a.sqrt().asin();
    let y = dl.sin() * p2.cos();
    let x = p1.cos() * p2.sin() - p1.sin() * p2.cos() * dl.cos();
    let bearing = (y.atan2(x).to_degrees() + 360.0) % 360.0;
    (distance, bearing)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOG: &str = "START-OF-LOG: 3.0
CALLSIGN: SP5TLS
CONTEST: CQ-WW-CW
CATEGORY-OPERATOR: SINGLE-OP
CATEGORY-BAND: ALL
GRID-LOCATOR: KO02MF
QSO: 14025 CW 2023-11-25 0000 SP5TLS 599 15 W1AW 599 05
QSO: 14025 CW 2023-11-25 0001 SP5TLS 599 15 DL1ABC 599 14
QSO: 14025 CW 2023-11-25 0001 SP5TLS 599 15 W1AW 599 05
QSO: 7025 CW 2023-11-25 0005 SP5TLS 599 15 W1AW 599 05
QSO: 7025 CW 2023-11-25 0200 SP5TLS 599 15 JA1XYZ 599 25
END-OF-LOG:
";

    fn contest() -> ContestLog {
        ContestLog::from_cabrillo(&CabrilloLog::parse(LOG).unwrap())
    }

    #[test]
    fn summary_counts() {
        let r = contest().report(&ReportFilter::default());
        assert_eq!(r.summary.qsos, 5);
        assert_eq!(r.summary.dupes, 1);
        assert_eq!(r.summary.unique_calls, 3);
        assert_eq!(r.summary.dxcc, 3);
        assert_eq!(r.summary.cq_zones, 3);
        // 20m: K + DL (2 dxcc, 2 zones), 40m: K + JA (2 dxcc, 2 zones)
        assert_eq!(r.summary.band_mults, 8);
        assert_eq!(r.bands, vec!["40m", "20m"]);
    }

    #[test]
    fn breaks_and_operating_time() {
        let r = contest().report(&ReportFilter::default());
        assert_eq!(r.breaks.len(), 1);
        assert_eq!(r.breaks[0].minutes, 115);
        assert_eq!(r.summary.participation_min, 121);
        assert_eq!(r.summary.operating_min, 6);
    }

    #[test]
    fn best_rate_windows() {
        let r = contest().report(&ReportFilter::default());
        let ten = r.best_rates.iter().find(|w| w.minutes == 10).unwrap();
        assert_eq!(ten.qsos, 4);
        assert_eq!(ten.per_hour(), 24.0);
    }

    #[test]
    fn hourly_rows_include_empty_hours() {
        let r = contest().report(&ReportFilter::default());
        assert_eq!(r.hours.len(), 3);
        assert_eq!(r.hours[0].total, 4);
        assert_eq!(r.hours[1].total, 0);
        assert_eq!(r.hours[2].cumulative, 5);
        assert_eq!(r.hours[0].new_dxcc.len(), 2);
    }

    #[test]
    fn band_filter() {
        let filter = ReportFilter {
            band: Some("40m".into()),
            ..Default::default()
        };
        let r = contest().report(&filter);
        assert_eq!(r.summary.qsos, 2);
        assert_eq!(r.summary.dupes, 0);
        assert_eq!(r.countries.len(), 2);
    }

    #[test]
    fn station_from_locator_and_distance() {
        let c = contest();
        let s = c.station.as_ref().unwrap();
        assert!(s.from_locator);
        assert!((s.lat - 52.23).abs() < 0.1 && (s.lon - 21.04).abs() < 0.1);
        let w1aw = &c.qsos[0];
        assert!(w1aw.distance_km.unwrap() > 6000.0);
        let b = w1aw.bearing.unwrap();
        assert!(b > 270.0 && b < 330.0, "bearing {b}");
    }

    #[test]
    fn zones_from_cqww_exchange() {
        let c = contest();
        // W1AW sent zone 5 in the exchange; country file agrees, JA1XYZ sent 25
        assert!(c.zones_from_exchange);
        let log = CabrilloLog::parse(
            "CONTEST: CQ-WW-SSB\nQSO: 14200 PH 2025-10-25 0000 8P5A 59 8 W6ABC 59 3\n",
        )
        .unwrap();
        let c = ContestLog::from_cabrillo(&log);
        assert_eq!(c.qsos[0].cq_zone(), 3);
        let r = c.report(&ReportFilter::default());
        assert_eq!(r.zones[0].zone, 3);
    }

    #[test]
    fn run_estimate() {
        let mut lines = String::from("CONTEST: CQ-WW-CW\n");
        // ten QSOs running on 14025, then three S&P on changing frequencies
        for m in 0..10 {
            lines += &format!("QSO: 14025 CW 2025-11-29 00{:02} SP5TLS 599 15 DL{}AA 599 14\n", m, m);
        }
        for (m, f) in [(10, 14031), (11, 14044), (12, 14052)] {
            lines += &format!("QSO: {} CW 2025-11-29 00{} SP5TLS 599 15 OK{}AA 599 15\n", f, m, m % 10);
        }
        let c = ContestLog::from_cabrillo(&CabrilloLog::parse(&lines).unwrap());
        assert!(c.has_run_estimate);
        let r = c.report(&ReportFilter::default());
        assert_eq!(r.summary.run_qsos, Some(10));
        assert_eq!(r.runs.len(), 1);
        assert_eq!(r.runs[0].qsos, 10);
        assert_eq!(r.runs[0].freq, "14025");
    }

    #[test]
    fn no_run_estimate_for_band_edges() {
        let c = contest();
        // the test log only has 14025 and 7025, which are precise
        assert!(c.has_run_estimate);
        let log = CabrilloLog::parse(
            "QSO: 14000 CW 2025-11-29 0000 SP5TLS 599 15 W1AW 599 5\nQSO: 14000 CW 2025-11-29 0001 SP5TLS 599 15 K1ZZ 599 5\n",
        )
        .unwrap();
        let c = ContestLog::from_cabrillo(&log);
        assert!(!c.has_run_estimate);
        assert_eq!(c.report(&ReportFilter::default()).summary.run_qsos, None);
    }

    #[test]
    fn wpx_prefixes() {
        assert_eq!(wpx_prefix("SP5TLS"), "SP5");
        assert_eq!(wpx_prefix("3D2AG"), "3D2");
        assert_eq!(wpx_prefix("K1ABC/4"), "K4");
        assert_eq!(wpx_prefix("SP5TLS/P"), "SP5");
        assert_eq!(wpx_prefix("VP2E/K1ABC"), "VP2E");
        assert_eq!(wpx_prefix("F/K1ABC"), "F0");
        assert_eq!(wpx_prefix("RAEM"), "RA0");
    }
}
