//! Query planning algorithm.
//!
//! Strategy (kept narrow on purpose - covers Phase 3 acceptance and grows
//! incrementally):
//!
//! 1. Locate the operation (named or anonymous) in the parsed document.
//! 2. For each top-level selection, look up the owning subgraph in
//!    [`SupergraphCatalog::root_query_fields`] / `root_mutation_fields`.
//! 3. If the field returns a known entity type, partition the entity's
//!    sub-selections by owning subgraph:
//!      * fields owned by the initial subgraph stay in the initial query,
//!      * fields owned by extenders are pulled out into `_entities`
//!        fragments and grouped per-subgraph.
//! 4. Always include `__typename` and the entity key fields in the initial
//!    query so the executor can build representations for the
//!    `_entities` calls.
//! 5. Variables: only those actually referenced by the synthesized operation
//!    are forwarded.
//!
//! Things we deliberately don't do here:
//!   * nested entity refs beyond one level (a `Review.author` entity inside
//!     a `Property.reviews` list) - the planner just inlines them in the
//!     extending subgraph's fetch, which works as long as the extending
//!     subgraph can resolve them locally.
//!   * `@requires` / `@provides` field requirements - none of our subgraphs
//!     declare them yet.
//!   * fragment spreads / inline fragments (we'd inline them before
//!     planning).

use crate::graphql::types::GraphQLError;
use crate::supergraph::SupergraphCatalog;
use apollo_compiler::ast::{Definition, Document, OperationDefinition, OperationType, Selection, Value, VariableDefinition};
use std::collections::{BTreeSet, HashMap, HashSet};

use super::types::{
    EntityFetch, ExecutionPlan, FieldFetch, InitialFetch, OperationKind, SelectionPath,
};

#[derive(Debug, thiserror::Error)]
pub enum PlanError {
    #[error("query has no executable operations")]
    NoOperations,
    #[error("operation \"{0}\" not found in document")]
    UnknownOperation(String),
    #[error("multiple operations require an operationName")]
    AmbiguousOperation,
    #[error("subscription operations are not supported by the router")]
    UnsupportedSubscription,
    #[error("no subgraph owns root field \"{0}\"")]
    UnknownRootField(String),
    #[error("subgraph \"{0}\" referenced by the supergraph is not in the route table")]
    UnknownSubgraph(String),
}

impl PlanError {
    pub fn into_graphql(self) -> GraphQLError {
        GraphQLError {
            message: self.to_string(),
            path: Vec::new(),
            locations: Vec::new(),
            extensions: Some(serde_json::json!({"code": "PLAN_ERROR"})),
        }
    }
}

