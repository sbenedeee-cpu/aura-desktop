// EXP-001: typed export and recovery contracts.
//
// The renderer never reads the database directly: export and import flow
// through these DTOs. The exported file is a versioned, self-describing
// envelope whose plaintext manifest lists what the archive contains, while
// all record content stays sealed with the DPAPI-bound key (ADR-004).

use serde::{Deserialize, Serialize};

/// On-disk envelope format version. The manifest carries it so future format
/// changes are a controlled migration rather than a silent break.
pub const EXPORT_FORMAT_VERSION: i64 = 2;

/// Sealing discriminator carried in the envelope. `"dpapi"` keeps the
/// EXP-001 behavior; `"passphrase"` marks an archive re-sealed with a
/// passphrase-derived key (EXP-002). Absent in version-1 envelopes, where
/// the DPAPI boundary is the only supported sealing. Used by the command
/// layer to decide which key must open an archive.
#[allow(dead_code)]
pub const DPAPI_SEALING: &str = "dpapi";
pub const PASSPHRASE_SEALING: &str = "passphrase";

/// Plaintext, human-readable inventory of an exported archive. No decrypted
/// record content ever appears here.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportManifest {
    pub format_version: i64,
    /// How the payload was sealed: `"dpapi"` or `"passphrase"`.
    pub sealing: String,
    pub exported_at: String,
    pub exported_by_version: String,
    pub record_counts: ExportRecordCounts,
    pub payload_checksum: String,
    pub payload_sealed_length: usize,
}

/// How many records of each kind the export envelope carries.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRecordCounts {
    pub projects: usize,
    pub captures: usize,
    pub decisions: usize,
    pub exclusion_rules: usize,
    pub settings: usize,
}

/// A single typed setting value included in the export envelope.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportedSetting {
    pub key: String,
    pub value: String,
}

/// The decrypted payload of an export envelope: everything the user owns.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportPayload {
    pub projects: Vec<serde_json::Value>,
    pub captures: Vec<serde_json::Value>,
    pub decisions: Vec<serde_json::Value>,
    pub exclusion_rules: Vec<serde_json::Value>,
    pub settings: Vec<ExportedSetting>,
}

impl ExportPayload {
    /// Record inventory matching the manifest counts. Counts are derived
    /// from the payload itself, so a tampered record list cannot silently
    /// pass the manifest check.
    pub fn record_counts(&self) -> ExportRecordCounts {
        ExportRecordCounts {
            projects: self.projects.len(),
            captures: self.captures.len(),
            decisions: self.decisions.len(),
            exclusion_rules: self.exclusion_rules.len(),
            settings: self.settings.len(),
        }
    }
}

/// Summary of an export or import attempt written to the append-only
/// `export_metadata` audit table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExportEvent {
    ExportRequested,
    ExportCompleted,
    ExportFailed,
    ImportRequested,
    ImportCompleted,
    ImportFailed,
}

impl ExportEvent {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ExportRequested => "export_requested",
            Self::ExportCompleted => "export_completed",
            Self::ExportFailed => "export_failed",
            Self::ImportRequested => "import_requested",
            Self::ImportCompleted => "import_completed",
            Self::ImportFailed => "import_failed",
        }
    }
}

/// A single row of the export-and-recovery audit log.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct ExportEventRecord {
    pub id: String,
    pub event_type: String,
    pub envelope_format_version: i64,
    pub record_counts: String,
    pub detail: String,
    pub created_at: String,
}
