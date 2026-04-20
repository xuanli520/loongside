use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use ed25519_dalek::{Signature as Ed25519Signature, Verifier, VerifyingKey};
use serde_json::json;
use sha2::Digest;
use std::{collections::BTreeSet, fs, io::Write, path::Path};

use crate::BUNDLED_SECURITY_SCAN_PROFILE;
use crate::spec_runtime::*;
use crate::spec_runtime::{KeyPurpose, SecurityProfileSignatureSpec, SigningKeyMeta, TrustAnchor, TrustAnchorSpec};

use super::SecurityScanDelta;

pub fn security_scan_policy(spec: &RunnerSpec) -> Result<Option<SecurityScanSpec>, String> {
    let Some(mut policy) = spec
        .bridge_support
        .as_ref()
        .filter(|bridge| bridge.enabled)
        .and_then(|bridge| bridge.security_scan.clone())
    else {
        return Ok(None);
    };

    if !policy.enabled {
        return Ok(None);
    }

    validate_security_scan_policy(&policy)?;

    let profile = resolve_security_scan_profile(&policy)?;

    if policy.high_risk_metadata_keywords.is_empty() {
        policy.high_risk_metadata_keywords = profile.high_risk_metadata_keywords;
    }

    if policy.wasm.blocked_import_prefixes.is_empty() {
        policy.wasm.blocked_import_prefixes = profile.wasm.blocked_import_prefixes;
    }

    if policy.wasm.max_module_bytes == 0 {
        policy.wasm.max_module_bytes = profile.wasm.max_module_bytes;
    }

    if policy.wasm.allowed_path_prefixes.is_empty() {
        policy.wasm.allowed_path_prefixes = profile.wasm.allowed_path_prefixes;
    }

    if policy.wasm.required_sha256_by_plugin.is_empty() {
        policy.wasm.required_sha256_by_plugin = profile.wasm.required_sha256_by_plugin;
    }

    Ok(Some(policy))
}

fn validate_security_scan_policy(policy: &SecurityScanSpec) -> Result<(), String> {
    if policy.profile_sha256.is_some() && policy.profile_path.is_none() {
        return Err(
            "security scan profile_sha256 requires security_scan.profile_path to be set".to_owned(),
        );
    }
    if policy.profile_signature.is_some() && policy.profile_path.is_none() {
        return Err(
            "security scan profile_signature requires security_scan.profile_path to be set"
                .to_owned(),
        );
    }
    if let Some(signature) = policy.profile_signature.as_ref() {
        if signature.public_key_base64.trim().is_empty() {
            return Err(
                "security scan profile_signature.public_key_base64 cannot be empty".to_owned(),
            );
        }
        if signature.signature_base64.trim().is_empty() {
            return Err(
                "security scan profile_signature.signature_base64 cannot be empty".to_owned(),
            );
        }
    }
    if let Some(export) = policy.siem_export.as_ref().filter(|value| value.enabled)
        && export.path.trim().is_empty()
    {
        return Err("security scan siem_export.path cannot be empty when enabled".to_owned());
    }
    if policy.runtime.execute_wasm_component && policy.runtime.allowed_path_prefixes.is_empty() {
        return Err(
            "security scan runtime.execute_wasm_component requires runtime.allowed_path_prefixes to be configured".to_owned(),
        );
    }
    validate_connector_circuit_breaker_policy(
        &policy.runtime.bridge_circuit_breaker,
        "security scan runtime.bridge_circuit_breaker",
    )?;
    Ok(())
}

fn resolve_security_scan_profile(policy: &SecurityScanSpec) -> Result<SecurityScanProfile, String> {
    if let Some(path) = policy.profile_path.as_deref() {
        let profile = load_security_scan_profile_from_path(path);
        match profile {
            Ok(profile) => {
                if let Some(expected_sha256) = policy.profile_sha256.as_deref() {
                    let actual_sha256 = super::security_scan_profile_sha256(&profile);
                    if !expected_sha256.eq_ignore_ascii_case(&actual_sha256) {
                        return Err(format!(
                            "security scan profile sha256 mismatch for {path}: expected {expected_sha256}, actual {actual_sha256}",
                        ));
                    }
                }
                if let Some(signature) = policy.profile_signature.as_ref() {
                    verify_security_scan_profile_signature(&profile, signature, policy.trust_anchor.as_ref()).map_err(|error| {
                        format!(
                            "security scan profile signature verification failed for {path}: {error}"
                        )
                    })?;
                }
                return Ok(profile);
            }
            Err(error) if policy.profile_sha256.is_some() || policy.profile_signature.is_some() => {
                return Err(format!(
                    "failed to load security scan profile at {path} while profile integrity is pinned: {error}",
                ));
            }
            Err(_) => {}
        }
    }

    Ok(bundled_security_scan_profile())
}

