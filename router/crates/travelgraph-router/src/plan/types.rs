//! Plan node types. Kept in their own module so the executor can pattern-match
//! against them without depending on the planner internals.

use std::collections::HashSet;
use std::time::Duration;

// Public re-exports use these.

/// A complete execution plan for one GraphQL operation. Contains one
/// [`FieldFetch`] per top-level root field selected; each [`FieldFetch`]
/// owns the initial subgraph call plus any `_entities` follow-ups required
/// to satisfy fields owned by extending subgraphs.
#[derive(Debug, Clone)]
pub struct ExecutionPlan {
    /// Kept for plan-level introspection and future telemetry.
    #[allow(dead_code)]
    pub operation_kind: OperationKind,
    pub operation_name: Option<String>,
    pub field_fetches: Vec<FieldFetch>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationKind {
    Query,
    Mutation,
}

/// Plan for a single top-level field selection (e.g.
/// `searchProperties(city: "Austin") { ... }`).
#[derive(Debug, Clone)]
pub struct FieldFetch {
    /// Top-level response key, e.g. "searchProperties".
    pub response_key: String,
    /// Whether the response data under `response_key` is a list (so the
    /// executor knows to iterate each element when collecting entity refs).
    /// Surfaced for plan-debug logs only - the executor inspects the actual
    /// JSON shape at runtime.
    #[allow(dead_code)]
    pub is_list: bool,
    /// Entity type name returned by this field, if it is a federated entity
    /// type (otherwise `None` and entity fetches will be empty).
    pub entity_type: Option<String>,
    /// Initial subgraph call.
    pub initial: InitialFetch,
    /// Per-extending-subgraph follow-up `_entities` calls. One per subgraph,
    /// regardless of how many entities the initial call returns - that's the
    /// "1 batched call per subgraph" guarantee.
    pub entity_fetches: Vec<EntityFetch>,
    /// Subset of the entity's `@key` fields that the client explicitly
    /// requested. Used by the merger to decide whether to strip
    /// planner-injected key fields (e.g. `id`) from the final response.
    pub client_requested_id_keys: HashSet<String>,
}

#[derive(Debug, Clone)]
pub struct InitialFetch {
    /// Subgraph name (e.g. "property") and its route (URL + timeout).
    pub subgraph: String,
    pub url: String,
    pub timeout: Duration,
    /// Self-contained GraphQL operation text targeted at this subgraph.
    pub query_text: String,
    /// Variables referenced by `query_text` (subset of the inbound request).
    pub variable_names: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct EntityFetch {
    pub subgraph: String,
    pub url: String,
    pub timeout: Duration,
    pub type_name: String,
    pub key_field: String,
    /// Inline-fragment body (between `... on Type {` and `}`).
    pub fragment_body: String,
    /// Where in the initial-fetch response to find entities. Reserved for
    /// future planners that batch nested entities; the current executor
    /// always walks from `FieldFetch::response_key`.
    #[allow(dead_code)]
    pub selection_path: SelectionPath,
}

/// Path describing how to traverse an initial-fetch response to reach the
/// entity (or list of entities) the executor should batch into an
/// `_entities` call. Always begins with the response key of the field; the
/// executor handles list-vs-scalar shape internally.
#[derive(Debug, Clone)]
pub struct SelectionPath {
    #[allow(dead_code)]
    pub root_key: String,
    #[allow(dead_code)]
    pub is_list: bool,
}
