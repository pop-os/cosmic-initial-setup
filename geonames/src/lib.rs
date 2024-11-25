pub use bitcode;

#[derive(Clone, Debug, bitcode::Decode, bitcode::Encode)]
pub struct City {
    pub name: String,
    pub alternate_names: Vec<String>,
    pub latitude: f64,
    pub longitude: f64,
    pub population: u64,
    pub timezone: String,
}
