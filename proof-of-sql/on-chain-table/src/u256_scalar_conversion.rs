use primitive_types::U256;
use proof_of_sql::base::scalar::Scalar;
use snafu::Snafu;

/// Number out of bounds for target scale.
#[derive(Debug, Snafu)]
#[snafu(display("number out of bounds of target scalar"))]
pub struct OutOfScalarBounds;

/// Converts a substrate U256 (interpreted as signed) into the target proof-of-sql `Scalar`.
pub fn u256_to_scalar<S: Scalar>(value: &U256) -> Result<S, OutOfScalarBounds> {
    let is_negative = value > &(U256::MAX / 2);

    let abs_value = if is_negative {
        &(U256::MAX - value + 1)
    } else {
        value
    };

    if abs_value > &U256(S::MAX_SIGNED.into()) {
        return Err(OutOfScalarBounds);
    }

    // Convert limbs to Scalar and adjust for sign
    let scalar: S = abs_value.0.into();
    Ok(if is_negative { -scalar } else { scalar })
}

#[cfg(test)]
mod tests {
    use proof_of_sql::proof_primitive::dory::DoryScalar;
    use proof_of_sql::proof_primitive::hyperkzg::BNScalar;

    use super::*;

    fn we_can_convert_u256_to_scalar<S: Scalar>() {
        let min_signed = U256::MAX - U256(S::MAX_SIGNED.into()) + 1;
        assert_eq!(
            u256_to_scalar::<S>(&min_signed).unwrap(),
            S::MAX_SIGNED + S::ONE,
        );

        let neg_one = U256::MAX;
        assert_eq!(u256_to_scalar::<S>(&neg_one).unwrap(), -S::ONE);

        let zero = U256::zero();
        assert_eq!(u256_to_scalar::<S>(&zero).unwrap(), S::ZERO);

        let one = U256::one();
        assert_eq!(u256_to_scalar::<S>(&one).unwrap(), S::ONE);

        let max_signed = U256(S::MAX_SIGNED.into());
        assert_eq!(u256_to_scalar::<S>(&max_signed).unwrap(), S::MAX_SIGNED,);
    }

    #[test]
    fn we_can_convert_u256_to_dory_scalar() {
        we_can_convert_u256_to_scalar::<DoryScalar>()
    }

    #[test]
    fn we_can_convert_u256_to_hyper_kzg_scalar() {
        we_can_convert_u256_to_scalar::<BNScalar>()
    }

    fn we_cannot_convert_out_of_bounds_u256_to_scalar<S: Scalar>() {
        let too_positive = U256(S::MAX_SIGNED.into()) + 1;
        assert!(matches!(
            u256_to_scalar::<S>(&too_positive),
            Err(OutOfScalarBounds)
        ));

        let max_u256_signed = U256::MAX / 2;
        assert!(matches!(
            u256_to_scalar::<S>(&max_u256_signed),
            Err(OutOfScalarBounds)
        ));

        let min_u256_signed = U256::MAX / 2 + 1;
        assert!(matches!(
            u256_to_scalar::<S>(&min_u256_signed),
            Err(OutOfScalarBounds)
        ));

        let too_negative = U256::MAX - U256(S::MAX_SIGNED.into());
        assert!(matches!(
            u256_to_scalar::<S>(&too_negative),
            Err(OutOfScalarBounds)
        ));
    }

    #[test]
    fn we_cannot_convert_out_of_bounds_u256_to_dory_scalar() {
        we_cannot_convert_out_of_bounds_u256_to_scalar::<DoryScalar>()
    }

    #[test]
    fn we_cannot_convert_out_of_bounds_u256_to_hyper_kzg_scalar() {
        we_cannot_convert_out_of_bounds_u256_to_scalar::<BNScalar>()
    }
}
