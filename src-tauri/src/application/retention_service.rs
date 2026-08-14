use chrono::{Duration, Utc};

use crate::db::repositories::captures::LifecycleCapture;
use crate::domain::capture::CaptureClassification;
use crate::domain::project::AuraError;
use crate::domain::retention::{RetentionSweepResult, ReviewableCapture};

/// EXP-003: retention sweep / context-ageing policy.
///
/// This module is deliberately pure of database access: the database
/// repository supplies raw rows and applies the transitions, while the policy
/// (which captures are reviewable, which are protected, when a capture
/// becomes aged) lives here so it can be exercised by unit tests with a fake
/// clock.
pub const REVIEW_AFTER_DAYS: i64 = 30;

pub struct RetentionService {
    now: chrono::DateTime<Utc>,
}

impl RetentionService {
    pub fn new(now: chrono::DateTime<Utc>) -> Self {
        Self { now }
    }

    /// Current policy time for the sweep. Tests inject the clock; production
    /// uses the real UTC wall clock at sweep start so a long-running pass is
    /// internally consistent. Kept as part of the public sweep surface even
    /// when no caller references it yet.
    #[allow(dead_code)]
    pub fn now(&self) -> chrono::DateTime<Utc> {
        self.now
    }

    /// Captures eligible for age transition in this pass.
    ///
    /// Rules, in priority order:
    /// 1. `deleted` or already `aged` captures are skipped entirely.
    /// 2. Captures whose retention policy is `until_deleted` are never aged;
    ///    the user's retention choice is respected as a boundary.
    /// 3. `sensitive` captures are never aged automatically: privacy-by-default
    ///    means higher-stakes context waits for a deliberate human decision.
    /// 4. `review_in_30_days` captures created at least `REVIEW_AFTER_DAYS`
    ///    ago become reviewable (aged).
    pub fn classify_pass(
        &self,
        captures: &[LifecycleCapture],
    ) -> Result<(Vec<ReviewableCapture>, RetentionSweepResult), AuraError> {
        let cutoff = self.now - Duration::days(REVIEW_AFTER_DAYS);
        let swept_at = self
            .now
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

        let mut reviewable = Vec::new();
        let mut captures_aged_now = 0_u64;
        let mut captures_already_aged = 0_u64;
        let mut captures_protected = 0_u64;

        for capture in captures {
            if capture.lifecycle_state == "aged" {
                captures_already_aged += 1;
                continue;
            }
            if capture.lifecycle_state != "active" {
                continue;
            }
            let created = capture
                .created_at
                .parse::<chrono::DateTime<Utc>>()
                .map_err(|_| {
                    AuraError::Storage(
                        "Aura could not read a capture timestamp while ageing context.".to_string(),
                    )
                })?;
            let classification = CaptureClassification::from_store(&capture.classification)
                .map_err(|_| {
                    AuraError::Storage(
                        "Aura found an unsupported capture classification while sweeping."
                            .to_string(),
                    )
                })?;
            let retention = crate::domain::capture::CaptureRetention::from_store(
                &capture.retention,
            )
            .map_err(|_| {
                AuraError::Storage(
                    "Aura found an unsupported capture retention policy while sweeping."
                        .to_string(),
                )
            })?;

            if classification == CaptureClassification::Sensitive
                || retention == crate::domain::capture::CaptureRetention::UntilDeleted
            {
                captures_protected += 1;
                continue;
            }
            if created < cutoff {
                captures_aged_now += 1;
                let days_aged = self.now.signed_duration_since(created).num_days().max(0);
                reviewable.push(ReviewableCapture {
                    id: capture.id.clone(),
                    project_id: capture.project_id.clone(),
                    label: capture.label.clone(),
                    classification: capture.classification.clone(),
                    created_at: capture.created_at.clone(),
                    aged_at: swept_at.clone(),
                    days_aged,
                });
            }
        }

        let result = RetentionSweepResult {
            swept_at,
            captures_reviewed: captures.len() as u64,
            captures_aged_now,
            captures_already_aged,
            captures_protected,
        };

        Ok((reviewable, result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repositories::captures::LifecycleCapture;

    fn capture(
        retention: &str,
        classification: &str,
        days_old: i64,
        state: &str,
    ) -> LifecycleCapture {
        let created = Utc::now() - Duration::days(days_old);
        LifecycleCapture {
            id: format!("cap-{retention}-{classification}-{state}"),
            project_id: "project-id".to_string(),
            kind: "manual_note".to_string(),
            label: "Retention test capture".to_string(),
            content: "Aging context used by retention sweep tests.".to_string(),
            classification: classification.to_string(),
            retention: retention.to_string(),
            created_at: created.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            lifecycle_state: state.to_string(),
            lifecycle_updated_at: created.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        }
    }

    #[test]
    fn standard_review_capture_becomes_aged_after_thirty_days() {
        let service = RetentionService::new(Utc::now());
        let captures = vec![capture("review_in_30_days", "standard", 31, "active")];
        let (reviewable, result) = service.classify_pass(&captures).expect("classify pass");

        assert_eq!(reviewable.len(), 1);
        assert_eq!(result.captures_aged_now, 1);
        assert_eq!(result.captures_protected, 0);
        assert_eq!(reviewable[0].days_aged, 31);
    }

    #[test]
    fn capture_younger_than_thirty_days_stays_active() {
        let service = RetentionService::new(Utc::now());
        let captures = vec![capture("review_in_30_days", "standard", 12, "active")];
        let (_, result) = service.classify_pass(&captures).expect("classify pass");

        assert_eq!(result.captures_aged_now, 0);
    }

    #[test]
    fn until_deleted_retention_is_never_aged() {
        let service = RetentionService::new(Utc::now());
        let captures = vec![capture("until_deleted", "standard", 400, "active")];
        let (reviewable, result) = service.classify_pass(&captures).expect("classify pass");

        assert!(reviewable.is_empty());
        assert_eq!(result.captures_protected, 1);
    }

    #[test]
    fn sensitive_captures_are_never_aged_automatically() {
        let service = RetentionService::new(Utc::now());
        let captures = vec![capture("review_in_30_days", "sensitive", 60, "active")];
        let (reviewable, result) = service.classify_pass(&captures).expect("classify pass");

        assert!(reviewable.is_empty());
        assert_eq!(result.captures_protected, 1);
    }

    #[test]
    fn already_aged_captures_are_counted_but_not_re_aged() {
        let service = RetentionService::new(Utc::now());
        let captures = vec![
            capture("review_in_30_days", "standard", 45, "aged"),
            capture("review_in_30_days", "standard", 31, "active"),
        ];
        let (reviewable, result) = service.classify_pass(&captures).expect("classify pass");

        assert_eq!(reviewable.len(), 1);
        assert_eq!(result.captures_already_aged, 1);
        assert_eq!(result.captures_aged_now, 1);
    }

    #[test]
    fn deleted_captures_are_ignored_by_the_sweep() {
        let service = RetentionService::new(Utc::now());
        let captures = vec![capture("review_in_30_days", "standard", 31, "deleted")];
        let (reviewable, result) = service.classify_pass(&captures).expect("classify pass");

        assert!(reviewable.is_empty());
        assert_eq!(result.captures_reviewed, 1);
        assert_eq!(result.captures_aged_now, 0);
    }

    #[test]
    fn empty_workspace_sweep_is_harmless() {
        let service = RetentionService::new(Utc::now());
        let (_, result) = service.classify_pass(&[]).expect("classify pass");

        assert_eq!(result.captures_reviewed, 0);
        assert_eq!(result.captures_aged_now, 0);
    }
}
