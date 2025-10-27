//! # Stats Library
//!
//! A Rust library for statistical analysis of amateur radio QSO logs in Cabrillo format.
//! Provides in-memory SQL database-backed statistics with WASM compatibility.
//!
//! ## Features
//! - Parse and enrich Cabrillo QSO data
//! - In-memory SQLite database for efficient querying
//! - Statistical analysis including time intervals, distributions, and time-series
//! - WASM-compatible for web applications
//!
//! ## Example
//! ```rust
//! use cabrillo_log::CabrilloLog;
//! use stats::QsoStats;
//!
//! let log = CabrilloLog::parse("START-OF-LOG: 3.0\nQSO: 14000 CW 2023-10-01 1200 N1MM 599 001 W1AW 599 001\nEND-OF-LOG:").unwrap();
//! let mut stats = QsoStats::new(log.qsos).unwrap();
//!
//! // Get total QSO count
//! let total = stats.total_qso_count(None).unwrap();
//! println!("Total QSOs: {}", total);
//!
//! // Get QSOs per band
//! let per_band = stats.qso_per_band(None).unwrap();
//! println!("QSOs per band: {:?}", per_band);
//! ```

use cabrillo_log::QSO;
use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use enricher::enrich_callsign;
use gluesql::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Map frequency string to ham radio band name
pub fn frequency_to_band(freq_str: &str) -> String {
    // Parse frequency as float (MHz)
    let freq_khz = match freq_str.parse::<f64>() {
        Ok(f) => f,
        Err(_) => return "Unknown".to_string(),
    };

    // Convert to Hz for easier comparison
    let freq_hz = freq_khz * 1_000.0;

    match freq_hz {
        f if (1_800_000.0..=2_000_000.0).contains(&f) => "160m".to_string(),
        f if (3_500_000.0..=4_000_000.0).contains(&f) => "80m".to_string(),
        f if (7_000_000.0..=7_300_000.0).contains(&f) => "40m".to_string(),
        f if (10_100_000.0..=10_150_000.0).contains(&f) => "30m".to_string(),
        f if (14_000_000.0..=14_350_000.0).contains(&f) => "20m".to_string(),
        f if (18_068_000.0..=18_168_000.0).contains(&f) => "17m".to_string(),
        f if (21_000_000.0..=21_450_000.0).contains(&f) => "15m".to_string(),
        f if (24_890_000.0..=24_990_000.0).contains(&f) => "12m".to_string(),
        f if (28_000_000.0..=29_700_000.0).contains(&f) => "10m".to_string(),
        f if (50_000_000.0..=54_000_000.0).contains(&f) => "6m".to_string(),
        f if (70_000_000.0..=70_500_000.0).contains(&f) => "4m".to_string(),
        f if (144_000_000.0..=148_000_000.0).contains(&f) => "2m".to_string(),
        f if (420_000_000.0..=450_000_000.0).contains(&f) => "70cm".to_string(),
        _ => "Unknown".to_string(),
    }
}

/// Errors that can occur during statistics operations.
#[derive(Debug, Clone, PartialEq)]
pub enum StatsError {
    DatabaseError(String),
    EnrichmentError(String),
    InvalidFilter(String),
    NoData(String),
}

impl std::fmt::Display for StatsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StatsError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
            StatsError::EnrichmentError(msg) => write!(f, "Enrichment error: {}", msg),
            StatsError::InvalidFilter(msg) => write!(f, "Invalid filter: {}", msg),
            StatsError::NoData(msg) => write!(f, "No data: {}", msg),
        }
    }
}

impl std::error::Error for StatsError {}

impl From<gluesql::prelude::Error> for StatsError {
    fn from(err: gluesql::prelude::Error) -> Self {
        StatsError::DatabaseError(err.to_string())
    }
}

/// Internal enriched QSO representation with additional metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichedQso {
    pub id: i64,
    pub timestamp: DateTime<Utc>,
    pub freq: String,
    pub mode: String,
    pub sent_call: String,
    pub rcvd_call: String,
    pub country: Option<String>,
    pub cq_zone: Option<u32>,
    pub itu_zone: Option<u32>,
    pub continent: Option<String>,
    pub dxcc: Option<u32>,
    pub band_name: String,
}

