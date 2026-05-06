//! GraphQL request/response shapes plus parser and validator.
//!
//! Schema-aware planning lives in [`crate::plan`] (Phase 3); the modules here
//! deal only with the inbound document text:
//!
//!   * [`types`]    - `{query, variables, operationName}` request and the
//!                    spec-shaped `{data, errors}` response envelope.
//!   * [`parse`]    - `apollo-compiler` AST parsing with location-aware
//!                    error reporting.
//!   * [`validate`] - schema-independent validation (no anonymous mixed
//!                    operations, declared variables only, etc.).

pub mod parse;
pub mod types;
pub mod validate;
