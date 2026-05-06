//! Project a parsed operation onto a per-subgraph operation.
//!
//! Phase 2 doesn't have a real query planner yet (Phase 3 will). For each
//! top-level field of the request operation we look up the owning subgraph
//! in the [`SubgraphRegistry`] and synthesize a smaller operation that
//! contains *only* the fields that subgraph owns.
//!
//! Variables are filtered to those actually referenced inside the projected
//! sub-tree so that the subgraph doesn't reject the request for
//! "variable defined but unused" (GraphQL spec rule 5.8.4).
//!
//! NOTE: This is intentionally a hand-coded gateway and NOT real Apollo
//! Federation. Cross-subgraph entity stitching (e.g. resolving `Property.price`
//! through `_entities`) lands in Phase 3.

use apollo_compiler::ast::{
    Definition, Document, OperationDefinition, Selection, Value, VariableDefinition,
};
use apollo_compiler::Node;
use std::collections::HashMap;

use crate::registry::SubgraphRegistry;

use super::types::GraphQLError;

/// One per-subgraph projection: the fields that subgraph owns plus the
/// serialized GraphQL text of the synthetic operation we'll send to it.
#[derive(Debug)]
pub struct PerSubgraphPlan {
    pub subgraph: String,
    pub query_text: String,
    /// The top-level response keys this subgraph contributes to the merged
    /// response (alias if present, otherwise the field name).
    pub response_keys: Vec<String>,
}

/// Build per-subgraph plans for one top-level operation.
///
/// Returns:
///   * `Ok(plans)` - one plan per subgraph that has at least one field to run,
///   * `Err(errors)` - GraphQL errors describing routing problems
///     (unknown top-level field, fragment spread at top level, etc.).
pub fn plan(
    document: &Document,
    operation_name: Option<&str>,
    registry: &SubgraphRegistry,
) -> Result<Vec<PerSubgraphPlan>, Vec<GraphQLError>> {
    let op = pick_operation(document, operation_name)?;

    // Group top-level selections by subgraph name (preserve original order).
    let mut order: Vec<String> = Vec::new();
    let mut buckets: HashMap<String, Vec<Selection>> = HashMap::new();

    for sel in &op.selection_set {
        match sel {
            Selection::Field(field) => {
                let field_name = field.name.as_str();
                let subgraph = registry
                    .subgraph_for(op.operation_type, field_name)
                    .ok_or_else(|| {
                        vec![GraphQLError::message(format!(
                            "No subgraph owns top-level field \"{field_name}\"."
                        ))]
                    })?;
                if !buckets.contains_key(subgraph) {
                    order.push(subgraph.to_owned());
                }
                buckets.entry(subgraph.to_owned()).or_default().push(sel.clone());
            }
            Selection::FragmentSpread(_) | Selection::InlineFragment(_) => {
                return Err(vec![GraphQLError::message(
                    "Fragment spreads at the top level are not supported by the Phase 2 router. \
                     They will be handled by the Phase 3 query planner.",
                )]);
            }
        }
    }

    let mut plans = Vec::with_capacity(order.len());
    for subgraph in order {
        let selections = buckets.remove(&subgraph).unwrap_or_default();
        let response_keys = selections
            .iter()
            .filter_map(|s| match s {
                Selection::Field(f) => Some(
                    f.alias
                        .as_ref()
                        .map(|n| n.as_str().to_owned())
                        .unwrap_or_else(|| f.name.as_str().to_owned()),
                ),
                _ => None,
            })
            .collect::<Vec<_>>();

        let query_text = synthesize_operation(op, &selections);
        plans.push(PerSubgraphPlan {
            subgraph,
            query_text,
            response_keys,
        });
    }
    Ok(plans)
}

/// Pick the operation to execute. Mirrors apollo-compiler's higher-level
/// `operations.get(name)` helper without requiring a schema.
fn pick_operation<'doc>(
    document: &'doc Document,
    operation_name: Option<&str>,
) -> Result<&'doc OperationDefinition, Vec<GraphQLError>> {
    let mut chosen: Option<&OperationDefinition> = None;
    let mut count = 0usize;

    for def in &document.definitions {
        if let Definition::OperationDefinition(op) = def {
            count += 1;
            match operation_name {
                Some(want) => {
                    if op.name.as_ref().map(|n| n.as_str()) == Some(want) {
                        chosen = Some(op);
                    }
                }
                None => {
                    chosen = Some(op);
                }
            }
        }
    }

    if count == 0 {
        return Err(vec![GraphQLError::message(
            "Document contains no executable operations.",
        )]);
    }
    if operation_name.is_none() && count > 1 {
        return Err(vec![GraphQLError::message(
            "Document contains multiple operations but no operationName was provided.",
        )]);
    }

    chosen.ok_or_else(|| match operation_name {
        Some(name) => vec![GraphQLError::message(format!(
            "Operation \"{name}\" not found in document."
        ))],
        None => vec![GraphQLError::message("No operation could be selected.")],
    })
}

