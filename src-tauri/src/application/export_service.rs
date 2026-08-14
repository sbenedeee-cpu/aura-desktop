// EXP-001: local export and recovery service.
//
// Assembles the full user workspace into a single versioned, encrypted
// envelope (manifest plaintext, record content sealed with the DPAPI-bound
// key) and applies an imported envelope back through the typed repository
// boundary inside one all-or-nothing transaction. The renderer never reads
// the database and never chooses filesystem paths: the native file dialog
// supplies the export/import destination.

use crate::db::migrations::utc_timestamp;
use crate::db::LocalStore;
use crate::domain::capture::CaptureRetention;
use crate::domain::export::{
    ExportEvent, ExportManifest, ExportPayload, ExportRecordCounts, ExportedSetting,
    EXPORT_FORMAT_VERSION, PASSPHRASE_SEALING,
};
use crate::domain::project::AuraError;
use crate::security::key_vault::KeyVault;
use crate::security::passphrase::{
    generate_salt, Passphrase, PassphraseKey, ARGON2_M_COST, ARGON2_P_COST, ARGON2_T_COST,
    PASSPHRASE_SALT_LENGTH,
};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Export application version embedded in the manifest so an older Aura
/// build can display a meaningful warning about newer archives.
pub const EXPORT_APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// On-disk envelope: plaintext manifest plus the sealed record payload.
///
/// Version 2 adds the `sealing` discriminator. When the sealing is
/// `"passphrase"`, the argon2id salt and public derivation parameters are
/// carried in plaintext next to the manifest (they are non-secret), and the
/// sealed payload is produced with the passphrase-derived key rather than
/// the DPAPI-bound workspace key. Version-1 envelopes have no `sealing`
/// field and are always DPAPI-sealed.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportEnvelope {
    pub format_version: i64,
    /// How the payload was sealed: `"dpapi"` (default) or `"passphrase"`.
    #[serde(default = "default_sealing")]
    pub sealing: String,
    #[serde(rename = "exportedAt")]
    pub exported_at: String,
    pub exported_by_version: String,
    pub record_counts: ExportRecordCounts,
    pub payload_checksum: String,
    /// Hex-encoded: one version byte, the 12-byte nonce, then the AEAD
    /// ciphertext. See `KeyVault::encode_sealed`.
    pub payload_sealed_hex: String,
    /// Hex-encoded argon2id salt; present only for `"passphrase"` envelopes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub passphrase_salt_hex: Option<String>,
    /// Public argon2id parameters; present only for `"passphrase"` envelopes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub passphrase_params: Option<PassphraseParams>,
}

fn default_sealing() -> String {
    "dpapi".to_string()
}

/// Public argon2id derivation parameters stored next to a passphrase-sealed
/// payload. These are non-secret: they only let any Aura install re-derive
/// the same key from the same salt and passphrase.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PassphraseParams {
    pub memory_cost_kib: u32,
    pub time_cost: u32,
    pub parallelism: u32,
}

impl PassphraseParams {
    pub fn owasp_defaults() -> Self {
        Self {
            memory_cost_kib: ARGON2_M_COST,
            time_cost: ARGON2_T_COST,
            parallelism: ARGON2_P_COST,
        }
    }
}

pub struct ExportService<'store> {
    store: &'store mut LocalStore,
    key_vault: &'store KeyVault,
    /// Optional passphrase for opening passphrase-sealed archives. Set by
    /// the command layer before `apply_import`; never serialized or logged.
    passphrase: Option<Passphrase>,
}

impl<'store> ExportService<'store> {
    pub fn new(store: &'store mut LocalStore, key_vault: &'store KeyVault) -> Self {
        Self {
            store,
            key_vault,
            passphrase: None,
        }
    }

    /// Supplies the passphrase needed to open a passphrase-sealed archive.
    /// The key is derived inside `open_envelope` and cleared on drop.
    pub fn set_passphrase(&mut self, passphrase: Option<Passphrase>) {
        self.passphrase = passphrase;
    }

    /// Reads every record the user owns through the typed repositories and
    /// returns the sealed envelope ready to be written to disk by the caller.
    pub fn assemble_export(&self) -> Result<ExportEnvelope, AuraError> {
        let projects = self
            .store
            .projects()
            .list_active()
            .map_err(|error| {
                AuraError::Storage(format!(
                    "Aura could not read its local projects for export: {error}"
                ))
            })?
            .into_iter()
            .map(|project| serde_json::to_value(&project))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| {
                AuraError::Storage(format!(
                    "Aura could not encode its local projects for export: {error}"
                ))
            })?;

