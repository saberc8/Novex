use std::collections::BTreeSet;

use crate::types::{PluginManifest, PluginManifestError, PluginRuntime};

pub fn validate_plugin_manifest(manifest: &PluginManifest) -> Result<(), PluginManifestError> {
    ensure_non_empty("code", &manifest.code)?;
    ensure_non_empty("name", &manifest.name)?;
    ensure_non_empty("version", &manifest.version)?;

    if manifest.capabilities.is_empty() {
        return Err(PluginManifestError::MissingCapabilities);
    }

    if matches!(manifest.runtime, PluginRuntime::HostedHttp)
        && manifest
            .network
            .allowlist
            .iter()
            .all(|host| host.trim().is_empty())
    {
        return Err(PluginManifestError::HostedHttpRequiresNetworkAllowlist);
    }

    let granted_permissions = manifest
        .permission_grants
        .iter()
        .map(|permission| permission.trim())
        .filter(|permission| !permission.is_empty())
        .collect::<BTreeSet<_>>();

    for capability in &manifest.capabilities {
        let capability_code = capability.code.trim();
        ensure_non_empty("capability.code", capability_code)?;
        let permission_code = capability.permission_code.trim();
        if permission_code.is_empty() {
            return Err(PluginManifestError::CapabilityMissingPermission {
                capability_code: capability_code.to_owned(),
            });
        }
        if !granted_permissions.contains(permission_code) {
            return Err(PluginManifestError::PermissionNotGranted {
                capability_code: capability_code.to_owned(),
                permission_code: permission_code.to_owned(),
            });
        }
    }

    Ok(())
}

pub fn required_plugin_permissions(manifest: &PluginManifest) -> Vec<String> {
    manifest
        .capabilities
        .iter()
        .map(|capability| capability.permission_code.trim())
        .filter(|permission| !permission.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(ToOwned::to_owned)
        .collect()
}

fn ensure_non_empty(field: &'static str, value: &str) -> Result<(), PluginManifestError> {
    if value.trim().is_empty() {
        return Err(PluginManifestError::MissingField(field));
    }
    Ok(())
}