pub fn plan_operation(
    document: &Document,
    operation_name: Option<&str>,
    catalog: &SupergraphCatalog,
) -> Result<ExecutionPlan, PlanError> {
    let op = pick_operation(document, operation_name)?;

    let kind = match op.operation_type {
        OperationType::Query => OperationKind::Query,
        OperationType::Mutation => OperationKind::Mutation,
        OperationType::Subscription => return Err(PlanError::UnsupportedSubscription),
    };

    let root_owners: &HashMap<String, String> = match kind {
        OperationKind::Query => &catalog.root_query_fields,
        OperationKind::Mutation => &catalog.root_mutation_fields,
    };

    let mut field_fetches: Vec<FieldFetch> = Vec::new();
    for selection in &op.selection_set {
        let Selection::Field(field) = selection else { continue };
        let field_name = field.name.as_str();
        let response_key = field
            .alias
            .as_ref()
            .map(|a| a.as_str().to_owned())
            .unwrap_or_else(|| field_name.to_owned());

        // Skip federation built-ins; the router answers `_service`/`_entities`
        // by routing them to subgraphs that own the relevant entities (we
        // don't intercept yet).
        if matches!(field_name, "__schema" | "__type") {
            continue;
        }

        let owner = root_owners.get(field_name).cloned().ok_or_else(|| {
            PlanError::UnknownRootField(format!("{}.{field_name}", root_kind_label(kind)))
        })?;
        let route = catalog
            .subgraphs
            .get(&owner)
            .ok_or_else(|| PlanError::UnknownSubgraph(owner.clone()))?;

        // Identify the entity type returned (if any) by looking at the
        // top-level subselection: fields whose owners differ from the
        // initial subgraph indicate this must be an entity return type.
        let entity_type = guess_entity_type(field, catalog);

        // Track which of the entity's @key fields the client actually asked
        // for - the merger uses this to decide whether to strip the
        // planner-injected key fields from the final response.
        let mut client_requested_id_keys: HashSet<String> = HashSet::new();
        if let Some(type_name) = &entity_type {
            if let Some(entity) = catalog.entity_types.get(type_name) {
                for sel in &field.selection_set {
                    if let Selection::Field(sub) = sel {
                        let n = sub.name.as_str();
                        if entity.key_fields.iter().any(|k| k == n) {
                            client_requested_id_keys.insert(n.to_string());
                        }
                    }
                }
            }
        }

        // Partition sub-selections by owning subgraph.
        let (initial_selection_text, extender_groups) = match &entity_type {
            Some(type_name) => partition_for_entity(field, type_name, &owner, catalog),
            None => (selection_set_text(field), Vec::<EntenderGroup>::new()),
        };

        // Build the synthesized initial operation.
        let initial_op_text = synthesize_root_operation(
            kind,
            op.variables.as_slice(),
            field,
            &initial_selection_text,
        );

        let referenced_vars = collect_variable_names(&initial_op_text);

        let initial = InitialFetch {
            subgraph: route.name.clone(),
            url: route.url.clone(),
            timeout: route.timeout,
            query_text: initial_op_text,
            variable_names: referenced_vars,
        };

        // Build entity fetches for each extending subgraph that has
        // contributing selections.
        let mut entity_fetches: Vec<EntityFetch> = Vec::new();
        if let Some(type_name) = entity_type.clone() {
            let entity = catalog
                .entity_types
                .get(&type_name)
                .expect("entity type must exist when extender_groups was computed");
            let key_field = entity
                .key_fields
                .first()
                .cloned()
                .unwrap_or_else(|| "id".to_string());

            for group in extender_groups {
                let route = catalog
                    .subgraphs
                    .get(&group.subgraph)
                    .ok_or_else(|| PlanError::UnknownSubgraph(group.subgraph.clone()))?;
                entity_fetches.push(EntityFetch {
                    subgraph: route.name.clone(),
                    url: route.url.clone(),
                    timeout: route.timeout,
                    type_name: type_name.clone(),
                    key_field: key_field.clone(),
                    fragment_body: group.body,
                    selection_path: SelectionPath {
                        root_key: response_key.clone(),
                        is_list: looks_like_list(field, catalog),
                    },
                });
            }
        }

        field_fetches.push(FieldFetch {
            response_key,
            is_list: looks_like_list(field, catalog),
            entity_type,
            initial,
            entity_fetches,
            client_requested_id_keys,
        });
    }

    if field_fetches.is_empty() {
        return Err(PlanError::NoOperations);
    }

    Ok(ExecutionPlan {
        operation_kind: kind,
        operation_name: op.name.as_ref().map(|n| n.as_str().to_owned()),
        field_fetches,
    })
}

fn root_kind_label(kind: OperationKind) -> &'static str {
    match kind {
        OperationKind::Query => "Query",
        OperationKind::Mutation => "Mutation",
    }
}

// ---- operation selection -------------------------------------------------

fn pick_operation<'a>(
    document: &'a Document,
    requested: Option<&str>,
) -> Result<&'a OperationDefinition, PlanError> {
    let operations: Vec<&OperationDefinition> = document
        .definitions
        .iter()
        .filter_map(|d| match d {
            Definition::OperationDefinition(op) => Some(op.as_ref()),
            _ => None,
        })
        .collect();

    if operations.is_empty() {
        return Err(PlanError::NoOperations);
    }
    if let Some(name) = requested {
        operations
            .iter()
            .find(|op| op.name.as_ref().map(|n| n.as_str()) == Some(name))
            .copied()
            .ok_or_else(|| PlanError::UnknownOperation(name.to_owned()))
    } else if operations.len() == 1 {
        Ok(operations[0])
    } else {
        Err(PlanError::AmbiguousOperation)
    }
}

// ---- entity detection / partitioning ------------------------------------

