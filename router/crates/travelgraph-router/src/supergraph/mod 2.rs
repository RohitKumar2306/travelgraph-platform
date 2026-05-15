//! Supergraph parsing and catalog.
//!
//! In Phase 3 the router stops trusting a hand-coded field-to-subgraph map
//! (Phase 2.3's `SubgraphRegistry`) and instead reads the composed supergraph
//! at startup. The supergraph SDL is produced by `schema-registry/composer`
//! using `@apollo/composition`.
//!
//! `SupergraphCatalog` is the in-memory representation the planner walks:
//!
//!   * `subgraphs` - name -> URL (sourced from `enum join__Graph`)
//!   * `root_query_fields` / `root_mutation_fields` - top-level field name ->
//!     owning subgraph name
//!   * `entity_types` - typename -> entity description
//!     (key fields, owner subgraph, extending subgraphs and the fields each
//!     of them contributes)
//!
//! The catalog is intentionally schema-shaped, not query-shaped: the planner
//! turns each incoming query into an [`ExecutionPlan`] using this catalog.

mod catalog;

#[cfg(test)]
pub use catalog::parse;
pub use catalog::{load_from_file, SupergraphCatalog};
#[allow(unused_imports)]
pub use catalog::{EntityExtender, EntityType, SupergraphError, SupergraphRoute};
