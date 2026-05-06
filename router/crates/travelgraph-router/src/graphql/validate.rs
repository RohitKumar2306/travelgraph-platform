//! Schema-less validation of executable GraphQL documents.
//!
//! These are the checks the GraphQL spec lets us perform without knowing the
//! types involved. They cover the Phase 2.2 requirements:
//!
//! * the document contains at least one operation,
//! * no anonymous operation appears alongside others
//!   (spec rule 5.2.2.1 "Lone Anonymous Operation"),
//! * every variable referenced is declared on its operation (rule 5.8.3
//!   "Variables Are Defined").
//!
//! `apollo_compiler::ast::Document::validate_standalone_executable` covers
//! most of these, so we delegate to it and additionally walk the AST to add
//! a friendlier "undefined variable" message that carries the variable name.

use apollo_compiler::ast::{Definition, Document, OperationDefinition, Selection, Value};
use std::collections::HashSet;

use super::parse::diagnostics_to_errors;
use super::types::GraphQLError;

/// Validate an already-parsed document. Returns the empty Vec on success,
/// otherwise a non-empty list of GraphQL-spec-shaped errors that the caller
/// should return with HTTP 200 (per the GraphQL HTTP spec, validation errors
/// are still a successful HTTP response).
pub fn validate(document: &Document) -> Vec<GraphQLError> {
    let mut errors = Vec::new();

    if let Err(diagnostics) = document.validate_standalone_executable() {
        errors.extend(diagnostics_to_errors(&diagnostics.to_string()));
    }

    let operation_count = document
        .definitions
        .iter()
        .filter(|d| matches!(d, Definition::OperationDefinition(_)))
        .count();
    if operation_count == 0 {
        errors.push(GraphQLError::message(
            "Document must contain at least one operation.",
        ));
    }

    // Schema-less variable-usage check. apollo-compiler's standalone validator
    // already covers this, but we add an explicit walk so the error has a
    // stable, test-friendly message even if upstream wording shifts.
    for def in &document.definitions {
        if let Definition::OperationDefinition(op) = def {
            for var_used in collect_variable_uses(op) {
                let declared = op.variables.iter().any(|v| v.name.as_str() == var_used);
                if !declared {
                    let op_label = op.name.as_ref().map(|n| n.as_str()).unwrap_or("<anonymous>");
                    errors.push(GraphQLError::message(format!(
                        "Variable \"${var_used}\" is not declared on operation \"{op_label}\"."
                    )));
                }
            }
        }
    }

    // De-duplicate identical messages so apollo-compiler's diagnostic and our
    // friendlier wording for the same problem don't both show up.
    let mut seen: HashSet<String> = HashSet::new();
    errors.retain(|e| seen.insert(e.message.clone()));
    errors
}

/// Walk an operation's selection set collecting every `$variable` referenced
/// in argument values (recursively into objects, lists, and nested fields).
fn collect_variable_uses(op: &OperationDefinition) -> HashSet<String> {
    let mut out = HashSet::new();
    walk_selections(&op.selection_set, &mut out);
    out
}

fn walk_selections(selections: &[Selection], out: &mut HashSet<String>) {
    for sel in selections {
        match sel {
            Selection::Field(field) => {
                for arg in &field.arguments {
                    walk_value(&arg.value, out);
                }
                walk_selections(&field.selection_set, out);
            }
            Selection::InlineFragment(frag) => walk_selections(&frag.selection_set, out),
            Selection::FragmentSpread(_) => {
                // Fragment definitions are not inspected here; the standalone
                // validator catches undefined-variable usage inside spreads.
            }
        }
    }
}

fn walk_value(value: &Value, out: &mut HashSet<String>) {
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
    use super::super::parse::parse;
    use super::*;

    #[test]
    fn valid_query_has_no_errors() {
        let parsed = parse("{ searchProperties(city: \"Austin\") { id name } }").unwrap();
        let errs = validate(&parsed.document);
        assert!(errs.is_empty(), "unexpected errors: {errs:?}");
    }

    #[test]
    fn undeclared_variable_is_rejected() {
        let parsed = parse("query Q { searchProperties(city: $city) { id } }").unwrap();
        let errs = validate(&parsed.document);
        assert!(!errs.is_empty(), "expected at least one error");
        let mentions_city = errs.iter().any(|e| e.message.contains("$city"));
        assert!(mentions_city, "errors should mention the offending variable: {errs:?}");
    }

    #[test]
    fn anonymous_with_named_operations_is_rejected() {
        let parsed = parse("{ a } query Named { b }").unwrap();
        let errs = validate(&parsed.document);
        assert!(!errs.is_empty(), "anonymous + named must fail validation");
    }
}
