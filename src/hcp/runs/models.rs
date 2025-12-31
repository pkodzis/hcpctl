//! Run data models

use serde::Deserialize;

use crate::hcp::traits::TfeResource;
use crate::hcp::workspaces::RelationshipData;

/// Individual run statuses for explicit filtering
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunStatus {
    Pending,
    Fetching,
    FetchingCompleted,
    PrePlanRunning,
    PrePlanCompleted,
    Queuing,
    PlanQueued,
    Planning,
    Planned,
    CostEstimating,
    CostEstimated,
    PolicyChecking,
    PolicyOverride,
    PolicySoftFailed,
    PolicyChecked,
    Confirmed,
    PostPlanRunning,
    PostPlanCompleted,
    PlannedAndFinished,
    PlannedAndSaved,
    ApplyQueued,
    Applying,
    Applied,
    Discarded,
    Errored,
    Canceled,
    ForceCanceled,
}

impl std::fmt::Display for RunStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            RunStatus::Pending => "pending",
            RunStatus::Fetching => "fetching",
            RunStatus::FetchingCompleted => "fetching_completed",
            RunStatus::PrePlanRunning => "pre_plan_running",
            RunStatus::PrePlanCompleted => "pre_plan_completed",
            RunStatus::Queuing => "queuing",
            RunStatus::PlanQueued => "plan_queued",
            RunStatus::Planning => "planning",
            RunStatus::Planned => "planned",
            RunStatus::CostEstimating => "cost_estimating",
            RunStatus::CostEstimated => "cost_estimated",
            RunStatus::PolicyChecking => "policy_checking",
            RunStatus::PolicyOverride => "policy_override",
            RunStatus::PolicySoftFailed => "policy_soft_failed",
            RunStatus::PolicyChecked => "policy_checked",
            RunStatus::Confirmed => "confirmed",
            RunStatus::PostPlanRunning => "post_plan_running",
            RunStatus::PostPlanCompleted => "post_plan_completed",
            RunStatus::PlannedAndFinished => "planned_and_finished",
            RunStatus::PlannedAndSaved => "planned_and_saved",
            RunStatus::ApplyQueued => "apply_queued",
            RunStatus::Applying => "applying",
            RunStatus::Applied => "applied",
            RunStatus::Discarded => "discarded",
            RunStatus::Errored => "errored",
            RunStatus::Canceled => "canceled",
            RunStatus::ForceCanceled => "force_canceled",
        };
        write!(f, "{}", s)
    }
}

impl std::str::FromStr for RunStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(RunStatus::Pending),
            "fetching" => Ok(RunStatus::Fetching),
            "fetching_completed" => Ok(RunStatus::FetchingCompleted),
            "pre_plan_running" => Ok(RunStatus::PrePlanRunning),
            "pre_plan_completed" => Ok(RunStatus::PrePlanCompleted),
            "queuing" => Ok(RunStatus::Queuing),
            "plan_queued" => Ok(RunStatus::PlanQueued),
            "planning" => Ok(RunStatus::Planning),
            "planned" => Ok(RunStatus::Planned),
            "cost_estimating" => Ok(RunStatus::CostEstimating),
            "cost_estimated" => Ok(RunStatus::CostEstimated),
            "policy_checking" => Ok(RunStatus::PolicyChecking),
            "policy_override" => Ok(RunStatus::PolicyOverride),
            "policy_soft_failed" => Ok(RunStatus::PolicySoftFailed),
            "policy_checked" => Ok(RunStatus::PolicyChecked),
            "confirmed" => Ok(RunStatus::Confirmed),
            "post_plan_running" => Ok(RunStatus::PostPlanRunning),
            "post_plan_completed" => Ok(RunStatus::PostPlanCompleted),
            "planned_and_finished" => Ok(RunStatus::PlannedAndFinished),
            "planned_and_saved" => Ok(RunStatus::PlannedAndSaved),
            "apply_queued" => Ok(RunStatus::ApplyQueued),
            "applying" => Ok(RunStatus::Applying),
            "applied" => Ok(RunStatus::Applied),
            "discarded" => Ok(RunStatus::Discarded),
            "errored" => Ok(RunStatus::Errored),
            "canceled" => Ok(RunStatus::Canceled),
            "force_canceled" => Ok(RunStatus::ForceCanceled),
            _ => Err(format!("Unknown run status: {}", s)),
        }
    }
}

