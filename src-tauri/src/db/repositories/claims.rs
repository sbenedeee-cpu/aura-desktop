use crate::{
    db::migrations::utc_timestamp,
    domain::{
        claim::{ClaimConfidence, ClaimSource, ClaimStatus, DecisionClaim},
        project::AuraError,
    },
};
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct CreateDecision {
    pub project_id: String,
    pub title: String,
    pub rationale: String,
    pub confidence: ClaimConfidence,
    pub source_labels: Vec<String>,
}

pub struct DecisionRepository<'connection> {
    connection: &'connection Connection,
}

impl<'connection> DecisionRepository<'connection> {
    pub fn new(connection: &'connection Connection) -> Self {
        Self { connection }
    }

    pub fn create(&self, input: CreateDecision) -> Result<DecisionClaim, AuraError> {
        self.ensure_active_project(&input.project_id)?;
        self.create_transaction(input, None, None)
    }

    pub fn supersede(
        &self,
        project_id: String,
        previous_claim_id: String,
        title: String,
        rationale: String,
        confidence: ClaimConfidence,
        source_labels: Vec<String>,
    ) -> Result<DecisionClaim, AuraError> {
        self.ensure_active_project(&project_id)?;
        let previous = self
            .find_by_id_for_project(&project_id, &previous_claim_id)?
            .ok_or_else(|| {
                AuraError::NotFound(
                    "Aura cannot correct a decision that is not stored in this local project."
                        .to_string(),
                )
            })?;
        if previous.status != "confirmed" {
            return Err(AuraError::InvalidInput(
                "Only the current version of a decision can be corrected. Review the latest local record."
                    .to_string(),
            ));
        }

        self.create_transaction(
            CreateDecision {
                project_id,
                title,
                rationale,
                confidence,
                source_labels,
            },
            Some(previous.id),
            Some(previous.title),
        )
    }