/// Filter options for statistics queries.
#[derive(Debug, Clone, Default)]
pub struct QsoFilter {
    pub band: Option<String>,
    pub country: Option<String>,
    pub cq_zone: Option<u32>,
    pub itu_zone: Option<u32>,
    pub mode: Option<String>,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
}

/// Filter options for statistics queries.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct QSOByBand {
    pub item: String,
    pub count160m: u32,
    pub count80m: u32,
    pub count40m: u32,
    pub count20m: u32,
    pub count15m: u32,
    pub count10m: u32,
    pub count6m: u32,
    pub total: u32,
}

/// Time interval statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeIntervalStats {
    pub min_minutes: f64,
    pub max_minutes: f64,
    pub avg_minutes: f64,
    pub count: usize,
}

/// Time-series QSO frequency data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesPoint {
    pub timestamp: DateTime<Utc>,
    pub count: u32,
}

/// Main statistics analyzer for QSO data.
pub struct QsoStats {
    glue: Glue<MemoryStorage>,
}

impl QsoStats {
    /// Create a new QsoStats instance from a vector of QSOs.
    ///
    /// This will enrich the QSO data with country/zone information and store
    /// everything in an in-memory database.
    pub fn new(qsos: Vec<QSO>) -> Result<Self, StatsError> {
        let mut glue = Glue::new(MemoryStorage::default());

        // Create tables
        Self::create_tables(&mut glue)?;

        // Enrich and insert QSOs
        Self::insert_qsos(&mut glue, qsos)?;

        Ok(QsoStats { glue })
    }

    /// Create the database schema.
    fn create_tables(glue: &mut Glue<MemoryStorage>) -> Result<(), StatsError> {
        let sql = "
            CREATE TABLE qsos (
                id INTEGER,
                timestamp TEXT,
                band TEXT,
                band_name TEXT,
                mode TEXT,
                sent_call TEXT,
                rcvd_call TEXT,
                country TEXT,
                cq_zone INTEGER,
                itu_zone INTEGER,
                continent TEXT,
                dxcc INTEGER
            );
        ";

        futures::executor::block_on(glue.execute(sql))?;
        Ok(())
    }

    /// Enrich and insert QSOs into the database.
    fn insert_qsos(glue: &mut Glue<MemoryStorage>, qsos: Vec<QSO>) -> Result<(), StatsError> {
        for (id, qso) in qsos.into_iter().enumerate() {
            let enriched = Self::enrich_qso(qso)?;
            let sql = format!(
                "INSERT INTO qsos VALUES (
                    {}, '{}', '{}', '{}', '{}', '{}', '{}', '{}', {}, {}, '{}', {}
                )",
                id,
                enriched.timestamp.to_rfc3339(),
                enriched.freq,
                enriched.band_name,
                enriched.mode,
                enriched.sent_call,
                enriched.rcvd_call,
                enriched.country.unwrap_or_default(),
                enriched.cq_zone.unwrap_or(0),
                enriched.itu_zone.unwrap_or(0),
                enriched.continent.unwrap_or_default(),
                enriched.dxcc.unwrap_or(0)
            );
            futures::executor::block_on(glue.execute(&sql))?;
        }