impl RunStatus {
    /// Check if this is a non-final (active) status
    ///
    /// Non-final statuses are runs that are still in progress and not yet completed.
    /// Final statuses are: applied, discarded, errored, canceled, force_canceled,
    /// planned_and_finished, planned_and_saved
    pub fn is_non_final(&self) -> bool {
        !matches!(
            self,
            RunStatus::Applied
                | RunStatus::Discarded
                | RunStatus::Errored
                | RunStatus::Canceled
                | RunStatus::ForceCanceled
                | RunStatus::PlannedAndFinished
                | RunStatus::PlannedAndSaved
        )
    }
}

/// Query options for listing runs
#[derive(Default, Clone)]
pub struct RunQuery {
    /// Filter by status group: "non_final", "final", "discardable"
    pub status_group: Option<String>,
    /// Filter by specific statuses (comma-separated in API)
    pub statuses: Option<Vec<RunStatus>>,
    /// Filter by workspace names (org endpoint only)
    pub workspace_names: Option<Vec<String>>,
    /// Page number for pagination
    pub page: Option<u32>,
    /// Page size for pagination
    pub page_size: Option<u32>,
}

impl RunQuery {
    /// Create a new query with default non_final status group
    pub fn non_final() -> Self {
        Self {
            status_group: Some("non_final".to_string()),
            ..Default::default()
        }
    }

    /// Create a query with specific statuses
    pub fn with_statuses(statuses: Vec<RunStatus>) -> Self {
        Self {
            statuses: Some(statuses),
            ..Default::default()
        }
    }
}

/// Response wrapper for runs list
#[derive(Deserialize, Debug)]
pub struct RunsResponse {
    pub data: Vec<Run>,
    #[serde(default)]
    pub meta: Option<RunPaginationMeta>,
}

/// Pagination metadata for runs (org endpoint doesn't have total-count)
#[derive(Deserialize, Debug, Default)]
pub struct RunPaginationMeta {
    pub pagination: Option<RunPagination>,
}

/// Pagination details for runs
#[derive(Deserialize, Debug)]
pub struct RunPagination {
    #[serde(rename = "current-page")]
    pub current_page: u32,
    #[serde(rename = "next-page")]
    pub next_page: Option<u32>,
    #[serde(rename = "prev-page")]
    pub prev_page: Option<u32>,
    #[serde(rename = "page-size")]
    pub page_size: u32,
    /// Only available for workspace endpoint, not org endpoint
    #[serde(rename = "total-count")]
    pub total_count: Option<u32>,
    /// Only available for workspace endpoint, not org endpoint
    #[serde(rename = "total-pages")]
    pub total_pages: Option<u32>,
}

/// Run data from TFE API
#[derive(Deserialize, Debug, Clone)]
pub struct Run {
    pub id: String,
    pub attributes: RunAttributes,
    pub relationships: Option<RunRelationships>,
}

/// Run attributes from TFE API
#[derive(Deserialize, Debug, Clone)]
pub struct RunAttributes {
    pub status: String,
    pub message: Option<String>,
    pub source: Option<String>,
    #[serde(rename = "created-at")]
    pub created_at: Option<String>,
    #[serde(rename = "has-changes")]
    pub has_changes: Option<bool>,
    #[serde(rename = "is-destroy")]
    pub is_destroy: Option<bool>,
    #[serde(rename = "plan-only")]
    pub plan_only: Option<bool>,
    #[serde(rename = "auto-apply")]
    pub auto_apply: Option<bool>,
    #[serde(rename = "trigger-reason")]
    pub trigger_reason: Option<String>,
    pub actions: Option<RunActions>,
}

