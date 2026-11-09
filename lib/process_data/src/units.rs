use nutype::nutype;

#[nutype(
    validate(less_or_equal = 100),
    validate(greater_or_equal = 0),
    derive(
        Debug,
        Default,
        Clone,
        Hash,
        PartialEq,
        Eq,
        Serialize,
        Deserialize,
        Copy,
        FromStr,
        Deref,
        TryFrom,
        Display,
        PartialOrd,
        Ord,
    ),
    default = 0
)]
pub struct IntegerPercentage(u8);

impl IntegerPercentage {
    #[must_use]
    pub fn fraction(self) -> f32 {
        self.into_inner() as f32 / 100.0
    }
}

#[must_use]
pub fn ns_delta_to_usage_fraction(newer_ns: u64, older_ns: u64, time_delta_ms: u64) -> Option<f32> {
    if time_delta_ms == 0 {
        None
    } else {
        let elapsed_ns = time_delta_ms * 1_000_000;
        Some(newer_ns.saturating_sub(older_ns) as f32 / elapsed_ns as f32)
    }
}

#[must_use]
pub fn cycles_delta_to_usage_fraction(
    newer_cycles: u64,
    older_cycles: u64,
    newer_total_cycles: u64,
    older_total_cycles: u64,
) -> Option<f32> {
    let cycles = newer_cycles.saturating_sub(older_cycles) as f64;
    let total_cycles = newer_total_cycles.saturating_sub(older_total_cycles) as f64;
    if total_cycles == 0.0 {
        None
    } else {
        Some((cycles / total_cycles) as f32)
    }
}
