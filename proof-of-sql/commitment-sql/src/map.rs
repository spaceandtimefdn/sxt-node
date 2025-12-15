/// IndexMap with a default `AHasher`, for `no_std` usage
pub type IndexMap<K, V> = indexmap::IndexMap<K, V, core::hash::BuildHasherDefault<ahash::AHasher>>;