/// Run action flags
#[derive(Deserialize, Debug, Clone)]
pub struct RunActions {
    #[serde(rename = "is-cancelable")]
    pub is_cancelable: Option<bool>,
    #[serde(rename = "is-confirmable")]
    pub is_confirmable: Option<bool>,
    #[serde(rename = "is-discardable")]
    pub is_discardable: Option<bool>,
    #[serde(rename = "is-force-cancelable")]
    pub is_force_cancelable: Option<bool>,
}

/// Run relationships from TFE API
#[derive(Deserialize, Debug, Clone)]
pub struct RunRelationships {
    pub workspace: Option<RelationshipData>,
    #[serde(rename = "configuration-version")]
    pub configuration_version: Option<RelationshipData>,
    #[serde(rename = "created-by")]
    pub created_by: Option<RelationshipData>,
    pub plan: Option<RelationshipData>,
    pub apply: Option<RelationshipData>,
}

/// Run event from TFE API
#[derive(Deserialize, Debug, Clone)]
pub struct RunEvent {
    pub id: String,
    pub attributes: RunEventAttributes,
    pub relationships: Option<RunEventRelationships>,
}

/// Run event attributes
#[derive(Deserialize, Debug, Clone)]
pub struct RunEventAttributes {
    pub action: String,
    #[serde(rename = "created-at")]
    pub created_at: Option<String>,
    pub description: Option<String>,
}

/// Run event relationships
#[derive(Deserialize, Debug, Clone)]
pub struct RunEventRelationships {
    pub target: Option<RelationshipData>,
}

impl RunEvent {
    /// Get the action
    pub fn action(&self) -> &str {
        &self.attributes.action
    }

    /// Get created_at timestamp
    pub fn created_at(&self) -> &str {
        self.attributes.created_at.as_deref().unwrap_or("")
    }

    /// Get target ID
    pub fn target_id(&self) -> &str {
        self.relationships
            .as_ref()
            .and_then(|r| r.target.as_ref())
            .and_then(|t| t.data.as_ref())
            .map(|d| d.id.as_str())
            .unwrap_or("")
    }

    /// Get target type
    pub fn target_type(&self) -> &str {
        self.relationships
            .as_ref()
            .and_then(|r| r.target.as_ref())
            .and_then(|t| t.data.as_ref())
            .and_then(|d| d.rel_type.as_deref())
            .unwrap_or("")
    }
}

/// Response wrapper for run events
#[derive(Deserialize, Debug)]
pub struct RunEventsResponse {
    pub data: Vec<RunEvent>,
}

/// Plan data from TFE API (GET /runs/:id/plan)
#[derive(Deserialize, Debug, Clone)]
pub struct Plan {
    pub id: String,
    pub attributes: PlanAttributes,
}

/// Plan attributes from TFE API
#[derive(Deserialize, Debug, Clone)]
pub struct PlanAttributes {
    pub status: String,
    #[serde(rename = "has-changes")]
    pub has_changes: Option<bool>,
    #[serde(rename = "resource-additions")]
    pub resource_additions: Option<i32>,
    #[serde(rename = "resource-changes")]
    pub resource_changes: Option<i32>,
    #[serde(rename = "resource-destructions")]
    pub resource_destructions: Option<i32>,
    #[serde(rename = "resource-imports")]
    pub resource_imports: Option<i32>,
    #[serde(rename = "log-read-url")]
    pub log_read_url: Option<String>,
    #[serde(rename = "status-timestamps")]
    pub status_timestamps: Option<serde_json::Value>,
}

impl Plan {
    /// Get plan status
    pub fn status(&self) -> &str {
        &self.attributes.status
    }

