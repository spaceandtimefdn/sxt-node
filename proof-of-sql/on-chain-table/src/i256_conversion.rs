use arrow::datatypes::i256;
use primitive_types::U256;

/// Convert arrow i256 to bnum I256
pub fn arrow_i256_to_u256(value: i256) -> U256 {
    U256::from_little_endian(&value.to_le_bytes())
}

/// Convert bnum I256 to arrow i256
pub fn u256_to_arrow_i256(value: U256) -> i256 {
    let mut buffer = [0u8; 32];

    value.to_little_endian(&mut buffer);

    i256::from_le_bytes(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn we_can_convert_arrow_to_bnum_i256() {
        assert_eq!(arrow_i256_to_u256(i256::MIN), U256::MAX / 2 + 1);
        assert_eq!(arrow_i256_to_u256(-i256::ONE), U256::MAX);
        assert_eq!(arrow_i256_to_u256(i256::ZERO), U256::zero());
        assert_eq!(arrow_i256_to_u256(i256::ONE), U256::one());
        assert_eq!(arrow_i256_to_u256(i256::MAX), U256::MAX / 2);
    }

    #[test]
    fn we_can_convert_bnum_to_arrow_i256() {
        assert_eq!(u256_to_arrow_i256(U256::MAX / 2 + 1), i256::MIN);
        assert_eq!(u256_to_arrow_i256(U256::MAX), -i256::ONE);
        assert_eq!(u256_to_arrow_i256(U256::zero()), i256::ZERO);
        assert_eq!(u256_to_arrow_i256(U256::one()), i256::ONE);
        assert_eq!(u256_to_arrow_i256(U256::MAX / 2), i256::MAX);
    }
}
