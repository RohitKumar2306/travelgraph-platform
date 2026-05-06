//! Query depth + complexity (Phase 4.3).

use crate::graphql::types::GraphQLError;
use crate::plan::OperationKind;
use crate::supergraph::SupergraphCatalog;
use apollo_compiler::ast::{Definition, Document, OperationType, Selection};
use std::collections::HashMap;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct CostReport {
    pub depth: usize,
    pub cost: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum CostLimitError {
    #[error("query has no executable operations")]
    NoOperations,
    #[error("operation \"{0}\" not found")]
    UnknownOperation(String),
    #[error("multiple operations require an operationName")]
    AmbiguousOperation,
    #[error("query nesting depth {depth} exceeds limit {max_depth}")]
    Depth { depth: usize, max_depth: usize },
    #[error("query complexity score {cost} exceeds limit {max_cost}")]
    Cost { cost: u32, max_cost: u32 },
    #[error("subscriptions are blocked by the router")]
    SubscriptionBlocked,
}

impl CostLimitError {
    pub fn into_graphql(self) -> GraphQLError {
        let (depth, max_depth, cost, max_cost) = match &self {
            CostLimitError::Depth { depth, max_depth } => {
                (Some(*depth), Some(*max_depth), None, None)
            }
            CostLimitError::Cost { cost, max_cost } => (None, None, Some(*cost), Some(*max_cost)),
            _ => (None, None, None, None),
        };
        let mut ext = serde_json::json!({
            "code": "GRAPHQL_COST_LIMIT",
        });
        if let Some(d) = depth {
            ext["depth"] = serde_json::json!(d);
            ext["maxDepth"] = serde_json::json!(max_depth.unwrap());
        }
        if let Some(c) = cost {
            ext["cost"] = serde_json::json!(c);
            ext["maxCost"] = serde_json::json!(max_cost.unwrap());
        }
        GraphQLError {
            message: self.to_string(),
            path: Vec::new(),
            locations: Vec::new(),
            extensions: Some(ext),
        }
    }
}

pub fn analyze(
    document: &Document,
    operation_name: Option<&str>,
    catalog: &SupergraphCatalog,
    max_depth: usize,
    max_cost: u32,
) -> Result<CostReport, CostLimitError> {
    let op = pick_operation(document, operation_name)?;
    if matches!(op.operation_type, OperationType::Subscription) {
        return Err(CostLimitError::SubscriptionBlocked);
    }
    let op_kind = match op.operation_type {
        OperationType::Query => OperationKind::Query,
        OperationType::Mutation => OperationKind::Mutation,
        OperationType::Subscription => unreachable!(),
    };

    let parent_type = match op_kind {
        OperationKind::Query => "Query",
        OperationKind::Mutation => "Mutation",
    };
    let overrides = &catalog.field_cost_overrides;
    let mut ctx = Ctx {
        overrides,
        parent_type,
        depth_seen: 0,
        cost: 0,
        op_kind,
    };
    for sel in &op.selection_set {
        walk_selection(sel, 1, &mut ctx);
    }

    if ctx.op_kind == OperationKind::Mutation {
        ctx.cost = ctx.cost.saturating_add(30);
    }

    let depth = ctx.depth_seen;
    let cost = ctx.cost;

    if depth > max_depth {
        return Err(CostLimitError::Depth {
            depth,
            max_depth,
        });
    }
    if cost > max_cost {
        return Err(CostLimitError::Cost { cost, max_cost });
    }
    Ok(CostReport { depth, cost })
}

struct Ctx<'a> {
    overrides: &'a HashMap<String, i32>,
    parent_type: &'a str,
    depth_seen: usize,
    cost: u32,
    op_kind: OperationKind,
}

fn walk_selection(sel: &Selection, depth: usize, ctx: &mut Ctx<'_>) {
    match sel {
        Selection::Field(field) => {
            ctx.depth_seen = ctx.depth_seen.max(depth);
            let fname = field.name.as_str();
            let key = format!("{}.{}", ctx.parent_type, fname);
            let mut field_cost: u32 = if fname.contains("search") {
                20
            } else {
                1
            };
            if let Some(o) = ctx.overrides.get(&key) {
                field_cost = (*o).clamp(0, i32::MAX) as u32;
            }

            if field.selection_set.is_empty() {
                ctx.cost = ctx.cost.saturating_add(field_cost);
                return;
            }

            ctx.cost = ctx.cost.saturating_add(field_cost.saturating_add(3));
            if looks_like_list_field(fname) {
                ctx.cost = ctx.cost.saturating_add(5);
            }

            for sub in &field.selection_set {
                walk_selection(sub, depth + 1, ctx);
            }
        }
        Selection::InlineFragment(frag) => {
            for sub in &frag.selection_set {
                walk_selection(sub, depth + 1, ctx);
            }
        }
        Selection::FragmentSpread(_) => {
            ctx.cost = ctx.cost.saturating_add(1);
        }
    }
}

fn looks_like_list_field(name: &str) -> bool {
    name.starts_with("search") || (name.ends_with('s') && !name.ends_with("ss"))
}

fn pick_operation<'a>(
    document: &'a Document,
    requested: Option<&str>,
) -> Result<&'a apollo_compiler::ast::OperationDefinition, CostLimitError> {
    let operations: Vec<_> = document
        .definitions
        .iter()
        .filter_map(|d| match d {
            Definition::OperationDefinition(op) => Some(op.as_ref()),
            _ => None,
        })
        .collect();

    if operations.is_empty() {
        return Err(CostLimitError::NoOperations);
    }
    if let Some(name) = requested {
        operations
            .into_iter()
            .find(|op| op.name.as_ref().map(|n| n.as_str()) == Some(name))
            .ok_or_else(|| CostLimitError::UnknownOperation(name.to_owned()))
    } else if operations.len() == 1 {
        Ok(operations[0])
    } else {
        Err(CostLimitError::AmbiguousOperation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphql::parse::parse;

    fn empty_cat() -> SupergraphCatalog {
        SupergraphCatalog {
            subgraphs: HashMap::new(),
            root_query_fields: HashMap::new(),
            root_mutation_fields: HashMap::new(),
            entity_types: HashMap::new(),
            field_cost_overrides: HashMap::new(),
        }
    }

    #[test]
    fn rejects_deep_nesting_beyond_limit() {
        let mut inner = "id".to_string();
        for i in 0..14 {
            inner = format!("f{i} {{ {inner} }}");
        }
        let q = format!("query Q {{ {inner} }}");
        let doc = parse(&q).unwrap();
        let r = analyze(&doc.document, None, &empty_cat(), 10, 10_000);
        assert!(matches!(r, Err(CostLimitError::Depth { .. })));
    }

    #[test]
    fn rejects_cost_above_limit_with_many_search_fields() {
        let body: String = (0..55)
            .map(|i| format!("  a{i}: searchProperties(city: \"x\") {{ id }}\n"))
            .collect();
        let q = format!("query Q {{\n{body}}}");
        let doc = parse(&q).unwrap();
        let r = analyze(&doc.document, None, &empty_cat(), 100, 1000);
        assert!(matches!(r, Err(CostLimitError::Cost { cost, .. }) if cost > 1000));
    }

    #[test]
    fn cost_override_from_catalog() {
        let mut cat = empty_cat();
        cat.field_cost_overrides
            .insert("Query.searchProperties".into(), 500);
        let q = r#"query Q { searchProperties(city: "A") { id } }"#;
        let doc = parse(q).unwrap();
        let r = analyze(&doc.document, None, &cat, 10, 400);
        assert!(matches!(r, Err(CostLimitError::Cost { .. })));
    }
}