/// Look at the top-level field's selection set. If the inbound query
/// requests sub-fields whose owning subgraphs include any of the catalog's
/// entity types, treat this as an entity return type.
fn guess_entity_type(
    field: &apollo_compiler::ast::Field,
    catalog: &SupergraphCatalog,
) -> Option<String> {
    let leaf_field_names: Vec<&str> = field
        .selection_set
        .iter()
        .filter_map(|s| match s {
            Selection::Field(f) => Some(f.name.as_str()),
            _ => None,
        })
        .collect();
    if leaf_field_names.is_empty() {
        return None;
    }
    // Find the entity type whose `field_owners` contains every selected
    // sub-field. We pick the first match - a more sophisticated planner
    // would resolve this from the schema directly, but the supergraph SDL
    // doesn't carry "this root field returns Property" metadata in our
    // simplified model. This heuristic works because our entity types
    // (Property, Booking, User) have disjoint field names.
    let mut best: Option<&String> = None;
    let mut best_score: usize = 0;
    for (type_name, entity) in &catalog.entity_types {
        let score = leaf_field_names
            .iter()
            .filter(|n| entity.field_owners.contains_key(**n))
            .count();
        if score > best_score {
            best_score = score;
            best = Some(type_name);
        }
    }
    if best_score == 0 {
        None
    } else {
        best.cloned()
    }
}

#[derive(Debug)]
struct EntenderGroup {
    subgraph: String,
    /// Inline-fragment body (the part between `{` and `}`).
    body: String,
}

/// Split the inbound field's selection set into:
///   * `initial_selection_text` - braces-wrapped selection set for the
///     initial fetch (always includes `__typename` + key field, plus any
///     fields owned by the initial subgraph).
///   * one [`EntenderGroup`] per extending subgraph that has contributing
///     selections.
fn partition_for_entity(
    field: &apollo_compiler::ast::Field,
    entity_type_name: &str,
    initial_subgraph: &str,
    catalog: &SupergraphCatalog,
) -> (String, Vec<EntenderGroup>) {
    let entity = match catalog.entity_types.get(entity_type_name) {
        Some(e) => e,
        None => return (selection_set_text(field), Vec::new()),
    };
    let key_field = entity
        .key_fields
        .first()
        .cloned()
        .unwrap_or_else(|| "id".to_string());

    let mut initial_fields: Vec<String> = vec!["__typename".to_string(), key_field.clone()];
    let mut by_extender: HashMap<String, Vec<String>> = HashMap::new();

    for selection in &field.selection_set {
        let Selection::Field(sub) = selection else { continue };
        let sub_name = sub.name.as_str();
        if sub_name == "__typename" || sub_name == key_field {
            continue; // Already covered by the canonical bookkeeping fields.
        }
        let owner = entity
            .field_owners
            .get(sub_name)
            .cloned()
            .unwrap_or_else(|| initial_subgraph.to_string());
        let rendered = render_field(sub);
        if owner == initial_subgraph {
            initial_fields.push(rendered);
        } else {
            by_extender.entry(owner).or_default().push(rendered);
        }
    }

    let initial_text = format!("{{ {} }}", initial_fields.join(" "));
    let extender_groups: Vec<EntenderGroup> = by_extender
        .into_iter()
        .map(|(subgraph, fields)| EntenderGroup {
            subgraph,
            body: fields.join(" "),
        })
        .collect();

    (initial_text, extender_groups)
}

// ---- AST -> text rendering ----------------------------------------------

fn selection_set_text(field: &apollo_compiler::ast::Field) -> String {
    if field.selection_set.is_empty() {
        return String::new();
    }
    let inner = field
        .selection_set
        .iter()
        .map(render_selection)
        .collect::<Vec<_>>()
        .join(" ");
    format!("{{ {inner} }}")
}

fn render_selection(selection: &Selection) -> String {
    match selection {
        Selection::Field(f) => render_field(f),
        Selection::FragmentSpread(spread) => format!("...{}", spread.fragment_name),
        Selection::InlineFragment(inline) => {
            let cond = inline
                .type_condition
                .as_ref()
                .map(|t| format!("... on {} ", t))
                .unwrap_or_default();
            let body = inline
                .selection_set
                .iter()
                .map(render_selection)
                .collect::<Vec<_>>()
                .join(" ");
            format!("{cond}{{ {body} }}")
        }
    }
}

fn render_field(field: &apollo_compiler::ast::Field) -> String {
    let alias = field
        .alias
        .as_ref()
        .map(|a| format!("{}: ", a))
        .unwrap_or_default();
    let name = field.name.as_str();
    let args = if field.arguments.is_empty() {
        String::new()
    } else {
        let parts: Vec<String> = field
            .arguments
            .iter()
            .map(|arg| format!("{}: {}", arg.name, render_value(&arg.value)))
            .collect();
        format!("({})", parts.join(", "))
    };
    let body = if field.selection_set.is_empty() {
        String::new()
    } else {
        let inner = field
            .selection_set
            .iter()
            .map(render_selection)
            .collect::<Vec<_>>()
            .join(" ");
        format!(" {{ {inner} }}")
    };
    format!("{alias}{name}{args}{body}")
}

