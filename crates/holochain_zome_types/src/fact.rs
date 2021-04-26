//! Facts about types in this crate

use crate::{header::HeaderType, Header};
use contrafact::*;

/// Fact: Header type matches the given types
pub struct HeaderOfType(Vec<HeaderType>);

impl Fact<Header> for HeaderOfType {
    fn constraints(&mut self) -> Constraints<Header> {
        let mut cs = Constraints::new();
        let header_types = self.0.clone();
        cs.add(
            |h: &mut Header| h,
            constraint::predicate(move |h: &Header| header_types.contains(&h.header_type())),
        );
        cs
    }
}
