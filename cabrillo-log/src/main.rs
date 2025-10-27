use cabrillo_log::{CabrilloError, CabrilloLog, QSO};
use chrono::{NaiveDate, NaiveTime};

fn main() -> Result<(), CabrilloError> {
    // Example Cabrillo content
    let content = r#"START-OF-LOG: 3.0
CALLSIGN: N1MM
CONTEST: ARRL-10
CATEGORY-OPERATOR: SINGLE-OP
CATEGORY-ASSISTED: NON-ASSISTED
CATEGORY-BAND: ALL
CATEGORY-MODE: CW
CATEGORY-POWER: LOW
CATEGORY-STATION: FIXED
CATEGORY-TRANSMITTER: ONE
CLAIMED-SCORE: 100
CLUB: Test Club
CONTEST: ARRL-10
CREATED-BY: Cabrillo Log 1.0
NAME: John Doe
ADDRESS: 123 Main St
ADDRESS: Anytown, USA
EMAIL: john@example.com
OPERATORS: N1MM
SOAPBOX: Test log
QSO: 14000 CW 2023-10-01 1200 N1MM 599 001 0 W1AW 599 001
QSO: 7000 CW 2023-10-01 1300 N1MM 599 002 0 K1ZZ 599 002
END-OF-LOG: 3.0
"#;

    // Parse the log
    let log = CabrilloLog::parse(content)?;
    println!("Parsed log: {:?}", log);

    // Validate the log
    log.validate()?;

    // Generate string using Display trait
    println!("Generated log:\n{}", log);

    // Create a new QSO
    let qso = QSO {
        freq: "21000".to_string(),
        mode: "CW".to_string(),
        date: NaiveDate::from_ymd_opt(2023, 10, 2).unwrap(),
        time: NaiveTime::from_hms_opt(14, 0, 0).unwrap(),
        sent_call: "N1MM".to_string(),
        sent_rst_exch: "599 003".to_string(),
        tx: Some("0".to_string()),
        rcvd_call: "W2XX".to_string(),
        rcvd_rst_exch: "599 003".to_string(),
    };

    // Add to log
    let mut new_log = log.clone();
    new_log.qsos.push(qso);

    // Output new log using Display trait
    println!("New log:\n{}", new_log);

    Ok(())
}
