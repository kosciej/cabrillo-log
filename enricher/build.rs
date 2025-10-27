use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("entities.rs");

    // let url = "https://www.country-files.com/bigcty/cty.csv";
    // let content = match reqwest::blocking::get(url) {
    //     Ok(response) => {
    //         if response.status().is_success() {
    //             match response.text() {
    //                 Ok(text) => {
    //                     println!("cargo:warning=Downloaded new CSV from {}", url);
    //                     text
    //                 }
    //                 Err(_) => {
    //                     println!("cargo:warning=Failed to read downloaded CSV, using local version");
    //                     fs::read_to_string("data/cty.csv").unwrap()
    //                 }
    //             }
    //         } else {
    //             println!("cargo:warning=Failed to download CSV from {}, using local version", url);
    //             fs::read_to_string("data/cty.csv").unwrap()
    //         }
    //     }
    //     Err(_) => {
    //         println!("cargo:warning=Network error downloading CSV from {}, using local version", url);
    //         fs::read_to_string("data/cty.csv").unwrap()
    //     }
    // };

    let content = fs::read_to_string("data/cty.csv").unwrap();
    let mut entities_map = HashMap::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 10 {
            continue;
        }

        let main_prefix = parts[0];
        let country = parts[1];
        let dxcc: u32 = parts[2].parse().unwrap();
        let continent = parts[3];
        let cq_zone: u32 = parts[4].parse().unwrap();
        let itu_zone: u32 = parts[5].parse().unwrap();
        let latitude: f64 = parts[6].parse().unwrap();
        let longitude: f64 = parts[7].parse().unwrap();
        let time_offset: f64 = parts[8].parse().unwrap();

        let mut prefixes = vec![];
        let prefix_str = parts[9..].join(",");
        let prefix_str = prefix_str.trim_end_matches(';');
        for prefix in prefix_str.split_whitespace() {
            let prefix = prefix.trim();
            if !prefix.is_empty() {
                let clean_prefix = prefix
                    .chars()
                    .filter(|c| c.is_alphanumeric())
                    .collect::<String>();
                prefixes.push(clean_prefix);
            }
        }

        let mut entity_country = country.to_string();
        let mut part = None;
        if country.contains('/') {
            let parts: Vec<&str> = country.split('/').collect();
            if parts.len() == 2 {
                entity_country = parts[0].to_string();
                part = Some(parts[1].to_string());
            }
        }

        let entity = format!(
            "Entity {{
                main_prefix: \"{}\",
                country: \"{}\",
                cq_zone: {},
                itu_zone: {},
                continent: \"{}\",
                latitude: {:.1},
                longitude: {:.1},
                time_offset: {:.1},
                dxcc: {},
                prefixes: vec![],
                part: {},
            }}",
            main_prefix,
            entity_country,
            cq_zone,
            itu_zone,
            continent,
            latitude,
            longitude,
            time_offset,
            dxcc,
            match &part {
                Some(p) => format!("Some(\"{}\")", p),
                None => "None".to_string(),
            }
        );

        for prefix in prefixes {
            let clean_prefix = prefix.trim_start_matches('=').to_string();
            // Use the last entity for duplicate prefixes
            entities_map.insert(clean_prefix, entity.clone());
        }
    }

    let mut entities = phf_codegen::Map::new();
    for (key, value) in entities_map {
        entities.entry(key, &value);
    }

    let phf_code = format!(
        "static ENTITIES: phf::Map<&'static str, Entity> = {};",
        entities.build()
    );

    fs::write(&dest_path, phf_code).unwrap();

    println!("cargo:rerun-if-changed=data/cty.csv");
}
