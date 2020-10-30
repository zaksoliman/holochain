//! Types to support tabular data dumps for Holochain databases

pub use csv::StringRecord as Record;

/// A data type which can be represented as tabular (CSV) data
pub trait Tabular {
    /// Generate the tabular form of this data
    fn tabular(&self) -> Record;
}
