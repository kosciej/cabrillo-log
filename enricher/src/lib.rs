#[derive(Debug, Clone)]
pub struct Entity {
    pub main_prefix: &'static str,
    pub country: &'static str,
    pub cq_zone: u32,
    pub itu_zone: u32,
    pub continent: &'static str,
    pub latitude: f64,
    pub longitude: f64,
    pub time_offset: f64,
    pub dxcc: u32,
    pub prefixes: Vec<String>,
    pub part: Option<&'static str>, // For sub-entities like islands
}

include!(concat!(env!("OUT_DIR"), "/entities.rs"));

pub fn enrich_callsign(callsign: &str) -> Option<&'static Entity> {
    let prefixes = get_all_prefixes_descending(callsign);
    prefixes.into_iter().filter_map(|p| ENTITIES.get(&p)).next()
}

pub fn enrich_callsign2(callsign: &str) -> Option<Entity> {
    // Find the longest matching prefix
    let mut best_match: Option<&Entity> = None;
    let mut best_len = 0;

    for (entity_prefix, entity) in &ENTITIES {
        if callsign.starts_with(entity_prefix) && entity_prefix.len() > best_len {
            best_match = Some(entity);
            best_len = entity_prefix.len();
        }
    }

    best_match.cloned()
}

pub fn get_all_prefixes_descending(s: &str) -> Vec<String> {
    (1..=s.len()).rev().map(|i| s[0..i].to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enrich_prefix_correctness() {
        let test_cases = vec![
            ("1A", "1A", "Sov Mil Order of Malta", 15, 246),
            ("3B6", "3B6", "Agalega & St. Brandon", 39, 4),
            ("4J", "4J", "Azerbaijan", 21, 18),
            ("K", "K", "United States", 5, 291),
            ("W", "K", "United States", 5, 291),
            ("3D2/c", "3D2", "Fiji", 32, 176),
            ("SP5TLS", "SP", "Poland", 15, 269),
            ("SN0K", "SP", "Poland", 15, 269),
            ("SW6ALL", "SV", "Greece", 20, 236),
            ("4U0IARU", "*4U1V", "Vienna Intl Ctr", 15, 206),
            ("TC1A", "*TA1", "European Turkey", 20, 390),
        ];

        for (callsign, expected_main_prefix, expected_country, expected_cq_zone, expected_dxcc) in
            test_cases
        {
            let entity = enrich_callsign(callsign)
                .unwrap_or_else(|| panic!("No entity found for callsign: {}", callsign));
            assert_eq!(
                entity.main_prefix, expected_main_prefix,
                "Main prefix mismatch for {}",
                callsign
            );
            assert_eq!(
                entity.country, expected_country,
                "Country mismatch for {}",
                callsign
            );
            assert_eq!(
                entity.cq_zone, expected_cq_zone,
                "CQ zone mismatch for {}",
                callsign
            );
            assert_eq!(entity.dxcc, expected_dxcc, "DXCC mismatch for {}", callsign);
        }
    }
}
