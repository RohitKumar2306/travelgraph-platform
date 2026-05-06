//! Parse a composed Apollo Federation v2 supergraph SDL into a
//! [`SupergraphCatalog`] the planner can consume.
//!
//! We use [`apollo_compiler::Schema::parse`] (no validation - the supergraph
//! is already validated by the composer) and walk:
//!
//!   * `enum join__Graph` for the subgraph name -> URL mapping (each value
//!     carries `@join__graph(name, url)`).
//!   * Every `OBJECT` type for `@join__type(graph, key, extension)` directives
//!     plus `@join__field(graph)` directives on its fields.
//!   * `Query` / `Mutation` root types specifically, so the planner can route
//!     top-level fields immediately.
//!
//! We deliberately do NOT support interfaces, unions, abstract `@key`s,
//! `@requires` / `@provides`, `@override`, or interface-object federation in
//! this Phase 3 cut. Those land in later phases; the parser surfaces an
//! ergonomic error if we encounter one we don't handle yet.

use apollo_compiler::ast::{Definition, Document, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct SupergraphCatalog {
    /// Subgraph name -> route (URL + timeout).
    pub subgraphs: HashMap<String, SupergraphRoute>,
    /// Query field name -> owning subgraph name.
    pub root_query_fields: HashMap<String, String>,
    /// Mutation field name -> owning subgraph name.
    pub root_mutation_fields: HashMap<String, String>,
    /// Entity-shaped object types, keyed by GraphQL type name.
    pub entity_types: HashMap<String, EntityType>,
}

#[derive(Debug, Clone)]
pub struct SupergraphRoute {
    pub name: String,
    pub url: String,
    pub timeout: Duration,
}

#[derive(Debug, Clone)]
pub struct EntityType {
    /// Type name (e.g. `Property`). Surfaced for logs and future telemetry.
    #[allow(dead_code)]
    pub name: String,
    /// Field names that compose this entity's `@key` (e.g. `["id"]`).
    pub key_fields: Vec<String>,
    /// Subgraph that owns the type (the one without `extension: true`).
    /// Surfaced for plan logging.
    #[allow(dead_code)]
    pub owner: String,
    /// Subgraphs that extend the type, plus the fields each contributes.
    /// Currently the planner derives extenders from `field_owners` directly;
    /// this list is retained for diagnostics.
    #[allow(dead_code)]
    pub extenders: Vec<EntityExtender>,
    /// Field name -> owning subgraph name. Includes fields owned by the
    /// owner subgraph itself.
    pub field_owners: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct EntityExtender {
    pub subgraph: String,
    pub fields: Vec<String>,
}

#[derive(Debug, Error)]
pub enum SupergraphError {
    #[error("could not read supergraph file at {path:?}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("supergraph SDL parse error: {0}")]
    Parse(String),
    #[error("supergraph validation error: {0}")]
    Validation(String),
}

/// Default per-subgraph timeout when the supergraph doesn't specify one.
/// Mirrors Phase 2.4's policy.
const DEFAULT_SUBGRAPH_TIMEOUT_MS: u64 = 1000;

pub fn load_from_file(path: impl AsRef<Path>) -> Result<SupergraphCatalog, SupergraphError> {
    let path = path.as_ref().to_path_buf();
    let sdl = std::fs::read_to_string(&path).map_err(|source| SupergraphError::Read {
        path: path.clone(),
        source,
    })?;
    parse(&sdl)
}

pub fn parse(sdl: &str) -> Result<SupergraphCatalog, SupergraphError> {
    let document = Document::parse(sdl, "supergraph.graphql")
        .map_err(|e| SupergraphError::Parse(e.errors.to_string()))?;

    let subgraphs = collect_subgraphs(&document)?;
    let entity_types = collect_entity_types(&document, &subgraphs)?;
    let (root_query_fields, root_mutation_fields) = collect_root_fields(&document)?;

    Ok(SupergraphCatalog {
        subgraphs,
        root_query_fields,
        root_mutation_fields,
        entity_types,
    })
}

// ---- subgraph enum --------------------------------------------------------

/// Read `enum join__Graph { NAME @join__graph(name, url) ... }` and turn it
/// into a `name -> SupergraphRoute` map.
fn collect_subgraphs(document: &Document) -> Result<HashMap<String, SupergraphRoute>, SupergraphError> {
    let mut out = HashMap::new();
    for def in &document.definitions {
        let Definition::EnumTypeDefinition(enum_def) = def else { continue };
        if enum_def.name.as_str() != "join__Graph" {
            continue;
        }
        for value in &enum_def.values {
            for directive in &value.directives {
                if directive.name.as_str() != "join__graph" {
                    continue;
                }
                let mut name: Option<String> = None;
                let mut url: Option<String> = None;
                for arg in &directive.arguments {
                    match arg.name.as_str() {
                        "name" => {
                            if let Value::String(s) = &*arg.value {
                                name = Some(s.clone());
                            }
                        }
                        "url" => {
                            if let Value::String(s) = &*arg.value {
                                url = Some(s.clone());
                            }
                        }
                        _ => {}
                    }
                }
                if let (Some(name), Some(url)) = (name, url) {
                    out.insert(
                        name.clone(),
                        SupergraphRoute {
                            name,
                            url,
                            timeout: Duration::from_millis(DEFAULT_SUBGRAPH_TIMEOUT_MS),
                        },
                    );
                }
            }
        }
    }
    if out.is_empty() {
        return Err(SupergraphError::Validation(
            "supergraph has no `enum join__Graph` declaration; was the SDL composed by Apollo?".into(),
        ));
    }
    Ok(out)
}

// ---- entity-shaped object types ------------------------------------------

/// Look at every `OBJECT` type declaration and pull out its `@join__type` and
/// `@join__field` directives. The resulting [`EntityType`]s describe which
/// subgraph owns the type and which extend it.
///
/// Types that aren't tagged with `@join__type` (e.g. plain value types defined
/// in a single subgraph, or root operation types) are skipped.
fn collect_entity_types(
    document: &Document,
    subgraphs: &HashMap<String, SupergraphRoute>,
) -> Result<HashMap<String, EntityType>, SupergraphError> {
    let enum_lookup = build_enum_lookup(document);

    let mut out = HashMap::new();
    for def in &document.definitions {
        let Definition::ObjectTypeDefinition(obj) = def else { continue };
        let name = obj.name.as_str();
        if matches!(name, "Query" | "Mutation" | "Subscription") {
            continue;
        }

        let mut owner: Option<String> = None;
        let mut extenders: Vec<EntityExtender> = Vec::new();
        let mut key_fields: Vec<String> = Vec::new();

        for directive in &obj.directives {
            if directive.name.as_str() != "join__type" {
                continue;
            }
            let mut graph: Option<String> = None;
            let mut key: Option<String> = None;
            let mut extension = false;
            for arg in &directive.arguments {
                match arg.name.as_str() {
                    "graph" => {
                        if let Value::Enum(name) = &*arg.value {
                            graph = enum_lookup
                                .get(name.as_str())
                                .cloned()
                                .or_else(|| Some(name.as_str().to_owned()));
                        }
                    }
                    "key" => {
                        if let Value::String(s) = &*arg.value {
                            key = Some(s.clone());
                        }
                    }
                    "extension" => {
                        if let Value::Boolean(b) = &*arg.value {
                            extension = *b;
                        }
                    }
                    _ => {}
                }
            }
            let Some(graph) = graph else { continue };

            if let Some(k) = key.as_ref() {
                if key_fields.is_empty() {
                    key_fields = parse_field_set(k);
                }
            }

            if extension {
                extenders.push(EntityExtender { subgraph: graph, fields: Vec::new() });
            } else {
                owner = Some(graph);
            }
        }

        // Skip objects with no @join__type metadata (locally-scoped value types).
        if owner.is_none() && extenders.is_empty() {
            continue;
        }
        let owner = owner.ok_or_else(|| {
            SupergraphError::Validation(format!(
                "type {name} has no owning @join__type (only extension entries)"
            ))
        })?;
        if !subgraphs.contains_key(&owner) {
            return Err(SupergraphError::Validation(format!(
                "type {name} owner subgraph \"{owner}\" not declared in join__Graph"
            )));
        }

        // Walk fields and figure out which subgraph contributes each one.
        let mut field_owners: HashMap<String, String> = HashMap::new();
        for field in &obj.fields {
            let field_name = field.name.as_str().to_owned();
            let owning_graphs: Vec<String> = field
                .directives
                .iter()
                .filter(|d| d.name.as_str() == "join__field")
                .filter_map(|d| graph_arg(d, &enum_lookup))
                .collect();

            let resolved_owner = if owning_graphs.is_empty() {
                owner.clone()
            } else {
                owning_graphs[0].clone()
            };
            field_owners.insert(field_name.clone(), resolved_owner.clone());

            // Attach to the right extender if not the owner.
            if resolved_owner != owner {
                if let Some(ext) = extenders.iter_mut().find(|e| e.subgraph == resolved_owner) {
                    ext.fields.push(field_name);
                }
            }
        }

        out.insert(
            name.to_owned(),
            EntityType {
                name: name.to_owned(),
                key_fields,
                owner,
                extenders,
                field_owners,
            },
        );
    }
    Ok(out)
}

// ---- root operation field ownership --------------------------------------

fn collect_root_fields(
    document: &Document,
) -> Result<(HashMap<String, String>, HashMap<String, String>), SupergraphError> {
    let enum_lookup = build_enum_lookup(document);
    let mut query = HashMap::new();
    let mut mutation = HashMap::new();
    for def in &document.definitions {
        let Definition::ObjectTypeDefinition(obj) = def else { continue };
        let name = obj.name.as_str();
        let target = match name {
            "Query" => &mut query,
            "Mutation" => &mut mutation,
            _ => continue,
        };
        // Owner of the root type itself - used as fallback when a field has
        // no explicit @join__field annotation.
        let owner_fallback = obj
            .directives
            .iter()
            .find(|d| {
                d.name.as_str() == "join__type"
                    && d.arguments.iter().any(|a| a.name.as_str() == "graph")
            })
            .and_then(|d| graph_arg(d, &enum_lookup));

        for field in &obj.fields {
            let field_name = field.name.as_str().to_owned();
            // Skip federation built-ins.
            if field_name == "_service" || field_name == "_entities" {
                continue;
            }
            let owners: Vec<String> = field
                .directives
                .iter()
                .filter(|d| d.name.as_str() == "join__field")
                .filter_map(|d| graph_arg(d, &enum_lookup))
                .collect();
            let resolved = owners
                .first()
                .cloned()
                .or_else(|| owner_fallback.clone())
                .ok_or_else(|| {
                    SupergraphError::Validation(format!(
                        "{name}.{field_name} has no @join__field and no fallback owner"
                    ))
                })?;
            target.insert(field_name, resolved);
        }
    }
    Ok((query, mutation))
}

// ---- helpers --------------------------------------------------------------

/// Build a map from `join__Graph` enum value name (e.g. "PROPERTY") to the
/// human-friendly subgraph name (e.g. "property"). Apollo composer uses the
/// enum value as the canonical identifier in directives, but we want the
/// `name` from `@join__graph(name: ...)` everywhere downstream so logs and
/// errors read naturally.
fn build_enum_lookup(document: &Document) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for def in &document.definitions {
        let Definition::EnumTypeDefinition(enum_def) = def else { continue };
        if enum_def.name.as_str() != "join__Graph" {
            continue;
        }
        for value in &enum_def.values {
            let enum_name = value.value.as_str().to_owned();
            for directive in &value.directives {
                if directive.name.as_str() != "join__graph" {
                    continue;
                }
                if let Some(name) = directive
                    .arguments
                    .iter()
                    .find(|a| a.name.as_str() == "name")
                    .and_then(|a| {
                        if let Value::String(s) = &*a.value {
                            Some(s.clone())
                        } else {
                            None
                        }
                    })
                {
                    out.insert(enum_name.clone(), name);
                }
            }
        }
    }
    out
}

fn graph_arg(
    directive: &apollo_compiler::ast::Directive,
    lookup: &HashMap<String, String>,
) -> Option<String> {
    directive
        .arguments
        .iter()
        .find(|a| a.name.as_str() == "graph")
        .and_then(|a| match &*a.value {
            Value::Enum(name) => lookup
                .get(name.as_str())
                .cloned()
                .or_else(|| Some(name.as_str().to_owned())),
            Value::String(s) => Some(s.clone()),
            _ => None,
        })
}

/// Parse a `@key(fields: "id sku")` style field set into individual names.
/// We don't support nested key paths or compound keys with selection sets in
/// this Phase 3 cut.
fn parse_field_set(field_set: &str) -> Vec<String> {
    field_set
        .split_whitespace()
        .map(|s| s.to_owned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"
schema
  @link(url: "https://specs.apollo.dev/link/v1.0")
  @link(url: "https://specs.apollo.dev/join/v0.3", for: EXECUTION) {
  query: Query
}

directive @join__graph(name: String!, url: String!) on ENUM_VALUE
directive @join__type(graph: join__Graph!, key: String, extension: Boolean = false) repeatable on OBJECT
directive @join__field(graph: join__Graph!) on FIELD_DEFINITION

scalar join__FieldSet

enum join__Graph {
  PROPERTY @join__graph(name: "property", url: "http://property:8081/graphql")
  PRICING  @join__graph(name: "pricing",  url: "http://pricing:8082/graphql")
  REVIEW   @join__graph(name: "review",   url: "http://review:8085/graphql")
}

type Query
  @join__type(graph: PROPERTY)
  @join__type(graph: PRICING)
  @join__type(graph: REVIEW) {
  searchProperties(city: String!): [Property!]! @join__field(graph: PROPERTY)
  property(id: ID!): Property @join__field(graph: PROPERTY)
  price(propertyId: ID!): Price @join__field(graph: PRICING)
  reviews(propertyId: ID!): [Review!]! @join__field(graph: REVIEW)
}

type Property
  @join__type(graph: PROPERTY, key: "id")
  @join__type(graph: PRICING, key: "id", extension: true)
  @join__type(graph: REVIEW, key: "id", extension: true) {
  id: ID!
  name: String! @join__field(graph: PROPERTY)
  price: Price @join__field(graph: PRICING)
  reviews: [Review!]! @join__field(graph: REVIEW)
}

type Price @join__type(graph: PRICING) {
  totalAmount: String!
}

type Review @join__type(graph: REVIEW) {
  rating: Int!
}
"#;

    #[test]
    fn parses_subgraphs_and_routes() {
        let cat = parse(FIXTURE).unwrap();
        assert_eq!(cat.subgraphs.len(), 3);
        assert!(cat.subgraphs.contains_key("property"));
        assert_eq!(cat.subgraphs["pricing"].url, "http://pricing:8082/graphql");
    }

    #[test]
    fn maps_root_query_fields_to_owning_subgraphs() {
        let cat = parse(FIXTURE).unwrap();
        assert_eq!(cat.root_query_fields["searchProperties"], "property");
        assert_eq!(cat.root_query_fields["price"], "pricing");
        assert_eq!(cat.root_query_fields["reviews"], "review");
        assert!(!cat.root_query_fields.contains_key("_service"));
    }

    #[test]
    fn captures_property_owner_and_extenders() {
        let cat = parse(FIXTURE).unwrap();
        let prop = &cat.entity_types["Property"];
        assert_eq!(prop.owner, "property");
        assert_eq!(prop.key_fields, vec!["id".to_string()]);
        let extenders: Vec<&str> = prop.extenders.iter().map(|e| e.subgraph.as_str()).collect();
        assert!(extenders.contains(&"pricing"));
        assert!(extenders.contains(&"review"));
        assert_eq!(prop.field_owners["price"], "pricing");
        assert_eq!(prop.field_owners["reviews"], "review");
        assert_eq!(prop.field_owners["name"], "property");
    }
}