        let preferences = self.store.privacy_preferences()?;
        let settings_rows = vec![
            ExportedSetting {
                key: "privacy_mode".to_string(),
                value: preferences.privacy_mode.as_str().to_string(),
            },
            ExportedSetting {
                key: "default_capture_retention".to_string(),
                value: preferences.default_capture_retention.as_str().to_string(),
            },
        ];
        let exclusion_rules = preferences
            .exclusions
            .into_iter()
            .map(|rule| serde_json::to_value(&rule))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| {
                AuraError::Storage(format!(
                    "Aura could not encode its local exclusion rules for export: {error}"
                ))
            })?;

        let captures = self.collect_captures().map_err(|error| {
            AuraError::Storage(format!(
                "Aura could not read its local captures for export: {error}"
            ))
        })?;
        let decisions = self.collect_decisions().map_err(|error| {
            AuraError::Storage(format!(
                "Aura could not read its local decisions for export: {error}"
            ))
        })?;

        let payload = ExportPayload {
            projects,
            captures,
            decisions,
            exclusion_rules,
            settings: settings_rows,
        };
        let record_counts = payload.record_counts();

        let payload_json = serde_json::to_string(&payload).map_err(|error| {
            AuraError::Storage(format!(
                "Aura could not serialize its local workspace for export: {error}"
            ))
        })?;
        let checksum = hex::encode(Sha256::digest(payload_json.as_bytes()));

        let sealed = self
            .key_vault
            .seal(payload_json.as_bytes())
            .map_err(|error| {
                AuraError::Storage(format!(
                    "Aura could not seal its local workspace for export: {error}"
                ))
            })?;
        let payload_sealed_hex = hex::encode(KeyVault::encode_sealed(&sealed));

        Ok(ExportEnvelope {
            format_version: EXPORT_FORMAT_VERSION,
            sealing: "dpapi".to_string(),
            exported_at: utc_timestamp(),
            exported_by_version: EXPORT_APP_VERSION.to_string(),
            record_counts,
            payload_checksum: checksum,
            payload_sealed_hex,
            passphrase_salt_hex: None,
            passphrase_params: None,
        })
    }

    /// Reads every record the user owns and seals the payload with a
    /// passphrase-derived key (EXP-002). The passphrase is validated against
    /// the native strength gate before any derivation happens, and the
    /// derived key exists only for the duration of the sealing call.
    pub fn assemble_passphrase_export(
        &self,
        passphrase: Passphrase,
    ) -> Result<ExportEnvelope, AuraError> {
        if !passphrase.meets_strength_gate() {
            return Err(AuraError::InvalidInput(
                "Choose a stronger passphrase: at least 12 characters, or 8 characters mixing upper case, lower case, and digits.".to_string(),
            ));
        }

        let projects = self
            .store
            .projects()
            .list_active()
            .map_err(|error| {
                AuraError::Storage(format!(
                    "Aura could not read its local projects for export: {error}"
                ))
            })?
            .into_iter()
            .map(|project| serde_json::to_value(&project))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| {
                AuraError::Storage(format!(
                    "Aura could not encode its local projects for export: {error}"
                ))
            })?;

        let preferences = self.store.privacy_preferences()?;
        let settings_rows = vec![
            ExportedSetting {
                key: "privacy_mode".to_string(),
                value: preferences.privacy_mode.as_str().to_string(),
            },
            ExportedSetting {
                key: "default_capture_retention".to_string(),
                value: preferences.default_capture_retention.as_str().to_string(),
            },
        ];
        let exclusion_rules = preferences
            .exclusions
            .into_iter()
            .map(|rule| serde_json::to_value(&rule))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| {
                AuraError::Storage(format!(
                    "Aura could not encode its local exclusion rules for export: {error}"
                ))
            })?;

        let captures = self.collect_captures().map_err(|error| {
            AuraError::Storage(format!(
                "Aura could not read its local captures for export: {error}"
            ))
        })?;
        let decisions = self.collect_decisions().map_err(|error| {
            AuraError::Storage(format!(
                "Aura could not read its local decisions for export: {error}"
            ))
        })?;

        let payload = ExportPayload {
            projects,
            captures,
            decisions,
            exclusion_rules,
            settings: settings_rows,
        };
        let record_counts = payload.record_counts();

        let payload_json = serde_json::to_string(&payload).map_err(|error| {
            AuraError::Storage(format!(
                "Aura could not serialize its local workspace for export: {error}"
            ))
        })?;
        let checksum = hex::encode(Sha256::digest(payload_json.as_bytes()));

        let salt = generate_salt().map_err(|error| {
            AuraError::Storage(format!(
                "Aura could not produce a derivation salt for the export: {error}"
            ))
        })?;
        let key = PassphraseKey::derive(&passphrase, &salt).map_err(|error| {
            AuraError::Storage(format!(
                "Aura could not derive the export key from the passphrase: {error}"
            ))
        })?;
        let sealed_raw = key.seal(payload_json.as_bytes()).map_err(|error| {
            AuraError::Storage(format!(
                "Aura could not seal its local workspace with the passphrase key: {error}"
            ))
        })?;

        // The passphrase is dropped here; its bytes are zeroed before this
        // function returns. Only the salt and public parameters persist.
        Ok(ExportEnvelope {
            format_version: EXPORT_FORMAT_VERSION,
            sealing: PASSPHRASE_SEALING.to_string(),
            exported_at: utc_timestamp(),
            exported_by_version: EXPORT_APP_VERSION.to_string(),
            record_counts,
            payload_checksum: checksum,
            payload_sealed_hex: hex::encode(&sealed_raw),
            passphrase_salt_hex: Some(hex::encode(salt)),
            passphrase_params: Some(PassphraseParams::owasp_defaults()),
        })
    }

    /// Writes an export event to the append-only audit table. Export and
    /// import attempts are always recorded, successes and failures alike.
    pub fn record_export_event(
        &self,
        event: ExportEvent,
        record_counts: &ExportRecordCounts,
        detail: &str,
    ) -> Result<(), AuraError> {
        let connection = self.store.connection_ref();
        let next_sequence: i64 = connection
            .query_row(
                "SELECT COALESCE(MAX(sequence), 0) + 1 FROM export_metadata",
                [],
                |row| row.get(0),
            )
            .map_err(|error| {
                AuraError::Storage(format!(
                    "Aura could not read its export-and-recovery sequence: {error}"
                ))
            })?;
        connection
            .execute(
                "INSERT INTO export_metadata (id, event_type, envelope_format_version, record_counts, detail, created_at, sequence)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    Uuid::new_v4().to_string(),
                    event.as_str(),
                    EXPORT_FORMAT_VERSION,
                    serde_json::to_string(record_counts).unwrap_or_else(|_| "{}".to_string()),
                    detail,
                    utc_timestamp(),
                    next_sequence,
                ],
            )
            .map_err(|error| {
                AuraError::Storage(format!(
                    "Aura could not record its local export-and-recovery event: {error}"
                ))
            })?;
        Ok(())
    }

    /// Plaintext manifest preview. Returns no decrypted record content.
    /// Version-1 (DPAPI-only) envelopes remain readable for inventory
    /// purposes.
    pub fn envelope_manifest(raw: &[u8]) -> Result<ExportManifest, AuraError> {
        let envelope: ExportEnvelope = serde_json::from_slice(raw).map_err(|error| {
            AuraError::InvalidInput(format!(
                "Aura could not read that export file: it is not a supported Aura workspace archive. {error}"
            ))
        })?;

        if envelope.format_version > EXPORT_FORMAT_VERSION {
            return Err(AuraError::InvalidInput(format!(
                "Aura does not support export archives of format version {}; this build expects version {EXPORT_FORMAT_VERSION}.",
                envelope.format_version
            )));
        }
        if envelope.format_version < 1 {
            return Err(AuraError::InvalidInput(format!(
                "Aura does not support export archives of format version {}.",
                envelope.format_version
            )));
        }

        Ok(ExportManifest {
            format_version: envelope.format_version,
            sealing: envelope.sealing.clone(),
            exported_at: envelope.exported_at,
            exported_by_version: envelope.exported_by_version,
            record_counts: envelope.record_counts,
            payload_checksum: envelope.payload_checksum,
            payload_sealed_length: envelope.payload_sealed_hex.len() / 2,
        })
    }

    /// Opens the sealed payload of an envelope with the correct key for its
    /// sealing variant. Passphrase-sealed envelopes are opened with a key
    /// derived from the supplied passphrase; a wrong passphrase fails
    /// authentication here and no record is ever written.
    fn open_envelope(&self, envelope: &ExportEnvelope) -> Result<Vec<u8>, AuraError> {
        let sealed_bytes = hex::decode(&envelope.payload_sealed_hex).map_err(|error| {
            AuraError::InvalidInput(format!(
                "The exported archive is damaged: its sealed payload is not valid hex. {error}"
            ))
        })?;

        if envelope.sealing == PASSPHRASE_SEALING {
            let salt_hex = envelope.passphrase_salt_hex.as_deref().unwrap_or("");
            let salt = hex::decode(salt_hex).map_err(|error| {
                AuraError::InvalidInput(format!(
                    "The exported archive is damaged: its derivation salt is not valid hex. {error}"
                ))
            })?;
            if salt.len() != PASSPHRASE_SALT_LENGTH {
                return Err(AuraError::InvalidInput(
                    "The exported archive is damaged: its derivation salt has an unexpected length.".to_string(),
                ));
            }
            let key = PassphraseKey::derive(
                self.passphrase.as_ref().ok_or_else(|| {
                    AuraError::InvalidInput(
                        "That archive was sealed with a passphrase; the passphrase is required to open it.".to_string(),
                    )
                })?,
                &salt,
            )
            .map_err(|error| {
                AuraError::InvalidInput(format!(
                    "Aura could not derive the archive key from the passphrase: {error}"
                ))
            })?;
            return key.open(&sealed_bytes).map_err(|error| {
                AuraError::InvalidInput(format!(
                    "Aura could not open that archive: its sealed contents failed authentication with the supplied passphrase. {error}"
                ))
            });
        }

        let sealed = KeyVault::decode_sealed(&sealed_bytes).map_err(|error| {
            AuraError::InvalidInput(format!(
                "The exported archive is damaged: its sealed envelope could not be read. {error}"
            ))
        })?;
        self.key_vault.open(&sealed).map_err(|error| {
            AuraError::InvalidInput(format!(
                "Aura could not open that archive: its sealed contents failed authentication. The archive may be damaged or was created on a computer with a different workspace key. {error}"
            ))
        })
    }

    /// Decrypts, validates, and applies an imported archive. The entire
    /// import happens inside one transaction: on any conflict or validation
    /// failure nothing is applied and the original workspace is untouched.
    ///
    /// A DPAPI-sealed envelope (version 1 or version 2 `"dpapi"` sealing)
    /// opens with the local workspace key. A `"passphrase"` envelope
    /// requires the passphrase that sealed it; a wrong passphrase fails
    /// authentication before any record is written.
    pub fn apply_import(&mut self, raw: &[u8]) -> Result<ExportRecordCounts, AuraError> {
        let envelope: ExportEnvelope = serde_json::from_slice(raw).map_err(|error| {
            AuraError::InvalidInput(format!(
                "Aura could not read that export file: it is not a supported Aura workspace archive. {error}"
            ))
        })?;

        if envelope.format_version > EXPORT_FORMAT_VERSION {
            return Err(AuraError::InvalidInput(format!(
                "Aura does not support export archives of format version {}; this build expects version {EXPORT_FORMAT_VERSION}.",
                envelope.format_version
            )));
        }
        if envelope.format_version < 1 {
            return Err(AuraError::InvalidInput(format!(
                "Aura does not support export archives of format version {}.",
                envelope.format_version
            )));
        }

        let payload_bytes = self.open_envelope(&envelope)?;

        let payload_json = String::from_utf8(payload_bytes).map_err(|error| {
            AuraError::InvalidInput(format!(
                "The exported archive is damaged: its payload is not valid text. {error}"
            ))
        })?;

        let checksum = hex::encode(Sha256::digest(payload_json.as_bytes()));
        if checksum != envelope.payload_checksum {
            return Err(AuraError::InvalidInput(
                "The exported archive is damaged: its contents no longer match the recorded checksum.".to_string(),
            ));
        }

        let payload: ExportPayload = serde_json::from_str(&payload_json).map_err(|error| {
            AuraError::InvalidInput(format!(
                "The exported archive is damaged: its payload could not be decoded. {error}"
            ))
        })?;

        let computed_counts = payload.record_counts();
        if computed_counts != envelope.record_counts {
            return Err(AuraError::InvalidInput(
                "The exported archive is damaged: its record inventory no longer matches the manifest.".to_string(),
            ));
        }

        let transaction = self
            .store
            .connection_ref_mut()
            .transaction()
            .map_err(|error| {
                AuraError::Storage(format!(
                    "Aura could not begin the local workspace recovery: {error}"
                ))
            })?;

        apply_payload(&transaction, &payload)?;

        transaction.commit().map_err(|error| {
            AuraError::Storage(format!(
                "Aura could not finalize the local workspace recovery. Your existing data was not changed: {error}"
            ))
        })?;

        Ok(computed_counts)
    }
}

