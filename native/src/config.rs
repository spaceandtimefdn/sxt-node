//! Native interface for configuration access.

use alloc::string::String;

use polkadot_sdk::sp_externalities::ExternalitiesExt;
use polkadot_sdk::sp_runtime_interface::runtime_interface;

/// A trait for providing configuration values.
pub trait ConfigProvider: Send + Sync {
    /// Given a key, return the value under that key in the node's configuration.
    fn get(&self, key: &str) -> Option<String>;
}

#[cfg(feature = "std")]
impl ConfigProvider for std::collections::HashMap<String, String> {
    fn get(&self, key: &str) -> Option<String> {
        std::collections::HashMap::get(self, key).cloned()
    }
}

#[cfg(feature = "std")]
polkadot_sdk::sp_externalities::decl_extension! {
    /// Externalities extension exposing the node's configuration.
    pub struct ConfigExt(std::sync::Arc<dyn ConfigProvider>);
}

/// Native code interface onto the node's configuration, exposed via the [`ConfigExt`] externalities extension.
#[runtime_interface]
pub trait Config {
    /// Given a key, return the value under that key in the node's configuration.
    fn get(&mut self, key: &str) -> Option<Option<String>> {
        ExternalitiesExt::extension(self).map(|ConfigExt(provider)| provider.get(key))
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use polkadot_sdk::sp_io::TestExternalities;

    use super::*;

    #[test]
    fn get_is_none_when_extension_not_registered() {
        let mut ext = TestExternalities::default();
        ext.execute_with(|| {
            assert_eq!(config::get("some-key"), None);
        });
    }

    #[test]
    fn get_is_some_none_when_key_is_absent() {
        let mut ext = TestExternalities::default();
        ext.register_extension(ConfigExt(Arc::new(HashMap::new())));
        ext.execute_with(|| {
            assert_eq!(config::get("missing-key"), Some(None));
        });
    }

    #[test]
    fn get_forwards_registered_value() {
        let mut ext = TestExternalities::default();
        let mut map = HashMap::new();
        map.insert("some-key".to_string(), "some-value".to_string());
        ext.register_extension(ConfigExt(Arc::new(map)));
        ext.execute_with(|| {
            assert_eq!(
                config::get("some-key"),
                Some(Some("some-value".to_string()))
            );
        });
    }
}