        Ok(())
    }

    /// Enrich a single QSO with country/zone data.
    fn enrich_qso(qso: QSO) -> Result<EnrichedQso, StatsError> {
        // Combine date and time into a timestamp
        let datetime = NaiveDateTime::new(qso.date, qso.time);
        let timestamp = Utc.from_utc_datetime(&datetime);

        // Enrich received callsign
        let entity = enrich_callsign(&qso.rcvd_call);

        // Map frequency to band name
        let band_name = frequency_to_band(&qso.freq);

        let enriched = EnrichedQso {
            id: 0, // Will be set by database
            timestamp,
            freq: qso.freq,
            band_name,
            mode: qso.mode,
            sent_call: qso.sent_call,
            rcvd_call: qso.rcvd_call,
            country: entity.map(|e| e.country.to_string()),
            cq_zone: entity.map(|e| e.cq_zone),
            itu_zone: entity.map(|e| e.itu_zone),
            continent: entity.map(|e| e.continent.to_string()),
            dxcc: entity.map(|e| e.dxcc),
        };

        Ok(enriched)
    }

    /// Get total QSO count with optional filters.
    pub fn total_qso_count(&mut self, filter: Option<&QsoFilter>) -> Result<u64, StatsError> {
        let (where_clause, _params) = self.build_filter_clause(filter);

        let sql = format!("SELECT COUNT(*) FROM qsos{}", where_clause);
        let result = futures::executor::block_on(self.glue.execute(&sql))?;

        if let Some(payload) = result.first() {
            match payload {
                gluesql::prelude::Payload::Select { labels: _, rows } => {
                    if let Some(row) = rows.first() {
                        if let gluesql::prelude::Value::I64(count) = &row[0] {
                            Ok(*count as u64)
                        } else {
                            Err(StatsError::DatabaseError(
                                "Unexpected result type".to_string(),
                            ))
                        }
                    } else {
                        Ok(0)
                    }
                }
                _ => Err(StatsError::DatabaseError(
                    "Unexpected query result".to_string(),
                )),
            }
        } else {
            Ok(0)
        }
    }

    /// Get time interval statistics between consecutive QSOs.
    pub fn time_interval_stats(
        &mut self,
        filter: Option<&QsoFilter>,
    ) -> Result<TimeIntervalStats, StatsError> {
        let (where_clause, _params) = self.build_filter_clause(filter);

        let sql = format!(
            "SELECT timestamp FROM qsos{} ORDER BY timestamp",
            where_clause
        );
        let result = futures::executor::block_on(self.glue.execute(&sql))?;

        let timestamps: Vec<DateTime<Utc>> = if let Some(payload) = result.first() {
            match payload {
                gluesql::prelude::Payload::Select { labels: _, rows } => rows
                    .iter()
                    .filter_map(|row| {
                        if let gluesql::prelude::Value::Str(ts_str) = &row[0] {
                            DateTime::parse_from_rfc3339(ts_str)
                                .ok()
                                .map(|dt| dt.with_timezone(&Utc))
                        } else {
                            None
                        }
                    })
                    .collect(),
                _ => {
                    return Err(StatsError::DatabaseError(
                        "Unexpected query result".to_string(),
                    ))
                }
            }
        } else {
            Vec::new()
        };

        if timestamps.len() < 2 {
            return Err(StatsError::NoData(
                "Need at least 2 QSOs for interval statistics".to_string(),
            ));
        }

        let mut intervals = Vec::new();
        for i in 1..timestamps.len() {
            let diff = timestamps[i].signed_duration_since(timestamps[i - 1]);
            let minutes = diff.num_minutes() as f64;
            intervals.push(minutes);
        }

        let min_minutes = intervals.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_minutes = intervals.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let avg_minutes = intervals.iter().sum::<f64>() / intervals.len() as f64;

        Ok(TimeIntervalStats {
            min_minutes,
            max_minutes,
            avg_minutes,
            count: intervals.len(),
        })
    }

    /// Get QSO count per band.
    pub fn qso_per_band(
        &mut self,
        filter: Option<&QsoFilter>,
    ) -> Result<Vec<(String, u32)>, StatsError> {
        self.group_by_column("band_name", filter)
    }

    /// Get QSO count per country.
    pub fn qso_per_country(
        &mut self,
        filter: Option<&QsoFilter>,
    ) -> Result<Vec<(String, u32)>, StatsError> {
        self.group_by_column("country", filter)
    }

    /// Get QSO count per continent.
    pub fn qso_per_continent(
        &mut self,
        filter: Option<&QsoFilter>,
    ) -> Result<Vec<(String, u32)>, StatsError> {
        self.group_by_column("continent", filter)
    }

    /// Get QSO count per mode.
    pub fn qso_per_mode(
        &mut self,
        filter: Option<&QsoFilter>,
    ) -> Result<Vec<(String, u32)>, StatsError> {
        self.group_by_column("mode", filter)
    }

    /// Get QSO count per CQ zone.
    pub fn qso_per_cq_zone(
        &mut self,
        filter: Option<&QsoFilter>,
    ) -> Result<HashMap<u32, u32>, StatsError> {
        let (where_clause, _params) = self.build_filter_clause(filter);

        let sql = format!(
            "SELECT cq_zone, COUNT(*) FROM qsos{} WHERE cq_zone > 0 GROUP BY cq_zone",
            where_clause
        );
        let result = futures::executor::block_on(self.glue.execute(&sql))?;

        let mut result_map = HashMap::new();
        if let Some(payload) = result.first() {
            match payload {
                gluesql::prelude::Payload::Select { labels: _, rows } => {
                    for row in rows {
                        if let (
                            gluesql::prelude::Value::I64(zone),
                            gluesql::prelude::Value::I64(count),
                        ) = (&row[0], &row[1])
                        {
                            result_map.insert(*zone as u32, *count as u32);
                        }
                    }
                }
                _ => {
                    return Err(StatsError::DatabaseError(
                        "Unexpected query result".to_string(),
                    ))
                }
            }
        }

        Ok(result_map)
    }

    /// Get QSO count per country and band.
    pub fn qso_per_country_band(
        &mut self,
        filter: Option<&QsoFilter>,
    ) -> Result<Vec<QSOByBand>, StatsError> {
        let (where_clause, _params) = self.build_filter_clause(filter);

        let sql = format!(
            "SELECT country, 
            SUM(CASE WHEN band_name = '160m' THEN 1 ELSE 0 END) AS b160m,
            SUM(CASE WHEN band_name = '80m' THEN 1 ELSE 0 END) AS b80m,
            SUM(CASE WHEN band_name = '40m' THEN 1 ELSE 0 END) AS b40m,
            SUM(CASE WHEN band_name = '20m' THEN 1 ELSE 0 END) AS b20m,
            SUM(CASE WHEN band_name = '15m' THEN 1 ELSE 0 END) AS b15m,
            SUM(CASE WHEN band_name = '10m' THEN 1 ELSE 0 END) AS b10m,
            SUM(CASE WHEN band_name = '6m' THEN 1 ELSE 0 END) AS b6m,
            COUNT(*) as total
            FROM qsos{} WHERE country != '' AND band_name != '' GROUP BY country ORDER BY COUNT(*) desc",
            where_clause
        );
        let result = futures::executor::block_on(self.glue.execute(&sql))?;

        let mut result_vec: Vec<QSOByBand> = Vec::new();
        if let Some(payload) = result.first() {
            match payload {
                gluesql::prelude::Payload::Select { labels: _, rows } => {
                    for row in rows {
                        println!("{:?}", row);
                        if let (
                            gluesql::prelude::Value::Str(country),
                            gluesql::prelude::Value::I64(b160m),
                            gluesql::prelude::Value::I64(b80m),
                            gluesql::prelude::Value::I64(b40m),
                            gluesql::prelude::Value::I64(b20m),
                            gluesql::prelude::Value::I64(b15m),
                            gluesql::prelude::Value::I64(b10m),
                            gluesql::prelude::Value::I64(b6m),
                            gluesql::prelude::Value::I64(total),
                        ) = (
                            &row[0], &row[1], &row[2], &row[3], &row[4], &row[5], &row[6], &row[7],
                            &row[8],
                        ) {
                            let qso_by_band = QSOByBand {
                                item: country.to_string(),
                                count160m: *b160m as u32,
                                count80m: *b80m as u32,
                                count40m: *b40m as u32,
                                count20m: *b20m as u32,
                                count15m: *b15m as u32,
                                count10m: *b10m as u32,
                                count6m: *b6m as u32,
                                total: *total as u32,
                            };
                            result_vec.push(qso_by_band);
                        }
                    }
                }
                _ => {
                    return Err(StatsError::DatabaseError(
                        "Unexpected query result".to_string(),
                    ))
                }
            }
        }

        Ok(result_vec)
    }

    /// Get time-series QSO frequency (hourly).
    pub fn time_series_qso_frequency(
        &mut self,
        filter: Option<&QsoFilter>,
    ) -> Result<Vec<TimeSeriesPoint>, StatsError> {
        let (where_clause, _params) = self.build_filter_clause(filter);

        let sql = format!(
            "SELECT timestamp, COUNT(*) FROM qsos{} GROUP BY timestamp ORDER BY timestamp",
            where_clause
        );
        let result = futures::executor::block_on(self.glue.execute(&sql))?;

        let mut result_vec = Vec::new();
        if let Some(payload) = result.first() {
            match payload {
                gluesql::prelude::Payload::Select { labels: _, rows } => {
                    for row in rows {
                        if let (
                            gluesql::prelude::Value::Str(ts_str),
                            gluesql::prelude::Value::I64(count),
                        ) = (&row[0], &row[1])
                        {
                            if let Ok(timestamp) = DateTime::parse_from_rfc3339(ts_str) {
                                result_vec.push(TimeSeriesPoint {
                                    timestamp: timestamp.with_timezone(&Utc),
                                    count: *count as u32,
                                });
                            }
                        }
                    }
                }
                _ => {
                    return Err(StatsError::DatabaseError(
                        "Unexpected query result".to_string(),
                    ))
                }
            }
        }

        Ok(result_vec)
    }

    /// Helper method to group QSOs by a string column.
    fn group_by_column(
        &mut self,
        column: &str,
        filter: Option<&QsoFilter>,
    ) -> Result<Vec<(String, u32)>, StatsError> {
        let (where_clause, _params) = self.build_filter_clause(filter);

        let sql = format!(
            "SELECT {}, COUNT(*) FROM qsos{} WHERE {} != '' GROUP BY {} ORDER BY COUNT(*) DESC",
            column, where_clause, column, column
        );
        let result = futures::executor::block_on(self.glue.execute(&sql))?;

        let mut result_vec = Vec::new();
        if let Some(payload) = result.first() {
            match payload {
                gluesql::prelude::Payload::Select { labels: _, rows } => {
                    for row in rows {
                        if let (
                            gluesql::prelude::Value::Str(key),
                            gluesql::prelude::Value::I64(count),
                        ) = (&row[0], &row[1])
                        {
                            result_vec.push((key.clone(), *count as u32));
                        }
                    }
                }
                _ => {
                    return Err(StatsError::DatabaseError(
                        "Unexpected query result".to_string(),
                    ))
                }
            }
        }

        Ok(result_vec)
    }

    /// Build WHERE clause from filter.
    fn build_filter_clause(&self, filter: Option<&QsoFilter>) -> (String, Vec<String>) {
        if let Some(filter) = filter {
            let mut conditions = Vec::new();

            if let Some(ref band) = filter.band {
                conditions.push(format!("band = '{}'", band));
            }
            if let Some(ref country) = filter.country {
                conditions.push(format!("country = '{}'", country));
            }
            if let Some(cq_zone) = filter.cq_zone {
                conditions.push(format!("cq_zone = {}", cq_zone));
            }
            if let Some(itu_zone) = filter.itu_zone {
                conditions.push(format!("itu_zone = {}", itu_zone));
            }
            if let Some(ref mode) = filter.mode {
                conditions.push(format!("mode = '{}'", mode));
            }
            if let Some(start_date) = filter.start_date {
                conditions.push(format!("timestamp >= '{}'", start_date.to_rfc3339()));
            }
            if let Some(end_date) = filter.end_date {
                conditions.push(format!("timestamp <= '{}'", end_date.to_rfc3339()));
            }

            if conditions.is_empty() {
                ("".to_string(), Vec::new())
            } else {
                (format!(" WHERE {}", conditions.join(" AND ")), Vec::new())
            }
        } else {
            ("".to_string(), Vec::new())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cabrillo_log::QSO;
    use chrono::{NaiveDate, NaiveTime};

    fn create_test_qsos() -> Vec<QSO> {
        vec![
            QSO {
                freq: "14000".to_string(),
                mode: "CW".to_string(),
                date: NaiveDate::from_ymd_opt(2023, 10, 1).unwrap(),
                time: NaiveTime::from_hms_opt(12, 0, 0).unwrap(),
                sent_call: "N1MM".to_string(),
                sent_rst_exch: "599 001".to_string(),
                rcvd_call: "W1AW".to_string(),
                rcvd_rst_exch: "599 001".to_string(),
                tx: None,
            },
            QSO {
                freq: "7000".to_string(),
                mode: "PH".to_string(),
                date: NaiveDate::from_ymd_opt(2023, 10, 1).unwrap(),
                time: NaiveTime::from_hms_opt(12, 30, 0).unwrap(),
                sent_call: "N1MM".to_string(),
                sent_rst_exch: "59 001".to_string(),
                rcvd_call: "SP5TLS".to_string(),
                rcvd_rst_exch: "59 001".to_string(),
                tx: None,
            },
        ]
    }

    #[test]
    fn test_new_creates_stats() {
        let qsos = create_test_qsos();
        let mut stats = QsoStats::new(qsos).unwrap();
        assert_eq!(stats.total_qso_count(None).unwrap(), 2);
    }

    #[test]
    fn test_total_qso_count_with_filter() {
        let qsos = create_test_qsos();
        let mut stats = QsoStats::new(qsos).unwrap();

        let filter = QsoFilter {
            band: Some("14000".to_string()),
            ..Default::default()
        };
        assert_eq!(stats.total_qso_count(Some(&filter)).unwrap(), 1);

        let filter = QsoFilter {
            mode: Some("CW".to_string()),
            ..Default::default()
        };
        assert_eq!(stats.total_qso_count(Some(&filter)).unwrap(), 1);
    }

    #[test]
    fn test_qso_per_band() {
        let qsos = create_test_qsos();
        let mut stats = QsoStats::new(qsos).unwrap();

        let per_band = stats.qso_per_band(None).unwrap();
        assert!(per_band.iter().any(|x| x.0 == "20m" && x.1 == 1));
        assert!(per_band.iter().any(|x| x.0 == "40m" && x.1 == 1));
    }

    #[test]
    fn test_qso_per_country() {
        let qsos = create_test_qsos();
        let mut stats = QsoStats::new(qsos).unwrap();

        let per_country = stats.qso_per_country(None).unwrap();
        assert!(per_country.iter().any(|x| x.0 == "United States"));
        assert!(per_country.iter().any(|x| x.0 == "Poland"));
    }

    #[test]
    fn test_time_interval_stats() {
        let qsos = create_test_qsos();
        let mut stats = QsoStats::new(qsos).unwrap();

        let intervals = stats.time_interval_stats(None).unwrap();
        assert_eq!(intervals.count, 1);
        assert_eq!(intervals.min_minutes, 30.0);
        assert_eq!(intervals.max_minutes, 30.0);
        assert_eq!(intervals.avg_minutes, 30.0);
    }

    #[test]
    fn test_time_series_qso_frequency() {
        let qsos = create_test_qsos();
        let mut stats = QsoStats::new(qsos).unwrap();

        let series = stats.time_series_qso_frequency(None).unwrap();
        assert!(!series.is_empty());
        // Should have two points, one for each QSO timestamp
        assert_eq!(series.len(), 2);
        assert_eq!(series.iter().map(|p| p.count).sum::<u32>(), 2);
        // Check that timestamps are valid
        for point in &series {
            assert!(point.timestamp.to_rfc3339().contains("12:"));
        }
    }

    #[test]
    fn test_qso_per_country_band() {
        let qsos = create_test_qsos();
        let mut stats = QsoStats::new(qsos).unwrap();

        let per_country_band = stats.qso_per_country_band(None).unwrap();
        assert_eq!(per_country_band.len(), 2);

        // Find entries for United States and Poland
        let us_entry = per_country_band
            .iter()
            .find(|e| e.item == "United States")
            .unwrap();
        assert_eq!(us_entry.count20m, 1);
        assert_eq!(us_entry.count40m, 0);
        assert_eq!(us_entry.count80m, 0);
        assert_eq!(us_entry.count160m, 0);
        assert_eq!(us_entry.count15m, 0);
        assert_eq!(us_entry.count10m, 0);
        assert_eq!(us_entry.count6m, 0);
        assert_eq!(us_entry.total, 1);

        let pl_entry = per_country_band
            .iter()
            .find(|e| e.item == "Poland")
            .unwrap();
        assert_eq!(pl_entry.count20m, 0);
        assert_eq!(pl_entry.count40m, 1);
        assert_eq!(pl_entry.count80m, 0);
        assert_eq!(pl_entry.count160m, 0);
        assert_eq!(pl_entry.count15m, 0);
        assert_eq!(pl_entry.count10m, 0);
        assert_eq!(pl_entry.count6m, 0);
        assert_eq!(pl_entry.total, 1);
    }
}
