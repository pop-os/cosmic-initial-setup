pub use bitcode;

#[derive(Clone, Debug, bitcode::Decode, bitcode::Encode)]
pub struct City {
    pub name: Box<str>,
    pub alternate_names: Vec<Box<str>>,
    pub timezone: Box<str>,
    pub latitude: f64,
    pub longitude: f64,
}
