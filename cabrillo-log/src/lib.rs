//! # Cabrillo Log Library
//!
//! A Rust library for handling Cabrillo log format used in amateur radio contests.
//! Supports parsing and writing Cabrillo files, validation, and common use cases.
//!
//! ## Features
//! - Parse Cabrillo files into structured data
//! - Generate Cabrillo files from data structures
//! - Validate log entries
//! - Support for various contest types
//! - Error handling for malformed files
//!
//! ## Example
//! ```rust
//! use cabrillo_log::{CabrilloLog, QSO};
//!
//! let log = CabrilloLog::parse("START-OF-LOG: 3.0\nCALLSIGN: N1MM\nCONTEST: ARRL-10\nQSO: 14000 CW 2023-10-01 1200 N1MM 599 001 W1AW 599 001 0\nEND-OF-LOG: 3.0\n").unwrap();
//! println!("{}", log);  // Use Display trait instead of to_string
//! ```

use chrono::{NaiveDate, NaiveTime};
use std::collections::HashMap;
use std::fmt;

/// Represents a Cabrillo log file, containing headers and QSOs.
#[derive(Debug, Clone, PartialEq)]
pub struct CabrilloLog {
    pub headers: HashMap<String, String>,
    pub qsos: Vec<QSO>,
}

/// Represents a single QSO (contact) in the log.
#[derive(Debug, Clone, PartialEq)]
pub struct QSO {
    pub freq: String, // Frequency or band
    pub mode: String, // Mode like CW, PH
    pub date: NaiveDate,
    pub time: NaiveTime,
    pub sent_call: String,
    pub sent_rst_exch: String, // Combined RST and EXCH
    pub rcvd_call: String,
    pub rcvd_rst_exch: String, // Combined RST and EXCH
    pub tx: Option<String>,    // Transmitter ID, 0 or 1, optional
}

impl fmt::Display for CabrilloLog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "START-OF-LOG: 3.0")?;
        let mut keys: Vec<_> = self.headers.keys().collect();
        keys.sort();
        for key in keys {
            writeln!(f, "{}: {}", key, self.headers[key])?;
        }
        for qso in &self.qsos {
            write!(
                f,
                "QSO: {} {} {} {} {:<13} {:<8} {:<13} {:<8}",
                qso.freq,
                qso.mode,
                qso.date.format("%Y-%m-%d"),
                qso.time.format("%H%M"),
                qso.sent_call,
                qso.sent_rst_exch,
                qso.rcvd_call,
                qso.rcvd_rst_exch
            )?;
            if let Some(tx) = &qso.tx {
                write!(f, " {}", tx)?;
            }
            writeln!(f)?;
        }
        writeln!(f, "END-OF-LOG:")?;
        Ok(())
    }
}

/// Errors that can occur during parsing or validation.
#[derive(Debug, Clone, PartialEq)]
pub enum CabrilloError {
    InvalidFormat(String),
    MissingRequiredField(String),
    InvalidDate(String),
    InvalidTime(String),
    InvalidCallsign(String),
    ParseError(String),
}

impl fmt::Display for CabrilloError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CabrilloError::InvalidFormat(msg) => write!(f, "Invalid format: {}", msg),
            CabrilloError::MissingRequiredField(field) => {
                write!(f, "Missing required field: {}", field)
            }
            CabrilloError::InvalidDate(date) => write!(f, "Invalid date: {}", date),
            CabrilloError::InvalidTime(time) => write!(f, "Invalid time: {}", time),
            CabrilloError::InvalidCallsign(call) => write!(f, "Invalid callsign: {}", call),
            CabrilloError::ParseError(msg) => write!(f, "Parse error: {}", msg),
        }
    }
}

impl std::error::Error for CabrilloError {}

