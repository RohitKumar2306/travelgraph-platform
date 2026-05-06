//! GraphQL request/response shapes plus parser and validator.
//!
//! The router is intentionally schemaless in Phase 2: we don't have a
//! supergraph yet (that lands in Phase 3). We use [`apollo_compiler::ast`] to
//! parse and run "standalone" validation that can be done without a schema.

pub mod parse;
pub mod project;
pub mod types;
pub mod validate;
