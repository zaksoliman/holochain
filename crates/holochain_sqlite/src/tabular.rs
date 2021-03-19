use rusqlite::*;

pub trait Tabular {
    fn to_row<'a>(self) -> TableRow<'a>;
    fn from_row<'a>(row: TableRow<'a>) -> Self;
}

pub type FieldName = String;

pub struct TableRow<'a> {
    fields: HashMap<&'a FieldName, TableField>,
}

pub enum TableField {
    Simple(String),
    Index { name: String, unique: bool },
}

impl TableField {
    pub fn name(&self) -> &str {
        match self {
            Self::Simple(name) => name,
            Self::Index { name, .. } => name,
        }
    }
}

impl<'a> From<&'a str> for TableField {
    fn from(name: &'a str) -> Self {
        Self::Simple(name.to_string())
    }
}