pub fn load_security_scan_profile_from_path(path: &str) -> Result<SecurityScanProfile, String> {
    let content =
        fs::read_to_string(path).map_err(|error| format!("read profile failed: {error}"))?;
    serde_json::from_str::<SecurityScanProfile>(&content)
        .map_err(|error| format!("parse profile failed: {error}"))
}

fn bundled_security_scan_profile() -> SecurityScanProfile {
    BUNDLED_SECURITY_SCAN_PROFILE
        .get_or_init(|| {
            let raw = include_str!("../../config/security-scan-medium-balanced.json");
            serde_json::from_str(raw)
                .or_else(|_| serde_json::from_str::<SecurityScanProfile>("{}"))
                .unwrap_or_else(|_| SecurityScanProfile {
                    high_risk_metadata_keywords: Vec::new(),
                    wasm: WasmSecurityScanSpec::default(),
                })
        })
        .clone()
}

fn verify_security_scan_profile_signature(
    profile: &SecurityScanProfile,
    signature: &SecurityProfileSignatureSpec,
    trust_anchor: Option<&TrustAnchorSpec>,
) -> Result<(), String> {
    let algorithm = signature.algorithm.trim().to_ascii_lowercase();
    if algorithm != "ed25519" {
        return Err(format!(
            "unsupported profile signature algorithm: {algorithm} (expected ed25519)"
        ));
    }

    let public_key_base64 = if let Some(key_id) = signature.key_id.as_deref() {
        // Trust anchor lookup required
        let anchor = load_trust_anchor(trust_anchor)?;
        let key_meta = lookup_key_by_id(&anchor, key_id)?;
        validate_key_for_profile_signing(key_meta, &anchor)?;
        key_meta.public_key_base64.clone()
    } else {
        signature.public_key_base64.clone()
    };

    let public_key_bytes = BASE64_STANDARD
        .decode(public_key_base64.trim())
        .map_err(|error| format!("invalid public_key_base64: {error}"))?;
    let public_key_bytes: [u8; 32] = public_key_bytes
        .as_slice()
        .try_into()
        .map_err(|_err| "invalid ed25519 public key length (expected 32 bytes)".to_owned())?;
    let verifying_key = VerifyingKey::from_bytes(&public_key_bytes)
        .map_err(|error| format!("invalid ed25519 public key bytes: {error}"))?;

    let signature_bytes = BASE64_STANDARD
        .decode(signature.signature_base64.trim())
        .map_err(|error| format!("invalid signature_base64: {error}"))?;
    let signature_bytes: [u8; 64] = signature_bytes
        .as_slice()
        .try_into()
        .map_err(|_err| "invalid ed25519 signature length (expected 64 bytes)".to_owned())?;
    let signature = Ed25519Signature::from_bytes(&signature_bytes);

    let message = super::security_scan_profile_message(profile);
    verifying_key
        .verify(&message, &signature)
        .map_err(|error| format!("ed25519 verification failed: {error}"))
}

/// Load and optionally integrity-check a trust anchor.
fn load_trust_anchor(spec: Option<&TrustAnchorSpec>) -> Result<TrustAnchor, String> {
    let spec = spec.ok_or_else(|| "profile_signature.key_id requires trust_anchor configuration".to_owned())?;

    let content = fs::read_to_string(&spec.path)
        .map_err(|error| format!("failed to read trust anchor at {}: {error}", spec.path))?;

    if let Some(expected_sha256) = spec.sha256.as_deref() {
        let actual_sha256 = sha2_digest(&content);
        if !expected_sha256.eq_ignore_ascii_case(&actual_sha256) {
            return Err(format!(
                "trust anchor sha256 mismatch for {}: expected {expected_sha256}, actual {actual_sha256}",
                spec.path
            ));
        }
    }

    serde_json::from_str::<TrustAnchor>(&content)
        .map_err(|error| format!("failed to parse trust anchor at {}: {error}", spec.path))
}

/// Compute SHA-256 hex digest of a string.
fn sha2_digest(content: &str) -> String {
    let digest = sha2::Sha256::digest(content.as_bytes());
    hex::encode(digest)
}