// ---------------------------------------------------------------------------
// Payload application: reads typed JSON back through the repository
// boundary, inside the caller-held transaction.
// ---------------------------------------------------------------------------

fn apply_payload(transaction: &Connection, payload: &ExportPayload) -> Result<(), AuraError> {
    for raw_project in &payload.projects {
        let project =
            serde_json::from_value::<ExportedProject>(raw_project.clone()).map_err(|error| {
                AuraError::InvalidInput(format!(
                    "The archive contains a damaged project record: {error}"
                ))
            })?;
        apply_project(transaction, project)?;
    }

    for raw_exclusion in &payload.exclusion_rules {
        let rule = serde_json::from_value::<ExportedExclusion>(raw_exclusion.clone()).map_err(
            |error| {
                AuraError::InvalidInput(format!(
                    "The archive contains a damaged exclusion rule: {error}"
                ))
            },
        )?;
        apply_exclusion(transaction, rule)?;
    }

    for setting in &payload.settings {
        apply_setting(transaction, setting)?;
    }

    for raw_capture in &payload.captures {
        let capture =
            serde_json::from_value::<ExportedCapture>(raw_capture.clone()).map_err(|error| {
                AuraError::InvalidInput(format!(
                    "The archive contains a damaged capture record: {error}"
                ))
            })?;
        apply_capture(transaction, capture)?;
    }

    for raw_decision in &payload.decisions {
        let decision =
            serde_json::from_value::<ExportedDecision>(raw_decision.clone()).map_err(|error| {
                AuraError::InvalidInput(format!(
                    "The archive contains a damaged decision record: {error}"
                ))
            })?;
        apply_decision(transaction, decision)?;
    }

    Ok(())
}

/// Import-time project representation. Status is validated against the
/// supported lifecycle values; an unsupported value fails the import.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportedProject {
    id: String,
    name: String,
    goal: Option<String>,
    status: Option<String>,
    current_task: Option<String>,
    blocker: Option<String>,
    next_step: Option<String>,
    created_at: String,
    updated_at: String,
    archived_at: Option<String>,
}

fn apply_project(transaction: &Connection, project: ExportedProject) -> Result<(), AuraError> {
    if project.name.trim().is_empty() {
        return Err(AuraError::InvalidInput(
            "The archive contains a project without a name.".to_string(),
        ));
    }
    let status = project.status.as_deref().unwrap_or("active");
    if !matches!(status, "active" | "paused" | "archived") {
        return Err(AuraError::InvalidInput(format!(
            "The archive contains a project with an unsupported status: {status}."
        )));
    }

    let created_at = utc_timestamp();
    let mut statement = transaction.prepare(
        "INSERT INTO projects (id, name, goal, status, current_task, blocker, next_step, created_at, updated_at, archived_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT(id) DO UPDATE SET
             name = excluded.name,
             goal = excluded.goal,
             status = excluded.status,
             current_task = excluded.current_task,
             blocker = excluded.blocker,
             next_step = excluded.next_step,
             updated_at = excluded.updated_at,
             archived_at = excluded.archived_at",
    ).map_err(|error| AuraError::Storage(format!("Aura could not prepare the project recovery write: {error}")))?;

    statement
        .execute(params![
            &project.id,
            project.name.trim(),
            project.goal.as_deref().unwrap_or(""),
            status,
            project.current_task.as_deref().unwrap_or(""),
            project.blocker.as_deref().unwrap_or(""),
            project.next_step.as_deref().unwrap_or(""),
            &project.created_at,
            &project.updated_at,
            project.archived_at.as_deref().unwrap_or(""),
        ])
        .map_err(|error| {
            AuraError::Storage(format!(
                "Aura could not write the project record during recovery: {error}"
            ))
        })?;

    transaction
        .execute(
            "INSERT INTO context_markers (id, project_id, source, created_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET created_at = excluded.created_at",
            params![
                Uuid::new_v4().to_string(),
                &project.id,
                "import",
                &created_at
            ],
        )
        .map_err(|error| {
            AuraError::Storage(format!(
                "Aura could not mark the recovered project context: {error}"
            ))
        })?;

    Ok(())
}