    pub fn list_for_project(&self, project_id: &str) -> Result<Vec<DecisionClaim>, AuraError> {
        self.ensure_project_exists(project_id)?;
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, project_id, title, rationale, confidence, status, created_at, updated_at,
                        supersedes_claim_id, superseded_by_claim_id
                 FROM decision_claims WHERE project_id = ?1 ORDER BY created_at DESC, id DESC",
            )
            .map_err(storage_error("read local decisions"))?;
        let rows = statement
            .query_map([project_id], claim_row)
            .map_err(storage_error("read local decisions"))?;
        rows.map(|row| self.claim_from_row(row.map_err(storage_error("read local decision"))?))
            .collect()
    }

    fn create_transaction(
        &self,
        input: CreateDecision,
        supersedes_claim_id: Option<String>,
        superseded_title: Option<String>,
    ) -> Result<DecisionClaim, AuraError> {
        let title = required_text("Decision", input.title, 160)?;
        let rationale = required_text("Rationale", input.rationale, 4_000)?;
        let source_labels = source_labels(input.source_labels)?;
        let now = utc_timestamp();
        let claim = DecisionClaim {
            id: Uuid::new_v4().to_string(),
            project_id: input.project_id,
            title,
            rationale,
            confidence: input.confidence.as_str().to_string(),
            author_type: "user".to_string(),
            status: ClaimStatus::Confirmed.as_str().to_string(),
            created_at: now.clone(),
            updated_at: now.clone(),
            supersedes_claim_id: supersedes_claim_id.clone(),
            superseded_by_claim_id: None,
            sources: source_labels
                .iter()
                .map(|label| ClaimSource {
                    id: Uuid::new_v4().to_string(),
                    label: label.clone(),
                    created_at: now.clone(),
                })
                .collect(),
        };
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(storage_error("begin decision storage"))?;
        transaction
            .execute(
                "INSERT INTO decision_claims (
                    id, project_id, title, rationale, confidence, author_type, status,
                    supersedes_claim_id, superseded_by_claim_id, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, 'user', 'confirmed', ?6, NULL, ?7, ?8)",
                params![
                    &claim.id,
                    &claim.project_id,
                    &claim.title,
                    &claim.rationale,
                    &claim.confidence,
                    &claim.supersedes_claim_id,
                    &claim.created_at,
                    &claim.updated_at,
                ],
            )
            .map_err(storage_error("save local decision"))?;
        for source in &claim.sources {
            transaction
                .execute(
                    "INSERT INTO decision_sources (id, claim_id, project_id, label, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        &source.id,
                        &claim.id,
                        &claim.project_id,
                        &source.label,
                        &source.created_at
                    ],
                )
                .map_err(storage_error("save local decision source"))?;
        }
        if let Some(previous_id) = &supersedes_claim_id {
            let changed = transaction
                .execute(
                    "UPDATE decision_claims
                     SET status = 'superseded', superseded_by_claim_id = ?1, updated_at = ?2
                     WHERE id = ?3 AND project_id = ?4 AND status = 'confirmed'",
                    params![&claim.id, &now, previous_id, &claim.project_id],
                )
                .map_err(storage_error("link corrected local decision"))?;
            if changed != 1 {
                return Err(AuraError::Storage(
                    "Aura could not link the correction to the current local decision safely."
                        .to_string(),
                ));
            }
        }
        let activity_title = if supersedes_claim_id.is_some() {
            "Decision corrected locally"
        } else {
            "Decision saved locally"
        };
        let activity_detail = match superseded_title {
            Some(previous_title) => {
                format!("“{}” now supersedes “{}”.", claim.title, previous_title)
            }
            None => format!(
                "“{}” was recorded as a user decision with explicit local provenance.",
                claim.title
            ),
        };
        transaction
            .execute(
                "INSERT INTO activity_records (id, project_id, kind, title, detail, created_at)
                 VALUES (?1, ?2, 'decision', ?3, ?4, ?5)",
                params![
                    Uuid::new_v4().to_string(),
                    &claim.project_id,
                    activity_title,
                    activity_detail,
                    now
                ],
            )
            .map_err(storage_error("record local decision activity"))?;
        transaction
            .commit()
            .map_err(storage_error("finalize local decision"))?;
        Ok(claim)
    }

    fn find_by_id_for_project(
        &self,
        project_id: &str,
        claim_id: &str,
    ) -> Result<Option<DecisionClaim>, AuraError> {
        let row = self.connection.query_row(
            "SELECT id, project_id, title, rationale, confidence, status, created_at, updated_at,
                    supersedes_claim_id, superseded_by_claim_id
             FROM decision_claims WHERE id = ?1 AND project_id = ?2",
            params![claim_id, project_id],
            claim_row,
        ).optional().map_err(storage_error("read local decision"))?;
        row.map(|claim| self.claim_from_row(claim)).transpose()
    }

    fn claim_from_row(&self, row: ClaimRow) -> Result<DecisionClaim, AuraError> {
        let confidence = ClaimConfidence::from_store(&row.confidence)?;
        let status = ClaimStatus::from_store(&row.status)?;
        let sources = self.sources_for_claim(&row.project_id, &row.id)?;
        Ok(DecisionClaim {
            id: row.id,
            project_id: row.project_id,
            title: row.title,
            rationale: row.rationale,
            confidence: confidence.as_str().to_string(),
            author_type: "user".to_string(),
            status: status.as_str().to_string(),
            created_at: row.created_at,
            updated_at: row.updated_at,
            supersedes_claim_id: row.supersedes_claim_id,
            superseded_by_claim_id: row.superseded_by_claim_id,
            sources,
        })
    }

    fn sources_for_claim(
        &self,
        project_id: &str,
        claim_id: &str,
    ) -> Result<Vec<ClaimSource>, AuraError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, label, created_at FROM decision_sources
             WHERE claim_id = ?1 AND project_id = ?2 ORDER BY created_at ASC, id ASC",
            )
            .map_err(storage_error("read local decision provenance"))?;
        let rows = statement
            .query_map(params![claim_id, project_id], |row| {
                Ok(ClaimSource {
                    id: row.get(0)?,
                    label: row.get(1)?,
                    created_at: row.get(2)?,
                })
            })
            .map_err(storage_error("read local decision provenance"))?;
        rows.map(|row| row.map_err(storage_error("read local decision provenance")))
            .collect()
    }

    fn ensure_active_project(&self, project_id: &str) -> Result<(), AuraError> {
        match self.project_status(project_id)?.as_deref() {
            Some("active") | Some("paused") => Ok(()),
            Some(status) => Err(AuraError::InvalidInput(format!(
                "Aura cannot save a decision to a {status} project."
            ))),
            None => Err(AuraError::NotFound(
                "Aura cannot find this local project for the decision.".to_string(),
            )),
        }
    }

    fn ensure_project_exists(&self, project_id: &str) -> Result<(), AuraError> {
        if self.project_status(project_id)?.is_none() {
            return Err(AuraError::NotFound(
                "Aura cannot find this local project.".to_string(),
            ));
        }
        Ok(())
    }

    fn project_status(&self, project_id: &str) -> Result<Option<String>, AuraError> {
        self.connection
            .query_row(
                "SELECT status FROM projects WHERE id = ?1",
                [project_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(storage_error("validate decision project"))
    }
}

#[derive(Debug)]
struct ClaimRow {
    id: String,
    project_id: String,
    title: String,
    rationale: String,
    confidence: String,
    status: String,
    created_at: String,
    updated_at: String,
    supersedes_claim_id: Option<String>,
    superseded_by_claim_id: Option<String>,
}

fn claim_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ClaimRow> {
    Ok(ClaimRow {
        id: row.get(0)?,
        project_id: row.get(1)?,
        title: row.get(2)?,
        rationale: row.get(3)?,
        confidence: row.get(4)?,
        status: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
        supersedes_claim_id: row.get(8)?,
        superseded_by_claim_id: row.get(9)?,
    })
}

fn required_text(field: &str, value: String, limit: usize) -> Result<String, AuraError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(AuraError::InvalidInput(format!(
            "{field} is required before Aura can save it locally."
        )));
    }
    if value.len() > limit {
        return Err(AuraError::InvalidInput(format!(
            "{field} must be {limit} characters or fewer."
        )));
    }
    Ok(value.to_string())
}

