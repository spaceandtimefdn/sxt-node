//! Contains [`Heavy`], a monad for composing functions that incur weight.

use core::iter::Sum;

use polkadot_sdk::frame_support::weights::Weight;

/// A simple monad for composing functions that incur polkadot weight.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub struct Heavy<T> {
    /// The output of the operation.
    pub out: T,
    /// The weight of the operation.
    pub weight: Weight,
}

impl<T> Heavy<T> {
    /// Maps a `Heavy<T>` to a `Heavy<U>` by applying a function to the contained `out` field.
    ///
    /// # Examples
    /// ```
    /// use frame_support::weights::Weight;
    /// use sxt_core::heavy::Heavy;
    ///
    /// let weight = Weight::zero().set_ref_time(1).set_proof_size(2);
    ///
    /// let initial = Heavy {
    ///     out: 4,
    ///     weight,
    /// };
    /// let mapped = initial.map(|n| n.to_string());
    ///
    /// assert_eq!(mapped, Heavy { out: "4".to_string(), weight });
    /// ```
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Heavy<U> {
        let Heavy { out, weight } = self;
        Heavy {
            out: f(out),
            weight,
        }
    }

    /// Apply a function `f` to the `out` and return a `Heavy` that sums the weights.
    ///
    /// `f` takes `out: T` and returns `Heavy<U>`. Then, this returns a `Heavy<U>` whose weight is
    /// the sum of `self`'s weight and the weight returned by `f`.
    ///
    /// # Examples
    /// ```
    /// use frame_support::weights::Weight;
    /// use sxt_core::heavy::Heavy;
    ///
    /// let weight = Weight::zero().set_ref_time(1).set_proof_size(2);
    ///
    /// let initial = Heavy {
    ///     out: 4,
    ///     weight,
    /// };
    /// let composed = initial.and_then(|n| {
    ///     let weight = Weight::zero().set_ref_time(3).set_proof_size(4);
    ///     Heavy { out: n.to_string(), weight }
    /// });
    ///
    /// let expected_weight = Weight::zero().set_ref_time(4).set_proof_size(6);
    ///
    /// assert_eq!(composed, Heavy { out: "4".to_string(), weight: expected_weight });
    /// ```
    pub fn and_then<U>(self, f: impl FnOnce(T) -> Heavy<U>) -> Heavy<U> {
        self.map(f).flatten()
    }
}

impl<T> Heavy<Heavy<T>> {
    /// Flattens a nested `Heavy` by summing the weights.
    ///
    /// # Examples
    /// ```
    /// use frame_support::weights::Weight;
    /// use sxt_core::heavy::Heavy;
    ///
    /// let inner_weight = Weight::zero().set_ref_time(1).set_proof_size(2);
    /// let outer_weight = Weight::zero().set_ref_time(3).set_proof_size(4);
    ///
    /// let nested = Heavy {
    ///     out: Heavy {
    ///         out: 4,
    ///         weight: inner_weight,
    ///     },
    ///     weight: outer_weight,
    /// };
    /// let flattened = nested.flatten();
    ///
    /// let expected_weight = Weight::zero().set_ref_time(4).set_proof_size(6);
    ///
    /// assert_eq!(flattened, Heavy { out: 4, weight: expected_weight });
    /// ```
    pub fn flatten(self) -> Heavy<T> {
        let Heavy {
            out: Heavy {
                out,
                weight: weight_a,
            },
            weight: weight_b,
        } = self;

        Heavy {
            out,
            weight: weight_a.saturating_add(weight_b),
        }
    }
}

impl<T> From<T> for Heavy<T> {
    fn from(out: T) -> Self {
        Heavy {
            out,
            weight: Weight::zero(),
        }
    }
}

impl From<Weight> for Heavy<()> {
    fn from(weight: Weight) -> Self {
        Heavy { out: (), weight }
    }
}

impl From<Heavy<()>> for Weight {
    fn from(Heavy { weight, .. }: Heavy<()>) -> Weight {
        weight
    }
}

impl Sum<Heavy<()>> for Heavy<()> {
    fn sum<I: Iterator<Item = Heavy<()>>>(iter: I) -> Self {
        let weight = iter.fold(Weight::zero(), |weight, heavy| {
            weight.saturating_add(heavy.weight)
        });
        Heavy { out: (), weight }
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    proptest! {
        #[test]
        fn we_can_sum_heavys(weight_values in proptest::collection::vec((0..1000u64, 0..1000u64), 0..1000)) {
            let heavys = weight_values.iter().map(|(ref_time, proof_size)| {
                Heavy::from(Weight::zero().set_ref_time(*ref_time).set_proof_size(*proof_size))
            });

            let sum: Heavy<()> = heavys.sum();

            let (expected_ref_time, expected_proof_size) = weight_values.iter().fold((0, 0), |acc, values| (acc.0 + values.0, acc.1 + values.1));
            let expected_weight =
                Weight::zero().set_ref_time(expected_ref_time).set_proof_size(expected_proof_size);

            assert_eq!(sum, Heavy { out: (), weight: expected_weight});
        }
    }
}
