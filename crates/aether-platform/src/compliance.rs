#[derive(Debug)]
pub enum StoreRegion {
    Global,
    Europe,
    NorthAmerica,
    Asia,
}

#[derive(Debug)]
pub struct StoreCompliance {
    pub platform: String,
    pub region: StoreRegion,
    pub age_rating: String,
    pub certifications: Vec<String>,
}

