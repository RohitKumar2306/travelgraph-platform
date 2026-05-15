//! Query planning.
//!
//! Replaces Phase 2.3's hand-coded `SubgraphRegistry` projection with a
//! supergraph-aware planner. Given a parsed GraphQL document and a
//! [`SupergraphCatalog`], `plan_operation` produces an [`ExecutionPlan`]:
//!
//!   * one `InitialFetch` per top-level field, scoped to the owning subgraph
//!   * zero or more `EntityFetch` nodes per top-level field, one per
//!     extending subgraph that contributes selected fields on the entity
//!     returned by the initial fetch
//!
//! The plan is execution-order aware: the executor (Phase 3.4) runs each
//! `InitialFetch` in parallel, then for each one fans out its
//! `EntityFetch`es (also in parallel) once the initial response is in.

mod planner;
mod types;

#[allow(unused_imports)]
pub use planner::{plan_operation, PlanError};
#[allow(unused_imports)]
pub use types::{
    EntityFetch, ExecutionPlan, FieldFetch, InitialFetch, OperationKind, SelectionPath,
};
