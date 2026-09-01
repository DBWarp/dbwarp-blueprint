/// Round `value` to the nearest multiple of `bucket` without overflowing.
///
/// Values exactly halfway between two buckets round up. When the mathematical
/// upper bucket is greater than `u64::MAX`, the result is the greatest complete
/// bucket that fits in `u64`. A zero bucket leaves the value unchanged.
pub fn round_to_bucket(value: u64, bucket: u64) -> u64 {
    if bucket == 0 {
        return value;
    }

    let quotient = value / bucket;
    let remainder = value % bucket;
    let round_up_threshold = bucket - (bucket / 2);
    if remainder >= round_up_threshold {
        quotient
            .checked_add(1)
            .and_then(|rounded| rounded.checked_mul(bucket))
            .unwrap_or_else(|| (u64::MAX / bucket) * bucket)
    } else {
        // `quotient` came from dividing a u64 by `bucket`, so this product is
        // always representable.
        quotient * bucket
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_half_up_rounding_is_unchanged() {
        assert_eq!(round_to_bucket(149, 100), 100);
        assert_eq!(round_to_bucket(150, 100), 200);
        assert_eq!(round_to_bucket(151, 100), 200);
        assert_eq!(round_to_bucket(2, 3), 3);
        assert_eq!(round_to_bucket(1, 3), 0);
        assert_eq!(round_to_bucket(17, 0), 17);
    }

    #[test]
    fn maximum_values_never_overflow_or_wrap() {
        assert_eq!(round_to_bucket(u64::MAX, 1), u64::MAX);
        assert_eq!(round_to_bucket(u64::MAX, 100), (u64::MAX / 100) * 100);
        assert_eq!(round_to_bucket(u64::MAX, u64::MAX), u64::MAX);
        assert_eq!(round_to_bucket(u64::MAX - 1, u64::MAX), u64::MAX);
    }
}
