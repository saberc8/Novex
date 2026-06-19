mod builtin;
mod module;
mod types;
mod validation;

pub use builtin::builtin_plugin_manifest;
pub use module::module;
pub use types::{
    PluginCapability, PluginCapabilityKind, PluginManifest, PluginManifestError,
    PluginNetworkPolicy, PluginRuntime,
};
pub use validation::{required_plugin_permissions, validate_plugin_manifest};

pub const CRATE_ID: &str = "novex-plugin";