fn render_value(value: &Value) -> String {
    match value {
        Value::Variable(name) => format!("${name}"),
        Value::Int(i) => i.as_str().to_string(),
        Value::Float(f) => f.as_str().to_string(),
        Value::String(s) => format!("\"{}\"", s.replace('"', "\\\"")),
        Value::Boolean(b) => b.to_string(),
        Value::Null => "null".to_string(),
        Value::Enum(name) => name.as_str().to_string(),
        Value::List(items) => {
            let parts: Vec<String> = items.iter().map(|v| render_value(v)).collect();
            format!("[{}]", parts.join(", "))
        }
        Value::Object(entries) => {
            let parts: Vec<String> = entries
                .iter()
                .map(|(name, v)| format!("{name}: {}", render_value(v)))
                .collect();
            format!("{{{}}}", parts.join(", "))
        }
    }
}

fn synthesize_root_operation(
    kind: OperationKind,
    declared_vars: &[apollo_compiler::Node<VariableDefinition>],
    field: &apollo_compiler::ast::Field,
    initial_selection_text: &str,
) -> String {
    let kw = match kind {
        OperationKind::Query => "query",
        OperationKind::Mutation => "mutation",
    };
    let rendered_field = render_field(field);
    let stripped = strip_selection_set(&rendered_field);
    let with_initial = format!("{stripped} {initial_selection_text}");

    // Prune unused variables.
    let used_vars = collect_variable_uses_in_str(&with_initial);
    let var_decl = if used_vars.is_empty() {
        String::new()
    } else {
        let kept: Vec<String> = declared_vars
            .iter()
            .filter(|v| used_vars.contains(v.name.as_str()))
            .map(|v| render_variable(v.as_ref()))
            .collect();
        if kept.is_empty() {
            String::new()
        } else {
            format!("({})", kept.join(", "))
        }
    };
    format!("{kw}{var_decl} {{ {with_initial} }}")
}

fn render_variable(var: &VariableDefinition) -> String {
    let mut s = format!("${}: {}", var.name, var.ty);
    if let Some(default) = &var.default_value {
        s.push_str(&format!(" = {}", render_value(default)));
    }
    s
}

/// Drop an existing `{ ... }` selection set tail so we can replace it with
/// a synthesized one. Operates on the rendered field; the planner produces
/// strings, not ASTs, so a rough text strip is enough.
fn strip_selection_set(rendered_field: &str) -> String {
    let bytes = rendered_field.as_bytes();
    let mut depth = 0;
    let mut start: Option<usize> = None;
    for (i, b) in bytes.iter().enumerate() {
        match b {
            b'{' => {
                if depth == 0 {
                    start = Some(i);
                }
                depth += 1;
            }
            b'}' => {
                depth -= 1;
            }
            _ => {}
        }
    }
    match start {
        Some(s) => rendered_field[..s].trim_end().to_string(),
        None => rendered_field.to_string(),
    }
}

fn collect_variable_names(text: &str) -> Vec<String> {
    let used = collect_variable_uses_in_str(text);
    let mut out: Vec<String> = used.into_iter().collect();
    out.sort();
    out
}

fn collect_variable_uses_in_str(text: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'$' {
            let mut j = i + 1;
            while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
                j += 1;
            }
            if j > i + 1 {
                out.insert(text[i + 1..j].to_string());
            }
            i = j;
        } else {
            i += 1;
        }
    }
    out
}