/// Import-time exclusion rule representation.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportedExclusion {
    id: String,
    /// The exclusion rule category; the export side serializes the domain
    /// `ExclusionRule` whose field is named `kind` (camelCase-renamed).
    kind: String,
    value: String,
    #[serde(rename = "isEnabled")]
    is_enabled: bool,
    created_at: String,
    updated_at: String,
}

fn apply_exclusion(transaction: &Connection, rule: ExportedExclusion) -> Result<(), AuraError> {
    if !matches!(rule.kind.as_str(), "application" | "domain" | "project") {
        return Err(AuraError::InvalidInput(format!(
            "The archive contains an exclusion rule with an unsupported type: {}.",
            rule.kind
        )));
    }
    if rule.value.trim().is_empty() {
        return Err(AuraError::InvalidInput(
            "The archive contains an exclusion rule without a value.".to_string(),
        ));
    }

    transaction
        .execute(
            "INSERT INTO exclusion_rules (id, rule_type, value, is_enabled, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET
                 rule_type = excluded.rule_type,
                 value = excluded.value,
                 is_enabled = excluded.is_enabled,
                 updated_at = excluded.updated_at",
            params![
                &rule.id,
                rule.kind,
                rule.value.trim(),
                rule.is_enabled as i64,
                &rule.created_at,
                &rule.updated_at,
            ],
        )
        .map_err(|error| {
            AuraError::Storage(format!(
                "Aura could not write the exclusion rule during recovery: {error}"
            ))
        })?;

    Ok(())
}

/// Import-time settings row representation. Only the two settings Aura
/// persists today are accepted; everything else is rejected so a newer
/// archive cannot silently inject unknown settings.
fn apply_setting(transaction: &Connection, setting: &ExportedSetting) -> Result<(), AuraError> {
    let known = matches!(
        setting.key.as_str(),
        "privacy_mode" | "default_capture_retention" | "selected_project_id"
    );
    if !known {
        return Err(AuraError::InvalidInput(format!(
            "The archive contains an unsupported workspace setting: {}.",
            setting.key
        )));
    }

    if setting.key == "default_capture_retention" {
        CaptureRetention::from_store(&setting.value).map_err(|_error| {
            AuraError::InvalidInput(format!(
                "The archive contains an unsupported capture retention value: {}.",
                setting.value
            ))
        })?;
    } else if setting.key == "privacy_mode"
        && setting.value != "manual_only"
        && setting.value != "paused"
    {
        return Err(AuraError::InvalidInput(format!(
            "The archive contains an unsupported privacy mode: {}.",
            setting.value
        )));
    }

    transaction
        .execute(
            "INSERT INTO settings (key, value, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![setting.key, &setting.value, utc_timestamp()],
        )
        .map_err(|error| AuraError::Storage(format!("Aura could not write the workspace setting during recovery: {error}")))?;

    Ok(())
}

/// Import-time capture representation. Captures are re-scoped to the
/// imported project id with the same lifecycle guarantees as an explicit
/// manual capture.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportedCapture {
    id: String,
    project_id: String,
    kind: String,
    label: String,
    content: String,
    classification: String,
    retention: String,
    created_at: String,
}

fn apply_capture(transaction: &Connection, capture: ExportedCapture) -> Result<(), AuraError> {
    if capture.kind != "manual_note" && capture.kind != "pasted_text" && capture.kind != "url" {
        return Err(AuraError::InvalidInput(format!(
            "The archive contains a capture with an unsupported type: {}.",
            capture.kind
        )));
    }
    if capture.classification != "standard" && capture.classification != "sensitive" {
        return Err(AuraError::InvalidInput(format!(
            "The archive contains a capture with an unsupported classification: {}.",
            capture.classification
        )));
    }
    let retention = CaptureRetention::from_store(&capture.retention).map_err(|_error| {
        AuraError::InvalidInput(format!(
            "The archive contains a capture with an unsupported retention: {}.",
            capture.retention
        ))
    })?;

    transaction
        .execute(
            "INSERT INTO captures (id, project_id, kind, label, content, classification, retention, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id) DO UPDATE SET
                 project_id = excluded.project_id,
                 kind = excluded.kind,
                 label = excluded.label,
                 content = excluded.content,
                 classification = excluded.classification,
                 retention = excluded.retention",
            params![
                &capture.id,
                &capture.project_id,
                capture.kind,
                capture.label.trim(),
                capture.content,
                capture.classification,
                retention.as_str(),
                &capture.created_at,
            ],
        )
        .map_err(|error| AuraError::Storage(format!("Aura could not write the capture record during recovery: {error}")))?;

    Ok(())
}

/// Import-time decision representation. Supersession links are applied as
/// plain references to their original claim ids.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportedDecision {
    id: String,
    project_id: String,
    title: String,
    rationale: String,
    confidence: String,
    author_type: String,
    status: String,
    created_at: String,
    updated_at: String,
    supersedes_claim_id: Option<String>,
    superseded_by_claim_id: Option<String>,
    sources: Vec<ExportedSource>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportedSource {
    id: String,
    label: String,
    created_at: String,
}

