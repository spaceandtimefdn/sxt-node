use on_chain_table::IndexSet;
use sqlparser::ast::Ident;

/// Returns a function that applies the mapping function `f` if the predicate `p` is `true`,
/// otherwise no-op.
pub fn map_if<T>(f: impl Fn(T) -> T, p: impl Fn(&T) -> bool) -> impl Fn(T) -> T {
    move |t| {
        if p(&t) {
            f(t)
        } else {
            t
        }
    }
}

/// Returns a function that applies the fallible mapping function `f` if the predicate `p` is `true`,
/// otherwise no-op (wrapped in `Ok`).
pub fn try_map_if<T, E>(
    f: impl Fn(T) -> Result<T, E>,
    p: impl Fn(&T) -> bool,
) -> impl Fn(T) -> Result<T, E> {
    move |t| {
        if p(&t) {
            f(t)
        } else {
            Ok(t)
        }
    }
}

/// Returns a function that returns `true` if the identifier is in the given `identifiers` set.
pub fn identifier_is_in_fn<M>(identifiers: &IndexSet<Ident>) -> impl Fn(&(Ident, M)) -> bool + '_ {
    move |(identifier, _)| identifiers.contains(identifier)
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use super::*;

    #[test]
    fn we_can_map_if_predicate() {
        [0u8, 1, 2, 3, 4, 5, 6, 7, 8, 9]
            .map(map_if(|x: u8| x + 1, |x| x % 2 == 1))
            .iter()
            .for_each(|x| assert_eq!(x % 2, 0));
    }

    #[test]
    fn we_can_try_map_if_predicate() {
        [0u8, 1, 2, 3, 4, 5, 6, 7, 8, 9]
            .map(try_map_if(
                |x: u8| x.checked_add(1).ok_or(()),
                |x| x % 2 == 1,
            ))
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
            .iter()
            .for_each(|x| assert_eq!(x % 2, 0));

        assert!([0, 1, 255]
            .map(try_map_if(
                |x: u8| x.checked_add(1).ok_or(()),
                |x| x % 2 == 1,
            ))
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .is_err());
    }

    #[test]
    fn we_can_check_if_set_contains_identifiers() {
        let idents = IndexSet::from_iter(["INT_COL", "VARCHAR_COL"].map(Ident::new));

        let identifier_is_in = identifier_is_in_fn(&idents);

        assert!(identifier_is_in(&(Ident::new("INT_COL"), ())));
        assert!(identifier_is_in(&(Ident::new("VARCHAR_COL"), ())));
        assert!(!identifier_is_in(&(Ident::new("VARBINARY_COL"), ())));
    }
}