impl CabrilloLog {
    /// Parse a Cabrillo log from a string.
    pub fn parse(content: &str) -> Result<Self, CabrilloError> {
        let mut headers = HashMap::new();
        let mut qsos = Vec::new();
        let mut in_header = true;

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue; // Skip empty lines and comments
            }
            if line.starts_with("START-OF-LOG:") || line.starts_with("END-OF-LOG:") {
                continue;
            } else if in_header {
                if line.starts_with("QSO:") {
                    in_header = false;
                    let qso = Self::parse_qso_line(line)?;
                    qsos.push(qso);
                } else if line.starts_with("X-QSO:") {
                    in_header = false;
                    // Ignore X-QSO
                } else if let Some((key, value)) = line.split_once(':') {
                    headers.insert(key.trim().to_string(), value.trim().to_string());
                }
            } else if line.starts_with("QSO:") {
                let qso = Self::parse_qso_line(line)?;
                qsos.push(qso);
            } else if line.starts_with("X-QSO:") {
                // Ignore X-QSO lines as per spec
                continue;
            }
        }

        Ok(CabrilloLog { headers, qsos })
    }

    /// Parse a Cabrillo log from a file.
    pub fn parse_from_file(path: &str) -> Result<Self, CabrilloError> {
        let content =
            std::fs::read_to_string(path).map_err(|e| CabrilloError::ParseError(e.to_string()))?;
        Self::parse(&content)
    }

    /// Parse a single QSO line.
    fn parse_qso_line(line: &str) -> Result<QSO, CabrilloError> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 10 || parts[0] != "QSO:" {
            return Err(CabrilloError::InvalidFormat("Invalid QSO line".to_string()));
        }

        let freq = parts[1].to_string();
        let mode = parts[2].to_string();
        let date = NaiveDate::parse_from_str(parts[3], "%Y-%m-%d")
            .map_err(|_| CabrilloError::InvalidDate(parts[3].to_string()))?;
        let time = NaiveTime::parse_from_str(parts[4], "%H%M")
            .map_err(|_| CabrilloError::InvalidTime(parts[4].to_string()))?;
        let sent_call = parts[5].to_string();

        // Find the received callsign (first valid callsign after sent_call)
        let mut rcvd_call_index = 6;
        while rcvd_call_index < parts.len() && !is_valid_callsign(parts[rcvd_call_index]) {
            rcvd_call_index += 1;
        }
        if rcvd_call_index >= parts.len() {
            return Err(CabrilloError::InvalidFormat(
                "No valid received callsign found".to_string(),
            ));
        }
        let rcvd_call = parts[rcvd_call_index].to_string();

        let sent_rst_exch = if rcvd_call_index > 6 {
            parts[6..rcvd_call_index].join(" ")
        } else {
            "".to_string()
        };

        // Now, parse the received part
        let rcvd_start = rcvd_call_index + 1;
        if rcvd_start >= parts.len() {
            return Err(CabrilloError::InvalidFormat(
                "Missing received RST/EXCH".to_string(),
            ));
        }

        let tx: Option<String>;
        let rcvd_rst_exch: String;

        if parts[parts.len() - 1] == "0" || parts[parts.len() - 1] == "1" {
            tx = Some(parts[parts.len() - 1].to_string());
            rcvd_rst_exch = if rcvd_start < parts.len() - 1 {
                parts[rcvd_start..parts.len() - 1].join(" ")
            } else {
                "".to_string()
            };
        } else {
            tx = None;
            rcvd_rst_exch = parts[rcvd_start..].join(" ");
        }

        Ok(QSO {
            freq,
            mode,
            date,
            time,
            sent_call,
            sent_rst_exch,
            rcvd_call,
            rcvd_rst_exch,
            tx,
        })
    }

    /// Validate the log.
    pub fn validate(&self) -> Result<(), CabrilloError> {
        for qso in &self.qsos {
            Self::validate_qso(qso)?;
        }

        Ok(())
    }

    /// Validate a single QSO.
    fn validate_qso(qso: &QSO) -> Result<(), CabrilloError> {
        if qso.sent_call.is_empty() || !is_valid_callsign(&qso.sent_call) {
            return Err(CabrilloError::InvalidCallsign(qso.sent_call.clone()));
        }
        if qso.rcvd_call.is_empty() || !is_valid_callsign(&qso.rcvd_call) {
            return Err(CabrilloError::InvalidCallsign(qso.rcvd_call.clone()));
        }
        if !is_valid_band(&qso.freq) {
            return Err(CabrilloError::InvalidFormat(format!(
                "Invalid band/freq: {}",
                qso.freq
            )));
        }
        if !is_valid_mode(&qso.mode) {
            return Err(CabrilloError::InvalidFormat(format!(
                "Invalid mode: {}",
                qso.mode
            )));
        }
        if let Some(tx) = &qso.tx
            && tx != "0"
            && tx != "1"
        {
            return Err(CabrilloError::InvalidFormat(format!(
                "Invalid transmitter: {}",
                tx
            )));
        }
        Ok(())
    }
}

/// Check if a string is a valid amateur radio callsign (basic check).
fn is_valid_callsign(call: &str) -> bool {
    let is_ascii = call.is_ascii();
    let has_digits = call.chars().any(|c| c.is_ascii_digit());
    let has_letters = call.chars().any(|c| c.is_ascii_alphabetic());
    is_ascii && has_digits && has_letters && call.len() > 2
}

/// Check if a string is a valid band or frequency.
fn is_valid_band(band: &str) -> bool {
    let valid_bands = [
        "160", "80", "40", "20", "15", "10", "6", "2", "222", "432", "902", "1.2G", "2.3G", "3.4G",
        "5.7G", "10G", "24G", "47G", "75G", "122G", "134G", "241G", "LIGHT", "50", "70", "144",
    ];
    valid_bands.contains(&band) || band.parse::<f64>().is_ok() // Allow frequencies like 14000
}