    /// Check if plan has changes
    pub fn has_changes(&self) -> bool {
        self.attributes.has_changes.unwrap_or(false)
    }

    /// Get resource additions count
    pub fn resource_additions(&self) -> i32 {
        self.attributes.resource_additions.unwrap_or(0)
    }

    /// Get resource changes count
    pub fn resource_changes(&self) -> i32 {
        self.attributes.resource_changes.unwrap_or(0)
    }

    /// Get resource destructions count
    pub fn resource_destructions(&self) -> i32 {
        self.attributes.resource_destructions.unwrap_or(0)
    }

    /// Get resource imports count
    pub fn resource_imports(&self) -> i32 {
        self.attributes.resource_imports.unwrap_or(0)
    }

    /// Get log read URL (temporary, expires in 1 minute)
    pub fn log_read_url(&self) -> Option<&str> {
        self.attributes.log_read_url.as_deref()
    }

    /// Check if plan is in a final state
    pub fn is_final(&self) -> bool {
        matches!(
            self.status(),
            "finished" | "errored" | "canceled" | "unreachable"
        )
    }
}

/// Response wrapper for plan
#[derive(Deserialize, Debug)]
pub struct PlanResponse {
    pub data: Plan,
}

/// Apply data from TFE API (GET /runs/:id/apply)
#[derive(Deserialize, Debug, Clone)]
pub struct Apply {
    pub id: String,
    pub attributes: ApplyAttributes,
}

/// Apply attributes from TFE API
#[derive(Deserialize, Debug, Clone)]
pub struct ApplyAttributes {
    pub status: String,
    #[serde(rename = "resource-additions")]
    pub resource_additions: Option<i32>,
    #[serde(rename = "resource-changes")]
    pub resource_changes: Option<i32>,
    #[serde(rename = "resource-destructions")]
    pub resource_destructions: Option<i32>,
    #[serde(rename = "resource-imports")]
    pub resource_imports: Option<i32>,
    #[serde(rename = "log-read-url")]
    pub log_read_url: Option<String>,
    #[serde(rename = "status-timestamps")]
    pub status_timestamps: Option<serde_json::Value>,
}

impl Apply {
    /// Get apply status
    pub fn status(&self) -> &str {
        &self.attributes.status
    }

    /// Get resource additions count
    pub fn resource_additions(&self) -> i32 {
        self.attributes.resource_additions.unwrap_or(0)
    }

    /// Get resource changes count
    pub fn resource_changes(&self) -> i32 {
        self.attributes.resource_changes.unwrap_or(0)
    }

    /// Get resource destructions count
    pub fn resource_destructions(&self) -> i32 {
        self.attributes.resource_destructions.unwrap_or(0)
    }

    /// Get resource imports count
    pub fn resource_imports(&self) -> i32 {
        self.attributes.resource_imports.unwrap_or(0)
    }

    /// Get log read URL (temporary, expires in 1 minute)
    pub fn log_read_url(&self) -> Option<&str> {
        self.attributes.log_read_url.as_deref()
    }

    /// Check if apply is in a final state
    pub fn is_final(&self) -> bool {
        matches!(
            self.status(),
            "finished" | "errored" | "canceled" | "unreachable"
        )
    }
}

/// Response wrapper for apply
#[derive(Deserialize, Debug)]
pub struct ApplyResponse {
    pub data: Apply,
}

impl TfeResource for Run {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        // Runs don't have names, use ID
        &self.id
    }
}

impl Run {
    /// Get the status of the run
    pub fn status(&self) -> &str {
        &self.attributes.status
    }

    /// Get the message, defaulting to empty string if not available
    pub fn message(&self) -> &str {
        self.attributes.message.as_deref().unwrap_or("")
    }

    /// Get the source, defaulting to "unknown" if not available
    pub fn source(&self) -> &str {
        self.attributes.source.as_deref().unwrap_or("unknown")
    }

