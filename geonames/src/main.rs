use geonames::City;
use std::{
    fs,
    io::{self, BufRead},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // There are files with a threshold of 500, 1000, 5000, and 15000.
    let threshold = 5000;
    let url = format!(
        "https://download.geonames.org/export/dump/cities{}.zip",
        threshold
    );
    println!("Downloading {}", url);
    let mut response = reqwest::get(url).await?;
    let length = response.content_length().unwrap_or(0) as usize;
    let mut data = Vec::with_capacity(length);
    while let Some(chunk) = response.chunk().await? {
        data.extend_from_slice(&chunk);
        print!("\rzip: {}/{}", data.len(), length);
    }
    println!();

    let mut zip = zip::ZipArchive::new(io::Cursor::new(data))?;
    let file = zip.by_name(&format!("cities{}.txt", threshold))?;
    let bufread = io::BufReader::new(file);
    let mut cities = Vec::new();
    for line_res in bufread.lines() {
        let line = line_res?;
        let mut parts = line.split('\t');
        let Some(_id) = parts.next() else { continue };
        let Some(name) = parts.next() else { continue };
        let Some(_ascii_name) = parts.next() else {
            continue;
        };
        let Some(alternate_names) = parts.next() else {
            continue;
        };
        let Some(latitude) = parts.next() else {
            continue;
        };
        let Some(longitude) = parts.next() else {
            continue;
        };
        let Some(_feature_class) = parts.next() else {
            continue;
        };
        let Some(_feature_code) = parts.next() else {
            continue;
        };
        let Some(_country_code) = parts.next() else {
            continue;
        };
        let Some(_alternate_country_codes) = parts.next() else {
            continue;
        };
        let Some(_admin1_code) = parts.next() else {
            continue;
        };
        let Some(_admin2_code) = parts.next() else {
            continue;
        };
        let Some(_admin3_code) = parts.next() else {
            continue;
        };
        let Some(_admin4_code) = parts.next() else {
            continue;
        };
        let Some(population) = parts.next() else {
            continue;
        };
        let Some(_elevation) = parts.next() else {
            continue;
        };
        let Some(_digital_elevation_model) = parts.next() else {
            continue;
        };
        let Some(timezone) = parts.next() else {
            continue;
        };
        let Some(_modification_date) = parts.next() else {
            continue;
        };
        let city = City {
            name: Box::from(name),
            alternate_names: alternate_names.split(',').map(Box::from).collect(),
            timezone: Box::from(timezone),
            latitude: latitude.parse()?,
            longitude: longitude.parse()?,
        };
        cities.push((population.parse::<u64>()?, city));
    }

    cities.sort_by(|(pop_a, _a), (pop_b, _b)| pop_b.cmp(pop_a));

    let cities = cities
        .into_iter()
        .map(|(_pop, city)| city)
        .collect::<Vec<City>>();

    println!("cities: {}", cities.len());

    let bitcode = bitcode::encode(&cities);
    println!("bitcode: {}", bitcode.len());
    fs::write("../res/cities.bitcode-v0-6", bitcode)?;

    Ok(())
}