/// Heuristic: does this field's selection set look like a list-shaped
/// response? Used only for plan logging - the executor inspects the actual
/// JSON shape at runtime.
fn looks_like_list(field: &apollo_compiler::ast::Field, _catalog: &SupergraphCatalog) -> bool {
    let name = field.name.as_str();
    name.starts_with("search") || (name.ends_with('s') && !name.ends_with("ss"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::supergraph::parse as parse_supergraph;

    const SUPERGRAPH: &str = r#"
schema { query: Query }
directive @join__graph(name: String!, url: String!) on ENUM_VALUE
directive @join__type(graph: join__Graph!, key: String, extension: Boolean = false) repeatable on OBJECT
directive @join__field(graph: join__Graph!) on FIELD_DEFINITION
scalar join__FieldSet

enum join__Graph {
  PROPERTY @join__graph(name: "property", url: "http://prop:8081/graphql")
  PRICING  @join__graph(name: "pricing",  url: "http://price:8082/graphql")
  REVIEW   @join__graph(name: "review",   url: "http://review:8085/graphql")
}

type Query
  @join__type(graph: PROPERTY)
  @join__type(graph: PRICING)
  @join__type(graph: REVIEW) {
  searchProperties(city: String!): [Property!]! @join__field(graph: PROPERTY)
  property(id: ID!): Property @join__field(graph: PROPERTY)
  reviewSummary(propertyId: ID!): ReviewSummary! @join__field(graph: REVIEW)
}

type Property
  @join__type(graph: PROPERTY, key: "id")
  @join__type(graph: PRICING,  key: "id", extension: true)
  @join__type(graph: REVIEW,   key: "id", extension: true) {
  id: ID!
  name: String! @join__field(graph: PROPERTY)
  price: Price @join__field(graph: PRICING)
  reviews: [Review!]! @join__field(graph: REVIEW)
}

type Price @join__type(graph: PRICING) { totalAmount: String! }
type Review @join__type(graph: REVIEW) { rating: Int! }
type ReviewSummary @join__type(graph: REVIEW) { count: Int! }
"#;

    fn parse_query(src: &str) -> apollo_compiler::ast::Document {
        apollo_compiler::ast::Document::parse(src, "q.graphql")
            .expect("query must parse cleanly in test")
    }

    #[test]
    fn single_subgraph_query_produces_single_subgraph_plan() {
        let cat = parse_supergraph(SUPERGRAPH).unwrap();
        let doc = parse_query("{ reviewSummary(propertyId: \"x\") { count } }");
        let plan = plan_operation(&doc, None, &cat).unwrap();
        assert_eq!(plan.field_fetches.len(), 1);
        let f = &plan.field_fetches[0];
        assert_eq!(f.initial.subgraph, "review");
        assert!(f.entity_fetches.is_empty(), "single-subgraph queries must not generate entity fetches");
    }

    #[test]
    fn property_with_reviews_plans_initial_then_entity_fetch() {
        let cat = parse_supergraph(SUPERGRAPH).unwrap();
        let doc = parse_query("{ property(id: \"x\") { name reviews { rating } } }");
        let plan = plan_operation(&doc, None, &cat).unwrap();
        let f = &plan.field_fetches[0];
        assert_eq!(f.initial.subgraph, "property");
        assert_eq!(f.entity_type.as_deref(), Some("Property"));
        // Initial query carries __typename + id + name (owned by property).
        assert!(f.initial.query_text.contains("__typename"));
        assert!(f.initial.query_text.contains(" id "), "id should be projected: {}", f.initial.query_text);
        assert!(f.initial.query_text.contains("name"));
        assert!(!f.initial.query_text.contains("rating"));
        assert_eq!(f.entity_fetches.len(), 1);
        assert_eq!(f.entity_fetches[0].subgraph, "review");
        assert!(f.entity_fetches[0].fragment_body.contains("reviews"));
    }

    #[test]
    fn search_with_price_and_reviews_plans_two_extender_fetches() {
        let cat = parse_supergraph(SUPERGRAPH).unwrap();
        let doc = parse_query(
            "{ searchProperties(city: \"Austin\") { name price { totalAmount } reviews { rating } } }",
        );
        let plan = plan_operation(&doc, None, &cat).unwrap();
        let f = &plan.field_fetches[0];
        assert_eq!(f.initial.subgraph, "property");
        let extenders: Vec<_> = f.entity_fetches.iter().map(|e| e.subgraph.as_str()).collect();
        assert!(extenders.contains(&"pricing"));
        assert!(extenders.contains(&"review"));
        assert_eq!(f.entity_fetches.len(), 2, "one batched _entities call per extending subgraph");
    }

    #[test]
    fn unknown_root_field_is_a_plan_error() {
        let cat = parse_supergraph(SUPERGRAPH).unwrap();
        let doc = parse_query("{ unknownField }");
        let err = plan_operation(&doc, None, &cat).expect_err("unknown root must error");
        assert!(matches!(err, PlanError::UnknownRootField(_)));
    }
}