/// Look up a signing key by its key_id in the trust anchor.
fn lookup_key_by_id<'a>(anchor: &'a TrustAnchor, key_id: &str) -> Result<&'a SigningKeyMeta, String> {
    anchor
        .keys
        .iter()
        .find(|k| k.key_id == key_id)
        .ok_or_else(|| format!("key_id '{}' not found in trust anchor", key_id))
}

/// Validate that a key is valid for profile signing: not revoked, not expired, correct purpose.
fn validate_key_for_profile_signing(key: &SigningKeyMeta, anchor: &TrustAnchor) -> Result<(), String> {
    // Check revocation
    if let Some(revocation_list) = anchor.revocation_list.as_ref() {
        if revocation_list.revoked_key_ids.iter().any(|id| id == &key.key_id) {
            return Err(format!("key_id '{}' has been revoked", key.key_id));
        }
    }

    // Check expiry
    if let Some(expires_at) = key.expires_at_epoch_s {
        let now = super::current_epoch_s() as u64;
        if now > expires_at {
            return Err(format!("key_id '{}' has expired", key.key_id));
        }
    }

    // Check purpose
    if key.purpose != KeyPurpose::ProfileSigning {
        return Err(format!(
            "key_id '{}' has purpose {:?}, expected ProfileSigning",
            key.key_id, key.purpose
        ));
    }

    Ok(())
}

pub(super) fn security_scan_process_allowlist(spec: &RunnerSpec) -> BTreeSet<String> {
    spec.bridge_support
        .as_ref()
        .filter(|bridge| bridge.enabled)
        .map(|bridge| {
            bridge
                .allowed_process_commands
                .iter()
                .map(|value| value.trim().to_ascii_lowercase())
                .filter(|value| !value.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn apply_security_scan_delta(report: &mut SecurityScanReport, delta: SecurityScanDelta) {
    report.scanned_plugins = report.scanned_plugins.saturating_add(delta.scanned_plugins);
    report.high_findings = report.high_findings.saturating_add(delta.high_findings);
    report.medium_findings = report.medium_findings.saturating_add(delta.medium_findings);
    report.low_findings = report.low_findings.saturating_add(delta.low_findings);
    report.total_findings = report
        .high_findings
        .saturating_add(report.medium_findings)
        .saturating_add(report.low_findings);
    report.findings.extend(delta.findings);
    if let Some(reason) = delta.block_reason {
        report.blocked = true;
        report.block_reason = Some(reason);
    }
}

pub(super) fn emit_security_scan_siem_record(
    pack_id: &str,
    agent_id: &str,
    report: &SecurityScanReport,
    export: &SecuritySiemExportSpec,
) -> Result<SecuritySiemExportReport, String> {
    let mut findings = report.findings.clone();
    let mut truncated_findings = 0usize;

    if export.include_findings {
        if let Some(limit) = export.max_findings_per_record
            && findings.len() > limit
        {
            truncated_findings = findings.len().saturating_sub(limit);
            findings.truncate(limit);
        }
    } else {
        truncated_findings = report.findings.len();
        findings.clear();
    }

    let categories: Vec<String> = report
        .findings
        .iter()
        .map(|finding| finding.category.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let finding_ids: Vec<String> = report
        .findings
        .iter()
        .map(|finding| finding.correlation_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    let record = json!({
        "event_type": "security_scan_report",
        "ts_epoch_s": super::current_epoch_s(),
        "pack_id": pack_id,
        "agent_id": agent_id,
        "blocked": report.blocked,
        "block_reason": report.block_reason.clone(),
        "counts": {
            "scanned_plugins": report.scanned_plugins,
            "total_findings": report.total_findings,
            "high_findings": report.high_findings,
            "medium_findings": report.medium_findings,
            "low_findings": report.low_findings,
        },
        "categories": categories,
        "finding_ids": finding_ids,
        "truncated_findings": truncated_findings,
        "findings": findings,
    });

    let path = Path::new(&export.path);
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create siem export directory failed: {error}"))?;
    }

    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| format!("open siem export file failed: {error}"))?;
    serde_json::to_writer(&mut file, &record)
        .map_err(|error| format!("serialize siem export record failed: {error}"))?;
    file.write_all(b"\n")
        .map_err(|error| format!("flush siem export record failed: {error}"))?;

    Ok(SecuritySiemExportReport {
        enabled: true,
        path: export.path.clone(),
        success: true,
        exported_records: 1,
        exported_findings: findings.len(),
        truncated_findings,
        error: None,
    })
}