/// Check if a string is a valid mode.
fn is_valid_mode(mode: &str) -> bool {
    let valid_modes = ["CW", "PH", "FM", "RY", "DG"];
    valid_modes.contains(&mode)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_log() {
        let content = "START-OF-LOG: 3.0\nCALLSIGN: N1MM\nQSO: 14000 CW 2023-10-01 1200 N1MM 599 001 W1AW 599 001 0\nEND-OF-LOG: 3.0\n";
        let log = CabrilloLog::parse(content).unwrap();
        assert_eq!(log.headers.get("CALLSIGN").unwrap(), "N1MM");
        assert_eq!(log.qsos.len(), 1);
        assert_eq!(log.qsos[0].freq, "14000");
        assert_eq!(log.qsos[0].mode, "CW");
        assert_eq!(log.qsos[0].tx, Some("0".to_string()));
    }

    #[test]
    fn test_parse_multi_exchange() {
        let content = "START-OF-LOG: 3.0\nQSO: 14042 CW 2023-10-01 0101 N5KO 1211 B 74 SCV VE3/KA5WSS 1071 A 74 ON 0\nEND-OF-LOG: 3.0\n";
        let log = CabrilloLog::parse(content).unwrap();
        assert_eq!(log.qsos[0].sent_rst_exch, "1211 B 74 SCV");
        assert_eq!(log.qsos[0].rcvd_rst_exch, "1071 A 74 ON");
        assert_eq!(log.qsos[0].tx, Some("0".to_string()));
    }

    #[test]
    fn test_validate_log() {
        let content = "START-OF-LOG: 3.0\nCALLSIGN: N1MM\nQSO: 14000 CW 2023-10-01 1200 N1MM 599 001 W1AW 599 001 0\nEND-OF-LOG: 3.0\n";
        let log = CabrilloLog::parse(content).unwrap();
        assert!(log.validate().is_ok());
    }

    #[test]
    fn test_invalid_callsign() {
        let content = "START-OF-LOG: 3.0\nCALLSIGN: N1MM\nQSO: 14000 CW 2023-10-01 1200 invalid 599 001 W1AW 599 001 0\nEND-OF-LOG: 3.0\n";
        let log = CabrilloLog::parse(content).unwrap();
        assert!(log.validate().is_err());
    }

    #[test]
    fn test_to_string() {
        let content = "START-OF-LOG: 3.0\nCALLSIGN: N1MM\nQSO: 14000 CW 2023-10-01 1200 N1MM 599 001 W1AW 599 001 0\nEND-OF-LOG: 3.0\n";
        let log = CabrilloLog::parse(content).unwrap();
        let output = log.to_string();
        assert!(output.contains("CALLSIGN: N1MM"));
        assert!(output.contains(
            "QSO: 14000 CW 2023-10-01 1200 N1MM          599 001  W1AW          599 001  0"
        ));
    }

    #[test]
    fn test_parse_without_tx() {
        let content = "START-OF-LOG: 3.0\nCALLSIGN: N1MM\nQSO: 14000 CW 2023-10-01 1200 N1MM 599 001 W1AW 599 001\nEND-OF-LOG: 3.0\n";
        let log = CabrilloLog::parse(content).unwrap();
        assert_eq!(log.qsos.len(), 1);
        assert_eq!(log.qsos[0].freq, "14000");
        assert_eq!(log.qsos[0].mode, "CW");
        assert_eq!(log.qsos[0].tx, None);
        assert_eq!(log.qsos[0].sent_rst_exch, "599 001");
        assert_eq!(log.qsos[0].rcvd_rst_exch, "599 001");
        let output = log.to_string();
        assert!(output.contains(
            "QSO: 14000 CW 2023-10-01 1200 N1MM          599 001  W1AW          599 001 "
        ));
    }

    #[test]
    fn test_parse_exchange_with_zero() {
        let content = "START-OF-LOG: 3.0\nQSO: 14000 CW 2023-10-01 1200 N1MM 599 001 W1AW 599 001 0\nEND-OF-LOG: 3.0\n";
        let log = CabrilloLog::parse(content).unwrap();
        assert_eq!(log.qsos[0].sent_rst_exch, "599 001");
        assert_eq!(log.qsos[0].tx, Some("0".to_string()));
    }

    #[test]
    fn test_parse_exchange_with_one() {
        let content = "START-OF-LOG: 3.0\nQSO: 14000 CW 2023-10-01 1200 N1MM 599 001 W1AW 599 001 1\nEND-OF-LOG: 3.0\n";
        let log = CabrilloLog::parse(content).unwrap();
        assert_eq!(log.qsos[0].sent_rst_exch, "599 001");
        assert_eq!(log.qsos[0].tx, Some("1".to_string()));
    }

    #[test]
    fn test_is_valid_callsign() {
        assert!(is_valid_callsign("N1MM"));
        assert!(is_valid_callsign("VE3/KA5WSS"));
        assert!(!is_valid_callsign("invalid"));
    }

    #[test]
    fn test_is_valid_band() {
        assert!(is_valid_band("20"));
        assert!(is_valid_band("14000"));
        assert!(!is_valid_band("invalid"));
    }

    #[test]
    fn test_is_valid_mode() {
        assert!(is_valid_mode("CW"));
        assert!(!is_valid_mode("invalid"));
    }
}