    /// Get created_at timestamp, defaulting to empty string if not available
    pub fn created_at(&self) -> &str {
        self.attributes.created_at.as_deref().unwrap_or("")
    }

    /// Check if run has changes
    pub fn has_changes(&self) -> bool {
        self.attributes.has_changes.unwrap_or(false)
    }

    /// Check if this is a destroy run
    pub fn is_destroy(&self) -> bool {
        self.attributes.is_destroy.unwrap_or(false)
    }

    /// Check if this is a plan-only run
    pub fn is_plan_only(&self) -> bool {
        self.attributes.plan_only.unwrap_or(false)
    }

    /// Get trigger reason, defaulting to "unknown" if not available
    pub fn trigger_reason(&self) -> &str {
        self.attributes
            .trigger_reason
            .as_deref()
            .unwrap_or("unknown")
    }

    /// Get workspace ID from relationships
    pub fn workspace_id(&self) -> Option<&str> {
        self.relationships
            .as_ref()
            .and_then(|r| r.workspace.as_ref())
            .and_then(|w| w.data.as_ref())
            .map(|d| d.id.as_str())
    }

    /// Check if run is cancelable
    pub fn is_cancelable(&self) -> bool {
        self.attributes
            .actions
            .as_ref()
            .and_then(|a| a.is_cancelable)
            .unwrap_or(false)
    }

    /// Check if run is confirmable
    pub fn is_confirmable(&self) -> bool {
        self.attributes
            .actions
            .as_ref()
            .and_then(|a| a.is_confirmable)
            .unwrap_or(false)
    }