/// Build the GraphQL text for the per-subgraph operation. We construct a
/// fresh [`OperationDefinition`] so we can prune unused variables and serialize
/// it via apollo-compiler's `Display` impl.
fn synthesize_operation(source: &OperationDefinition, selections: &[Selection]) -> String {
    let used_vars = collect_variable_uses(selections);

    let variables: Vec<Node<VariableDefinition>> = source
        .variables
        .iter()
        .filter(|var| used_vars.contains(var.name.as_str()))
        .cloned()
        .collect();

    let new_op = OperationDefinition {
        operation_type: source.operation_type,
        name: source.name.clone(),
        variables,
        directives: source.directives.clone(),
        selection_set: selections.to_vec(),
    };

    let new_doc = Document {
        definitions: vec![Definition::OperationDefinition(Node::new(new_op))],
        ..Default::default()
    };
    new_doc.serialize().to_string()
}

fn collect_variable_uses(selections: &[Selection]) -> std::collections::HashSet<String> {
    let mut out = std::collections::HashSet::new();
    walk_selections(selections, &mut out);
    out
}

fn walk_selections(selections: &[Selection], out: &mut std::collections::HashSet<String>) {
    for sel in selections {
        match sel {
            Selection::Field(field) => {
                for arg in &field.arguments {
                    walk_value(&arg.value, out);
                }
                walk_selections(&field.selection_set, out);
            }
            Selection::InlineFragment(frag) => walk_selections(&frag.selection_set, out),
            Selection::FragmentSpread(_) => { /* not supported at top level (see plan()) */ }
        }
    }
}

fn walk_value(value: &Value, out: &mut std::collections::HashSet<String>) {
    match value {
        Value::Variable(name) => {
            out.insert(name.as_str().to_owned());
        }
        Value::List(items) => items.iter().for_each(|v| walk_value(v, out)),
        Value::Object(fields) => fields.iter().for_each(|(_, v)| walk_value(v, out)),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, Server, SubgraphConfig};
    use crate::graphql::parse::parse;

    fn registry() -> SubgraphRegistry {
        let mut subgraphs = std::collections::HashMap::new();
        subgraphs.insert(
            "property".to_owned(),
            SubgraphConfig {
                url: "http://property/graphql".into(),
                fields: vec!["searchProperties".into(), "property".into()],
                mutations: vec![],
                timeout_ms: None,
            },
        );
        subgraphs.insert(
            "review".to_owned(),
            SubgraphConfig {
                url: "http://review/graphql".into(),
                fields: vec!["reviewSummary".into()],
                mutations: vec![],
                timeout_ms: None,
            },
        );
        SubgraphRegistry::from_config(&Config {
            server: Server {
                port: 8080,
                default_subgraph_timeout_ms: 1000,
            },
            subgraphs,
        })
        .unwrap()
    }

    #[test]
    fn produces_one_plan_per_subgraph_with_expected_response_keys() {
        let r = registry();
        let parsed = parse(
            "{ searchProperties(city: \"Austin\") { id name } reviewSummary(propertyId: \"00000000-0000-0000-0000-000000000001\") { count } }",
        )
        .unwrap();
        let plans = plan(&parsed.document, None, &r).unwrap();
        assert_eq!(plans.len(), 2);
        let property_plan = plans.iter().find(|p| p.subgraph == "property").unwrap();
        let review_plan = plans.iter().find(|p| p.subgraph == "review").unwrap();
        assert_eq!(property_plan.response_keys, vec!["searchProperties"]);
        assert_eq!(review_plan.response_keys, vec!["reviewSummary"]);
        // Each per-subgraph projection contains only that subgraph's field.
        assert!(property_plan.query_text.contains("searchProperties"));
        assert!(!property_plan.query_text.contains("reviewSummary"));
        assert!(review_plan.query_text.contains("reviewSummary"));
        assert!(!review_plan.query_text.contains("searchProperties"));
    }

    #[test]
    fn unknown_field_is_a_routing_error() {
        let r = registry();
        let parsed = parse("{ thisFieldDoesNotExist }").unwrap();
        let errs = plan(&parsed.document, None, &r).unwrap_err();
        assert!(errs[0].message.contains("thisFieldDoesNotExist"));
    }
}
