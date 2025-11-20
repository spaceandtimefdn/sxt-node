use core::iter::Sum;

use frame_support::weights::Weight;

/// A simple monad for composing functions that incur polkadot weight.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub struct Heavy<T> {
    pub out: T,
    pub weight: Weight,
}

impl<T> Heavy<T> {
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Heavy<U> {
        let Heavy { out, weight } = self;
        Heavy {
            out: f(out),
            weight,
        }
    }

    pub fn and_then<U>(self, f: impl FnOnce(T) -> Heavy<U>) -> Heavy<U> {
        self.map(f).flatten()
    }
}

impl<T> Heavy<Heavy<T>> {
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