fn source_labels(values: Vec<String>) -> Result<Vec<String>, AuraError> {
    if values.is_empty() {
        return Err(AuraError::InvalidInput(
            "Add at least one source or basis for this decision.".to_string(),
        ));
    }
    if values.len() > 12 {
        return Err(AuraError::InvalidInput(
            "A decision can include at most 12 local source references.".to_string(),
        ));
    }
    values
        .into_iter()
        .map(|value| required_text("Source reference", value, 240))
        .collect()
}

fn storage_error(action: &'static str) -> impl FnOnce(rusqlite::Error) -> AuraError {
    move |error| AuraError::Storage(format!("Aura could not {action}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::CreateDecision;
    use crate::{
        db::{repositories::projects::CreateProject, LocalStore},
        domain::claim::ClaimConfidence,
    };

    #[test]
    fn corrected_decisions_are_immutable_and_scoped_to_their_project() {
        let store = LocalStore::open_in_memory().expect("test store");
        let first = store
            .projects()
            .create(CreateProject {
                name: "Aura".into(),
                goal: None,
                current_task: None,
                next_step: None,
            })
            .expect("first project");
        let second = store
            .projects()
            .create(CreateProject {
                name: "Other".into(),
                goal: None,
                current_task: None,
                next_step: None,
            })
            .expect("second project");
        let original = store
            .decisions()
            .create(CreateDecision {
                project_id: first.id.clone(),
                title: "Use SQLite".into(),
                rationale: "Local-first workspace persistence.".into(),
                confidence: ClaimConfidence::High,
                source_labels: vec!["ADR-003".into()],
            })
            .expect("original");
        let corrected = store
            .decisions()
            .supersede(
                first.id.clone(),
                original.id.clone(),
                "Use bundled SQLite".into(),
                "The application needs a portable local database.".into(),
                ClaimConfidence::High,
                vec![
                    "ADR-003".to_string(),
                    "Windows compatibility review".to_string(),
                ],
            )
            .expect("correction");
        let first_claims = store
            .decisions()
            .list_for_project(&first.id)
            .expect("first claims");
        let second_claims = store
            .decisions()
            .list_for_project(&second.id)
            .expect("second claims");
        assert_eq!(first_claims.len(), 2);
        assert!(second_claims.is_empty());
        assert_eq!(
            corrected.supersedes_claim_id.as_deref(),
            Some(original.id.as_str())
        );
        let superseded = first_claims
            .iter()
            .find(|claim| claim.id == original.id)
            .expect("original in project");
        assert_eq!(superseded.status, "superseded");
        assert_eq!(
            superseded.superseded_by_claim_id.as_deref(),
            Some(corrected.id.as_str())
        );
    }
}