    /// Check if run is discardable
    pub fn is_discardable(&self) -> bool {
        self.attributes
            .actions
            .as_ref()
            .and_then(|a| a.is_discardable)
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_status_display() {
        assert_eq!(RunStatus::Pending.to_string(), "pending");
        assert_eq!(RunStatus::Planning.to_string(), "planning");
        assert_eq!(RunStatus::Applied.to_string(), "applied");
        assert_eq!(
            RunStatus::PlannedAndFinished.to_string(),
            "planned_and_finished"
        );
    }

    #[test]
    fn test_run_status_from_str() {
        assert_eq!("pending".parse::<RunStatus>().unwrap(), RunStatus::Pending);
        assert_eq!(
            "PLANNING".parse::<RunStatus>().unwrap(),
            RunStatus::Planning
        );
        assert_eq!("Applied".parse::<RunStatus>().unwrap(), RunStatus::Applied);
        assert!("invalid".parse::<RunStatus>().is_err());
    }

    #[test]
    fn test_run_query_non_final() {
        let query = RunQuery::non_final();
        assert_eq!(query.status_group, Some("non_final".to_string()));
        assert!(query.statuses.is_none());
    }

    #[test]
    fn test_run_query_with_statuses() {
        let query = RunQuery::with_statuses(vec![RunStatus::Planning, RunStatus::Applying]);
        assert!(query.status_group.is_none());
        assert_eq!(query.statuses.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn test_run_status_is_non_final() {
        // Non-final statuses
        assert!(RunStatus::Pending.is_non_final());
        assert!(RunStatus::Planning.is_non_final());
        assert!(RunStatus::Applying.is_non_final());
        assert!(RunStatus::Planned.is_non_final());
        assert!(RunStatus::Confirmed.is_non_final());
        assert!(RunStatus::ApplyQueued.is_non_final());

        // Final statuses
        assert!(!RunStatus::Applied.is_non_final());
        assert!(!RunStatus::Discarded.is_non_final());
        assert!(!RunStatus::Errored.is_non_final());
        assert!(!RunStatus::Canceled.is_non_final());
        assert!(!RunStatus::ForceCanceled.is_non_final());
        assert!(!RunStatus::PlannedAndFinished.is_non_final());
        assert!(!RunStatus::PlannedAndSaved.is_non_final());
    }

    #[test]
    fn test_run_event_deserialization() {
        let event: RunEvent = serde_json::from_value(serde_json::json!({
            "id": "re-abc123",
            "type": "run-events",
            "attributes": {
                "action": "queued",
                "created-at": "2025-01-01T10:00:00.000Z",
                "description": null
            },
            "relationships": {
                "target": {
                    "data": {
                        "id": "plan-xyz789",
                        "type": "plans"
                    }
                }
            }
        }))
        .unwrap();

        assert_eq!(event.id, "re-abc123");
        assert_eq!(event.action(), "queued");
        assert_eq!(event.created_at(), "2025-01-01T10:00:00.000Z");
        assert_eq!(event.target_id(), "plan-xyz789");
        assert_eq!(event.target_type(), "plans");
    }

    #[test]
    fn test_run_event_no_target() {
        let event: RunEvent = serde_json::from_value(serde_json::json!({
            "id": "re-abc123",
            "type": "run-events",
            "attributes": {
                "action": "confirmed",
                "created-at": "2025-01-01T10:00:00.000Z"
            },
            "relationships": {
                "target": {
                    "data": null
                }
            }
        }))
        .unwrap();

        assert_eq!(event.target_id(), "");
        assert_eq!(event.target_type(), "");
    }

    #[test]
    fn test_run_events_response_deserialization() {
        let response: RunEventsResponse = serde_json::from_value(serde_json::json!({
            "data": [
                {
                    "id": "re-1",
                    "type": "run-events",
                    "attributes": {"action": "created", "created-at": "2025-01-01T10:00:00.000Z"}
                },
                {
                    "id": "re-2",
                    "type": "run-events",
                    "attributes": {"action": "queued", "created-at": "2025-01-01T10:01:00.000Z"}
                }
            ]
        }))
        .unwrap();

        assert_eq!(response.data.len(), 2);
        assert_eq!(response.data[0].action(), "created");
        assert_eq!(response.data[1].action(), "queued");
    }

    #[test]
    fn test_plan_deserialization() {
        let plan: Plan = serde_json::from_value(serde_json::json!({
            "id": "plan-abc123",
            "type": "plans",
            "attributes": {
                "status": "finished",
                "has-changes": true,
                "resource-additions": 5,
                "resource-changes": 2,
                "resource-destructions": 1,
                "resource-imports": 0,
                "log-read-url": "https://archivist.terraform.io/v1/object/abc123"
            }
        }))
        .unwrap();

        assert_eq!(plan.id, "plan-abc123");
        assert_eq!(plan.status(), "finished");
        assert!(plan.has_changes());
        assert_eq!(plan.resource_additions(), 5);
        assert_eq!(plan.resource_changes(), 2);
        assert_eq!(plan.resource_destructions(), 1);
        assert_eq!(plan.resource_imports(), 0);
        assert_eq!(
            plan.log_read_url(),
            Some("https://archivist.terraform.io/v1/object/abc123")
        );
    }

    #[test]
    fn test_plan_is_final() {
        let finished: Plan = serde_json::from_value(serde_json::json!({
            "id": "plan-1",
            "type": "plans",
            "attributes": {"status": "finished"}
        }))
        .unwrap();
        assert!(finished.is_final());

        let errored: Plan = serde_json::from_value(serde_json::json!({
            "id": "plan-2",
            "type": "plans",
            "attributes": {"status": "errored"}
        }))
        .unwrap();
        assert!(errored.is_final());

        let running: Plan = serde_json::from_value(serde_json::json!({
            "id": "plan-3",
            "type": "plans",
            "attributes": {"status": "running"}
        }))
        .unwrap();
        assert!(!running.is_final());

        let pending: Plan = serde_json::from_value(serde_json::json!({
            "id": "plan-4",
            "type": "plans",
            "attributes": {"status": "pending"}
        }))
        .unwrap();
        assert!(!pending.is_final());
    }

    #[test]
    fn test_plan_defaults() {
        let plan: Plan = serde_json::from_value(serde_json::json!({
            "id": "plan-minimal",
            "type": "plans",
            "attributes": {"status": "pending"}
        }))
        .unwrap();

        assert!(!plan.has_changes());
        assert_eq!(plan.resource_additions(), 0);
        assert_eq!(plan.resource_changes(), 0);
        assert_eq!(plan.resource_destructions(), 0);
        assert_eq!(plan.resource_imports(), 0);
        assert!(plan.log_read_url().is_none());
    }

    #[test]
    fn test_plan_response_deserialization() {
        let response: PlanResponse = serde_json::from_value(serde_json::json!({
            "data": {
                "id": "plan-abc123",
                "type": "plans",
                "attributes": {
                    "status": "finished",
                    "has-changes": true
                }
            }
        }))
        .unwrap();

        assert_eq!(response.data.id, "plan-abc123");
        assert_eq!(response.data.status(), "finished");
    }

    #[test]
    fn test_apply_deserialization() {
        let apply: Apply = serde_json::from_value(serde_json::json!({
            "id": "apply-xyz789",
            "type": "applies",
            "attributes": {
                "status": "finished",
                "resource-additions": 3,
                "resource-changes": 1,
                "resource-destructions": 0,
                "resource-imports": 2,
                "log-read-url": "https://archivist.terraform.io/v1/object/xyz789"
            }
        }))
        .unwrap();

        assert_eq!(apply.id, "apply-xyz789");
        assert_eq!(apply.status(), "finished");
        assert_eq!(apply.resource_additions(), 3);
        assert_eq!(apply.resource_changes(), 1);
        assert_eq!(apply.resource_destructions(), 0);
        assert_eq!(apply.resource_imports(), 2);
        assert_eq!(
            apply.log_read_url(),
            Some("https://archivist.terraform.io/v1/object/xyz789")
        );
    }

    #[test]
    fn test_apply_is_final() {
        let finished: Apply = serde_json::from_value(serde_json::json!({
            "id": "apply-1",
            "type": "applies",
            "attributes": {"status": "finished"}
        }))
        .unwrap();
        assert!(finished.is_final());

        let errored: Apply = serde_json::from_value(serde_json::json!({
            "id": "apply-2",
            "type": "applies",
            "attributes": {"status": "errored"}
        }))
        .unwrap();
        assert!(errored.is_final());

        let canceled: Apply = serde_json::from_value(serde_json::json!({
            "id": "apply-3",
            "type": "applies",
            "attributes": {"status": "canceled"}
        }))
        .unwrap();
        assert!(canceled.is_final());

        let running: Apply = serde_json::from_value(serde_json::json!({
            "id": "apply-4",
            "type": "applies",
            "attributes": {"status": "running"}
        }))
        .unwrap();
        assert!(!running.is_final());
    }

    #[test]
    fn test_apply_defaults() {
        let apply: Apply = serde_json::from_value(serde_json::json!({
            "id": "apply-minimal",
            "type": "applies",
            "attributes": {"status": "pending"}
        }))
        .unwrap();

        assert_eq!(apply.resource_additions(), 0);
        assert_eq!(apply.resource_changes(), 0);
        assert_eq!(apply.resource_destructions(), 0);
        assert_eq!(apply.resource_imports(), 0);
        assert!(apply.log_read_url().is_none());
    }

    #[test]
    fn test_apply_response_deserialization() {
        let response: ApplyResponse = serde_json::from_value(serde_json::json!({
            "data": {
                "id": "apply-xyz789",
                "type": "applies",
                "attributes": {
                    "status": "finished"
                }
            }
        }))
        .unwrap();

        assert_eq!(response.data.id, "apply-xyz789");
        assert_eq!(response.data.status(), "finished");
    }
}