fn apply_decision(transaction: &Connection, decision: ExportedDecision) -> Result<(), AuraError> {
    if decision.confidence != "low"
        && decision.confidence != "medium"
        && decision.confidence != "high"
    {
        return Err(AuraError::InvalidInput(format!(
            "The archive contains a decision with an unsupported confidence: {}.",
            decision.confidence
        )));
    }
    if decision.status != "confirmed" && decision.status != "superseded" {
        return Err(AuraError::InvalidInput(format!(
            "The archive contains a decision with an unsupported status: {}.",
            decision.status
        )));
    }

    transaction
        .execute(
            "INSERT INTO decision_claims (id, project_id, title, rationale, confidence, author_type, status, created_at, updated_at, supersedes_claim_id, superseded_by_claim_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(id) DO UPDATE SET
                 project_id = excluded.project_id,
                 title = excluded.title,
                 rationale = excluded.rationale,
                 confidence = excluded.confidence,
                 author_type = excluded.author_type,
                 status = excluded.status,
                 updated_at = excluded.updated_at,
                 supersedes_claim_id = excluded.supersedes_claim_id,
                 superseded_by_claim_id = excluded.superseded_by_claim_id",
            params![
                &decision.id,
                &decision.project_id,
                decision.title.trim(),
                decision.rationale,
                decision.confidence,
                decision.author_type,
                decision.status,
                &decision.created_at,
                &decision.updated_at,
                decision.supersedes_claim_id.as_deref(),
                decision.superseded_by_claim_id.as_deref(),
            ],
        )
        .map_err(|error| AuraError::Storage(format!("Aura could not write the decision claim during recovery: {error}")))?;

    for source in &decision.sources {
        transaction
            .execute(
                "INSERT INTO decision_sources (id, claim_id, project_id, label, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(id) DO UPDATE SET label = excluded.label, created_at = excluded.created_at",
                params![&source.id, &decision.id, &decision.project_id, source.label.trim(), &source.created_at],
            )
            .map_err(|error| AuraError::Storage(format!("Aura could not write a decision source during recovery: {error}")))?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Read helpers for the export side (typed captures/decisions as JSON).
// ---------------------------------------------------------------------------

impl<'store> ExportService<'store> {
    fn collect_captures(&self) -> Result<Vec<serde_json::Value>, AuraError> {
        let connection = self.store.connection_ref();
        let mut statement = connection
            .prepare(
                "SELECT id, project_id, kind, label, content, classification, retention, created_at
                 FROM captures
                 ORDER BY created_at ASC",
            )
            .map_err(|error| {
                AuraError::Storage(format!(
                    "Aura could not prepare its export capture query: {error}"
                ))
            })?;

        let records: Vec<serde_json::Value> = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                ))
            })
            .map_err(|error| {
                AuraError::Storage(format!(
                    "Aura could not read its captures for export: {error}"
                ))
            })?
            .filter_map(|row| row.ok())
            .map(
                |(id, project_id, kind, label, content, classification, retention, created_at)| {
                    serde_json::json!({
                        "id": id,
                        "projectId": project_id,
                        "kind": kind,
                        "label": label,
                        "content": content,
                        "classification": classification,
                        "retention": retention,
                        "createdAt": created_at,
                    })
                },
            )
            .collect();

        Ok(records)
    }

    fn collect_decisions(&self) -> Result<Vec<serde_json::Value>, AuraError> {
        let connection = self.store.connection_ref();
        let mut statement = connection
            .prepare(
                "SELECT id, project_id, title, rationale, confidence, author_type, status, created_at, updated_at, supersedes_claim_id, superseded_by_claim_id
                 FROM decision_claims
                 ORDER BY created_at ASC",
            )
            .map_err(|error| AuraError::Storage(format!("Aura could not prepare its export decision query: {error}")))?;

        let records: Vec<serde_json::Value> = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, Option<String>>(9)?,
                    row.get::<_, Option<String>>(10)?,
                ))
            })
            .map_err(|error| AuraError::Storage(format!("Aura could not read its decisions for export: {error}")))?
            .filter_map(|row| row.ok())
            .map(|(id, project_id, title, rationale, confidence, author_type, status, created_at, updated_at, supersedes_claim_id, superseded_by_claim_id)| {
                let sources = Self::decision_sources(connection, &id).unwrap_or_default();
                serde_json::json!({
                    "id": id,
                    "projectId": project_id,
                    "title": title,
                    "rationale": rationale,
                    "confidence": confidence,
                    "authorType": author_type,
                    "status": status,
                    "createdAt": created_at,
                    "updatedAt": updated_at,
                    "supersedesClaimId": supersedes_claim_id.map(serde_json::Value::String).unwrap_or(serde_json::Value::Null),
                    "supersededByClaimId": superseded_by_claim_id.map(serde_json::Value::String).unwrap_or(serde_json::Value::Null),
                    "sources": sources,
                })
            })
            .collect();

        Ok(records)
    }

    fn decision_sources(
        connection: &Connection,
        claim_id: &str,
    ) -> Result<Vec<serde_json::Value>, AuraError> {
        let mut statement = connection
            .prepare("SELECT id, label, created_at FROM decision_sources WHERE claim_id = ?1 ORDER BY created_at ASC")
            .map_err(|error| AuraError::Storage(format!("Aura could not prepare its export decision source query: {error}")))?;

        let sources: Vec<serde_json::Value> = statement
            .query_map([claim_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(|error| {
                AuraError::Storage(format!(
                    "Aura could not read decision sources for export: {error}"
                ))
            })?
            .filter_map(|row| row.ok())
            .map(|(id, label, created_at)| {
                serde_json::json!({
                    "id": id,
                    "label": label,
                    "createdAt": created_at,
                })
            })
            .collect();

        Ok(sources)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{repositories::projects::CreateProject, LocalStore};
    use crate::domain::export::ExportEvent;

    /// A fresh, fully-migrated in-memory workspace with its own key vault,
    /// mirroring the setup the production `run()` performs.
    ///
    /// When `key_vault` is provided, the new workspace reuses that vault
    /// directory so it unwraps the same data-encryption key. This is how
    /// "restore on the same machine" is simulated: the archive stays bound
    /// to the exporting vault's key (see the work package and ADR-005).
    fn fresh_workspace() -> (LocalStore, KeyVault) {
        let store = LocalStore::open_in_memory().expect("open in-memory store");
        let directory = std::env::temp_dir().join(format!("aura-export-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&directory).expect("create temp directory");
        let cleanup_directory = directory.clone();
        // Clean up the temp vault directory after the test.
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(1));
            let _ = std::fs::remove_dir_all(cleanup_directory);
        });
        let key_vault = KeyVault::new(&directory).expect("open key vault");
        (store, key_vault)
    }

    /// Same as `fresh_workspace`, but seeding the new workspace with the same
    /// wrapped key as `existing_vault` so it can open archives the exporting
    /// workspace sealed (same-user restore scenario).
    fn fresh_workspace_with_shared_key(existing_vault: &KeyVault) -> (LocalStore, KeyVault) {
        use std::io::Read;
        use std::io::Write;

        let store = LocalStore::open_in_memory().expect("open in-memory store");
        let directory = std::env::temp_dir().join(format!("aura-export-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&directory).expect("create temp directory");
        let source_path = existing_vault.data_directory().join("aura.keywrap");
        if source_path.exists() {
            let mut blob = Vec::new();
            std::fs::File::open(&source_path)
                .and_then(|mut file| file.read_to_end(&mut blob))
                .expect("read wrapped key blob");
            std::fs::File::create(directory.join("aura.keywrap"))
                .and_then(|mut file| file.write_all(&blob))
                .expect("write wrapped key blob");
        }
        let key_vault = KeyVault::new(&directory).expect("open key vault");
        (store, key_vault)
    }

    /// Seed the workspace with one project, one capture, one decision, one
    /// exclusion rule, and both persisted settings so the export envelope
    /// carries every record kind Aura owns today.
    fn seed_workspace(store: &mut LocalStore) -> String {
        let project = store
            .projects()
            .create(CreateProject {
                name: "Export Test Project".to_string(),
                goal: Some("goal".to_string()),
                current_task: Some("task".to_string()),
                next_step: Some("next".to_string()),
            })
            .expect("create project");
        let connection = store.connection_ref();
        connection
            .execute(
                "INSERT INTO captures (id, project_id, kind, label, content, classification, retention, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![
                    Uuid::new_v4().to_string(),
                    &project.id,
                    "manual_note",
                    "seed note",
                    "seeded content",
                    "standard",
                    "until_deleted",
                    utc_timestamp(),
                ],
            )
            .expect("seed capture");
        connection
            .execute(
                "INSERT INTO decision_claims (id, project_id, title, rationale, confidence, author_type, status, created_at, updated_at, supersedes_claim_id, superseded_by_claim_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL, NULL)",
                rusqlite::params![
                    Uuid::new_v4().to_string(),
                    &project.id,
                    "Seed Decision",
                    "seeded rationale",
                    "high",
                    "user",
                    "confirmed",
                    utc_timestamp(),
                    utc_timestamp(),
                ],
            )
            .expect("seed decision");
        connection
            .execute(
                "INSERT INTO exclusion_rules (id, rule_type, value, is_enabled, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![
                    Uuid::new_v4().to_string(),
                    "domain",
                    "example.com",
                    1,
                    utc_timestamp(),
                    utc_timestamp(),
                ],
            )
            .expect("seed exclusion");
        connection
            .execute(
                "INSERT INTO settings (key, value, updated_at) VALUES ('privacy_mode', 'manual_only', ?1)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
                rusqlite::params![utc_timestamp()],
            )
            .expect("seed setting");
        project.id
    }

    fn service<'a>(store: &'a mut LocalStore, key_vault: &'a KeyVault) -> ExportService<'a> {
        ExportService::new(store, key_vault)
    }

    fn envelope_bytes(store: &mut LocalStore, key_vault: &KeyVault) -> Vec<u8> {
        let envelope = service(store, key_vault)
            .assemble_export()
            .expect("assemble export");
        serde_json::to_vec(&envelope).expect("encode envelope")
    }

    #[test]
    fn empty_workspace_produces_a_sealed_envelope_with_plaintext_manifest() {
        let (mut store, key_vault) = fresh_workspace();
        let raw = envelope_bytes(&mut store, &key_vault);

        let manifest = ExportService::envelope_manifest(&raw).expect("read manifest");

        assert_eq!(manifest.format_version, EXPORT_FORMAT_VERSION);
        assert_eq!(manifest.record_counts.projects, 0);
        assert_eq!(manifest.record_counts.captures, 0);
        assert_eq!(manifest.record_counts.decisions, 0);
        assert_eq!(manifest.record_counts.exclusion_rules, 0);
        assert!(!manifest.exported_at.is_empty());
        assert!(!manifest.payload_checksum.is_empty());
        assert!(manifest.payload_sealed_length > 0);
    }

    #[test]
    fn seeded_workspace_records_every_record_kind_in_the_manifest() {
        let (mut store, key_vault) = fresh_workspace();
        seed_workspace(&mut store);
        let raw = envelope_bytes(&mut store, &key_vault);

        let manifest = ExportService::envelope_manifest(&raw).expect("read manifest");

        assert_eq!(manifest.record_counts.projects, 1);
        assert_eq!(manifest.record_counts.captures, 1);
        assert_eq!(manifest.record_counts.decisions, 1);
        assert_eq!(manifest.record_counts.exclusion_rules, 1);
        assert_eq!(manifest.record_counts.settings, 2);
    }

    #[test]
    fn sealed_payload_cannot_be_read_without_the_bound_key() {
        let (mut store, key_vault) = fresh_workspace();
        seed_workspace(&mut store);
        let envelope: ExportEnvelope =
            serde_json::from_slice(&envelope_bytes(&mut store, &key_vault))
                .expect("decode envelope");

        let sealed =
            KeyVault::decode_sealed(&hex::decode(&envelope.payload_sealed_hex).expect("valid hex"))
                .expect("decode sealed");
        let payload_bytes = key_vault.open(&sealed).expect("open with bound key");
        let recovered: ExportPayload =
            serde_json::from_slice(&payload_bytes).expect("payload decodes");

        assert_eq!(recovered.projects.len(), 1);
        assert_eq!(recovered.captures.len(), 1);
        assert_eq!(recovered.decisions.len(), 1);
        assert_eq!(recovered.exclusion_rules.len(), 1);

        // A fresh vault (simulating a different machine or user) cannot open
        // the same sealed payload.
        let (mut _other_store, other_vault) = fresh_workspace();
        let other_result = other_vault.open(&sealed);
        assert!(
            other_result.is_err(),
            "a foreign key vault must never open a sealed export payload"
        );
    }

    #[test]
    fn tampered_checksum_is_rejected_by_the_import_path() {
        let (mut store, key_vault) = fresh_workspace();
        seed_workspace(&mut store);
        let raw_before = store
            .connection_ref()
            .query_row("SELECT COUNT(*) FROM projects", [], |row| {
                row.get::<_, i64>(0)
            })
            .expect("count projects");
        let mut raw = envelope_bytes(&mut store, &key_vault);

        // Corrupt one byte inside the sealed payload region so the checksum
        // no longer matches and AEAD authentication fails.
        let mut envelope: ExportEnvelope = serde_json::from_slice(&raw).expect("decode envelope");
        let mut sealed = hex::decode(&envelope.payload_sealed_hex).expect("valid hex");
        assert!(
            !sealed.is_empty(),
            "the envelope must carry a sealed payload"
        );
        sealed[0] = sealed[0].wrapping_add(1);
        envelope.payload_sealed_hex = hex::encode(&sealed);
        raw = serde_json::to_vec(&envelope).expect("encode envelope");

        let mut service = service(&mut store, &key_vault);
        let result = service.apply_import(&raw);

        assert!(
            result.is_err(),
            "a tampered archive must fail validation: OK={:?}",
            result.is_ok()
        );
        // The workspace must remain untouched: no new records were inserted.
        let connection = store.connection_ref();
        let project_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM projects", [], |row| row.get(0))
            .expect("count projects");
        assert_eq!(
            project_count, raw_before,
            "a failed import must not write anything"
        );
    }

    #[test]
    fn unsupported_format_version_is_rejected() {
        let (mut store, key_vault) = fresh_workspace();
        let mut envelope: ExportEnvelope =
            serde_json::from_slice(&envelope_bytes(&mut store, &key_vault))
                .expect("decode envelope");
        envelope.format_version = 99;
        let raw = serde_json::to_vec(&envelope).expect("encode envelope");

        assert!(
            ExportService::envelope_manifest(&raw).is_err(),
            "the manifest preview must reject unknown format versions"
        );
        assert!(
            service(&mut store, &key_vault).apply_import(&raw).is_err(),
            "the import path must reject unknown format versions"
        );
    }

    #[test]
    fn import_applies_all_records_transactionally() {
        let (mut source_store, key_vault) = fresh_workspace();
        let _project_id = seed_workspace(&mut source_store);
        let raw = envelope_bytes(&mut source_store, &key_vault);

        let (mut destination, destination_vault) = fresh_workspace_with_shared_key(&key_vault);
        let counts = service(&mut destination, &destination_vault)
            .apply_import(&raw)
            .expect("apply import");

        assert_eq!(counts.projects, 1);
        assert_eq!(counts.captures, 1);
        assert_eq!(counts.decisions, 1);
        assert_eq!(counts.exclusion_rules, 1);
        assert_eq!(counts.settings, 2);

        let connection = destination.connection_ref();
        let project_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM projects", [], |row| row.get(0))
            .expect("count projects");
        let capture_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM captures", [], |row| row.get(0))
            .expect("count captures");
        let decision_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM decision_claims", [], |row| row.get(0))
            .expect("count decisions");
        assert_eq!(project_count, 1);
        assert_eq!(capture_count, 1);
        assert_eq!(decision_count, 1);
    }

    #[test]
    fn import_rolls_back_entirely_when_an_inner_record_conflicts_unrecoverably() {
        let (mut source_store, key_vault) = fresh_workspace();
        let _project_id = seed_workspace(&mut source_store);
        let raw = envelope_bytes(&mut source_store, &key_vault);

        // Drop a target table so the transaction cannot complete. The import
        // must leave every other table untouched.
        let (mut destination, destination_vault) = fresh_workspace_with_shared_key(&key_vault);
        destination
            .connection_ref_mut()
            .execute_batch("DROP TABLE captures")
            .expect("drop captures table");

        let result = service(&mut destination, &destination_vault).apply_import(&raw);

        assert!(
            result.is_err(),
            "the import must fail when its transaction cannot apply"
        );
        let connection = destination.connection_ref();
        let project_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM projects", [], |row| row.get(0))
            .expect("count projects");
        let decision_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM decision_claims", [], |row| row.get(0))
            .expect("count decisions");
        assert_eq!(
            project_count, 0,
            "projects must not be partially applied when the import transaction fails"
        );
        assert_eq!(
            decision_count, 0,
            "decisions must not be partially applied when the import transaction fails"
        );
    }

    #[test]
    fn damaged_archive_that_fails_authentication_leaves_the_workspace_untouched() {
        let (mut source_store, key_vault) = fresh_workspace();
        seed_workspace(&mut source_store);
        let mut envelope: ExportEnvelope =
            serde_json::from_slice(&envelope_bytes(&mut source_store, &key_vault))
                .expect("decode envelope");
        // Flip bits deep in the ciphertext: the checksum check catches this
        // first, and even if it did not, AEAD authentication would.
        let sealed_bytes = hex::decode(&envelope.payload_sealed_hex).expect("valid hex");
        let mut flipped = sealed_bytes.clone();
        if let Some(last) = flipped.last_mut() {
            *last ^= 0xFF;
        }
        envelope.payload_sealed_hex = hex::encode(&flipped);
        let raw = serde_json::to_vec(&envelope).expect("encode envelope");

        let (mut destination, destination_vault) = fresh_workspace_with_shared_key(&key_vault);
        assert!(
            service(&mut destination, &destination_vault)
                .apply_import(&raw)
                .is_err(),
            "a damaged sealed payload must fail authentication"
        );
        let connection = destination.connection_ref();
        let project_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM projects", [], |row| row.get(0))
            .expect("count projects");
        assert_eq!(project_count, 0);
    }

    #[test]
    fn export_and_import_attempts_are_recorded_in_the_audit_table() {
        let (mut store, key_vault) = fresh_workspace();
        seed_workspace(&mut store);

        let service = service(&mut store, &key_vault);
        let envelope = service.assemble_export().expect("assemble export");
        let _raw = serde_json::to_vec(&envelope).expect("encode envelope");

        service
            .record_export_event(
                ExportEvent::ExportRequested,
                &ExportRecordCounts::default(),
                "",
            )
            .expect("record export request");
        service
            .record_export_event(
                ExportEvent::ExportCompleted,
                &envelope.record_counts,
                "audit test",
            )
            .expect("record export completion");
        service
            .record_export_event(ExportEvent::ImportRequested, &envelope.record_counts, "")
            .expect("record import request");
        service
            .record_export_event(
                ExportEvent::ImportFailed,
                &envelope.record_counts,
                "audit test failure",
            )
            .expect("record import failure");

        let connection = store.connection_ref();
        let event_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM export_metadata", [], |row| row.get(0))
            .expect("count audit events");
        assert_eq!(event_count, 4);

        let import_failed_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM export_metadata WHERE event_type = 'import_failed'",
                [],
                |row| row.get(0),
            )
            .expect("count import failed events");
        assert_eq!(import_failed_count, 1);

        // Every requested attempt recorded its matching completion outcome.
        let kinds: Vec<String> = connection
            .prepare("SELECT event_type FROM export_metadata ORDER BY sequence")
            .expect("prepare")
            .query_map([], |row| row.get::<_, String>(0))
            .expect("read kinds")
            .map(Result::unwrap)
            .collect();
        assert_eq!(
            kinds,
            vec![
                "export_requested",
                "export_completed",
                "import_requested",
                "import_failed"
            ],
            "audit events must appear in invocation order"
        );
    }

    #[test]
    fn import_rejects_an_archive_whose_record_inventory_does_not_match_its_manifest() {
        let (mut source_store, key_vault) = fresh_workspace();
        seed_workspace(&mut source_store);
        let mut envelope: ExportEnvelope =
            serde_json::from_slice(&envelope_bytes(&mut source_store, &key_vault))
                .expect("decode envelope");
        envelope.record_counts.projects = 999;
        let raw = serde_json::to_vec(&envelope).expect("encode envelope");

        let (mut destination, destination_vault) = fresh_workspace_with_shared_key(&key_vault);
        assert!(
            service(&mut destination, &destination_vault)
                .apply_import(&raw)
                .is_err(),
            "a mismatched inventory must fail the import"
        );
    }

    #[test]
    fn export_contains_no_sensitive_record_kinds() {
        // Aura's schema itself holds no screenshots, clipboard contents,
        // microphone data, or credentials; this test pins that invariant on
        // the exported payload structure so a future schema change cannot
        // silently widen what the archive carries.
        let (mut store, key_vault) = fresh_workspace();
        seed_workspace(&mut store);
        let envelope: ExportEnvelope =
            serde_json::from_slice(&envelope_bytes(&mut store, &key_vault))
                .expect("decode envelope");

        let sealed =
            KeyVault::decode_sealed(&hex::decode(&envelope.payload_sealed_hex).expect("valid hex"))
                .expect("decode sealed");
        let payload_bytes = key_vault.open(&sealed).expect("open payload");
        let payload: ExportPayload = serde_json::from_slice(&payload_bytes).expect("decode");

        let json = serde_json::to_string(&payload).expect("serialize");
        assert!(!json.contains("screenshot"));
        assert!(!json.contains("clipboard"));
        assert!(!json.contains("microphone"));
        assert!(!json.contains("password"));
        assert!(!json.contains("http://"));
        assert!(!json.contains("https://"));
    }

    // -----------------------------------------------------------------------
    // EXP-002: passphrase-sealed portable archives.
    // -----------------------------------------------------------------------

    #[test]
    fn passphrase_export_roundtrips_on_a_completely_fresh_installation() {
        let (mut source_store, source_vault) = fresh_workspace();
        seed_workspace(&mut source_store);

        let passphrase = Passphrase::new("Twilight-Sparkle-42".to_string());
        let envelope = service(&mut source_store, &source_vault)
            .assemble_passphrase_export(Passphrase::new("Twilight-Sparkle-42".to_string()))
            .expect("assemble passphrase export");

        assert_eq!(envelope.format_version, EXPORT_FORMAT_VERSION);
        assert_eq!(envelope.sealing, PASSPHRASE_SEALING);
        assert_eq!(envelope.record_counts.projects, 1);
        assert_eq!(envelope.record_counts.decisions, 1);
        assert!(envelope.passphrase_salt_hex.is_some());
        assert!(envelope.passphrase_params.is_some());

        let manifest = ExportService::envelope_manifest(
            &serde_json::to_vec(&envelope).expect("encode envelope"),
        )
        .expect("read manifest");
        assert_eq!(manifest.sealing, PASSPHRASE_SEALING);

        // Restore on a brand-new workspace whose key vault was never touched
        // by the exporter. A passphrase archive must not depend on the
        // exporting machine's workspace key.
        let (mut destination, _destination_vault) = fresh_workspace();
        let mut service = service(&mut destination, &_destination_vault);
        service.set_passphrase(Some(passphrase));
        let raw = serde_json::to_vec(&envelope).expect("encode envelope");
        let counts = service.apply_import(&raw).expect("apply import");
        assert_eq!(counts.projects, 1);
        assert_eq!(counts.decisions, 1);

        let connection = destination.connection_ref();
        let projects: i64 = connection
            .query_row("SELECT COUNT(*) FROM projects", [], |row| row.get(0))
            .expect("count projects");
        assert_eq!(projects, 1, "the project must have been recovered");
    }

    #[test]
    fn passphrase_export_rejects_a_weak_passphrase() {
        let (mut store, key_vault) = fresh_workspace();
        seed_workspace(&mut store);

        let weak = service(&mut store, &key_vault)
            .assemble_passphrase_export(Passphrase::new("weak1".to_string()));
        assert!(
            weak.is_err(),
            "a weak passphrase must not produce an archive"
        );

        let strong = service(&mut store, &key_vault)
            .assemble_passphrase_export(Passphrase::new("Strong-Passphrase-11".to_string()));
        assert!(strong.is_ok(), "a strong passphrase must be accepted");
    }

    #[test]
    fn passphrase_archive_requires_its_passphrase_to_restore() {
        let (mut source_store, source_vault) = fresh_workspace();
        seed_workspace(&mut source_store);
        let envelope = service(&mut source_store, &source_vault)
            .assemble_passphrase_export(Passphrase::new("Twilight-Sparkle-42".to_string()))
            .expect("assemble passphrase export");
        let raw = serde_json::to_vec(&envelope).expect("encode envelope");

        let (mut destination, destination_vault) = fresh_workspace();

        let without = service(&mut destination, &destination_vault).apply_import(&raw);
        assert!(
            without.is_err(),
            "an archive without its passphrase must fail to open"
        );

        let wrong = {
            let mut service = service(&mut destination, &destination_vault);
            service.set_passphrase(Some(Passphrase::new("Twilight-Sparkle-41".to_string())));
            service.apply_import(&raw)
        };
        assert!(
            wrong.is_err(),
            "a wrong passphrase must fail authentication before any record is written"
        );

        // No record survived either failure; the destination workspace stays
        // empty.
        let projects: i64 = destination
            .connection_ref()
            .query_row("SELECT COUNT(*) FROM projects", [], |row| row.get(0))
            .expect("count projects");
        assert_eq!(projects, 0, "failed imports must not apply partial data");
    }

    #[test]
    fn passphrase_archive_survives_being_read_on_a_different_machine() {
        // Simulates the portable restore: the envelope travels as raw bytes
        // to a fresh installation with a different key vault, and the
        // destination derives the same key from the stored salt plus the
        // remembered passphrase.
        let (mut source_store, source_vault) = fresh_workspace();
        seed_workspace(&mut source_store);
        let raw = serde_json::to_vec(
            &service(&mut source_store, &source_vault)
                .assemble_passphrase_export(Passphrase::new("Twilight-Sparkle-42".to_string()))
                .expect("assemble"),
        )
        .expect("encode");

        let (mut destination, _destination_vault) = fresh_workspace();
        let mut service = service(&mut destination, &_destination_vault);
        service.set_passphrase(Some(Passphrase::new("Twilight-Sparkle-42".to_string())));
        let counts = service.apply_import(&raw).expect("apply import");
        assert_eq!(counts.captures, 1);
        assert_eq!(counts.exclusion_rules, 1);
        assert_eq!(counts.settings, 2);
    }

    #[test]
    fn version_one_dpapi_archive_still_opens() {
        // A v1 envelope has no `sealing` field; deserialization must fall
        // back to the DPAPI default, and the import must still use the bound
        // workspace key (which a DPAPI archive requires).
        let (mut source_store, source_vault) = fresh_workspace();
        seed_workspace(&mut source_store);
        let mut v1_envelope: ExportEnvelope =
            serde_json::from_slice(&envelope_bytes(&mut source_store, &source_vault))
                .expect("decode");
        v1_envelope.format_version = 1;
        let raw = serde_json::to_vec(&v1_envelope).expect("encode v1 envelope");

        let (mut destination, destination_vault) = fresh_workspace_with_shared_key(&source_vault);
        let counts = service(&mut destination, &destination_vault)
            .apply_import(&raw)
            .expect("v1 archives remain importable");
        assert_eq!(counts.projects, 1);
    }

    #[test]
    fn envelope_serialization_carries_public_parameters_only() {
        let (mut store, key_vault) = fresh_workspace();
        seed_workspace(&mut store);
        let envelope = service(&mut store, &key_vault)
            .assemble_passphrase_export(Passphrase::new("Twilight-Sparkle-42".to_string()))
            .expect("assemble");

        let json = serde_json::to_string(&envelope).expect("serialize envelope");
        // The passphrase itself, its bytes, and any derivation output must
        // not appear in the transport form. Salt and parameters are
        // intentionally public.
        assert!(!json.contains("Twilight-Sparkle-42"));
        assert!(json.contains("\"sealing\":\"passphrase\""));
        assert!(json.contains("\"memoryCostKib\":19456"));
        assert!(envelope.passphrase_salt_hex.is_some());
        assert_eq!(
            envelope.passphrase_params,
            Some(PassphraseParams::owasp_defaults())
        );
    }
}
