use proof_of_sql::base::scalar::{MontScalar, ScalarExt};

/// Extension trait for proof-of-sql `MontScalar`s, for any additional methods and traits we may
/// need for on-chain scalar usage.
pub trait MontScalarExt: ScalarExt {
    /// Create a new MontScalar<T> from a [u8] modulus the field order. The array is expected to be
    /// in non-montgomery form.
    fn from_le_bytes_mod_order(bytes: &[u8]) -> Self;
}

impl<T> MontScalarExt for MontScalar<T>
where
    T: ark_ff::MontConfig<4>,
{
    fn from_le_bytes_mod_order(bytes: &[u8]) -> Self {
        Self::from_le_bytes_mod_order(bytes)
    }
}
