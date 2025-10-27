use enricher::enrich_callsign;

fn main() {
    // Test with some prefixes
    let test_prefixes = vec![
        "1A", "3B6", "4J", "K", "W", "3D2/c", "SP5TLS", "SN0K", "SW6ALL", "4U0IARU", "TC1A",
    ];

    for prefix in test_prefixes {
        if let Some(e) = enrich_callsign(prefix) {
            println!("Entity: {e:?}");
        } else {
            println!("Prefix: {} not found", prefix);
        }
    }
}
