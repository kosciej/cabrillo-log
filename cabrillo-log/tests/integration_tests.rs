use cabrillo_log::CabrilloLog;
use std::fs;
use std::path::Path;

#[test]
fn test_parse_and_save_logs() {
    let data_dir = "tests/data";
    let results_dir = "tests/results";

    // Create results directory if it doesn't exist
    fs::create_dir_all(results_dir).expect("Failed to create results directory");

    // Read all files from data directory
    let entries = fs::read_dir(data_dir).expect("Failed to read data directory");

    for entry in entries {
        let entry = entry.expect("Failed to read entry");
        let path = entry.path();

        if path.is_file() {
            let filename = path.file_name().unwrap().to_str().unwrap();

            // Read the file content
            let content = fs::read_to_string(&path).expect("Failed to read file");

            // Parse the log
            let log = CabrilloLog::parse(&content).expect("Failed to parse log");

            // Generate the output string
            let output = log.to_string();

            // Write to results directory
            let result_path = Path::new(results_dir).join(filename);
            fs::write(&result_path, output).expect("Failed to write result file");

            // Optionally, validate the log
            log.validate().expect("Log validation failed");
        }
    }
}
