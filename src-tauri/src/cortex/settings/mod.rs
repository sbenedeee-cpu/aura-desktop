// EXP-007: the Settings Store — Neural Cortex configuration.
//
// One JSON file in the app-data directory holds all Cortex configuration.
// Secrets (API keys) never appear in the file as plaintext: they are sealed
// through the KeyVault (DPAPI-wrapped envelope per ADR-003) and only the
// sealed blob travels to disk. Non-secret preferences (brain mode, STT
// mode, voice reply, ageing window) are stored plain so the user can
// inspect and edit the file by hand if ever needed.
//
// Module layout:
//   store     — persistence, field-level updates, defaults, validation
//   ../brain  — the reasoning engine that reads these settings at request time

pub mod store;

pub use store::{AssistantSettings, BrainMode, CloudProvider};
