//! Facts about types in this crate

use contrafact::*;

use crate::dht_op::DhtOp;

/// Fact: DhtOp's internal references are consistent and valid
pub struct DhtOpInternallyValid;

impl Fact<DhtOp> for DhtOpInternallyValid {
    fn constraints(&mut self) -> Constraints<DhtOp> {
        let mut cs = Constraints::new();
        // cs.add()
        cs
    }
}
