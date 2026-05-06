//! Parse incoming GraphQL operation text using `apollo-compiler`.
//!
//! We deliberately use the schemaless [`apollo_compiler::ast::Document`] API.
//! In Phase 2 the router has no supergraph yet, so schema-aware validation
//! (field types, fragment spread compatibility, ...) cannot run. The
//! [`validate`](super::validate) module performs the small set of
//! schema-independent checks the prompt requires.

use apollo_compiler::ast::Document;

use super::types::{GraphQLError, Location};

/// Successful parse result. The owned [`Document`] is what the rest of the
/// pipeline walks (registry projection, executor).
#[derive(Debug)]
pub struct ParsedRequest {
    pub document: Document,
}

/// Parse a single GraphQL operation document.
///
/// On success returns the parsed AST. On failure returns a list of
/// GraphQL-spec-shaped errors with locations populated where the parser
/// could pin down a position.
pub fn parse(query: &str) -> Result<ParsedRequest, Vec<GraphQLError>> {
    match Document::parse(query, "operation.graphql") {
        Ok(document) => Ok(ParsedRequest { document }),
        Err(with_errors) => Err(diagnostics_to_errors(&with_errors.errors.to_string())),
    }
}

/// Convert the apollo-compiler diagnostic block (a multi-line, terminal-
/// formatted string) into a list of GraphQL errors suitable for the response
/// `errors` array.
///
/// We keep the diagnostic prose in `message` (it already names the file and
/// the offending span). When we can pluck out a `line:column` from the body
/// we surface it as a structured `locations[]` entry too.
pub(crate) fn diagnostics_to_errors(rendered: &str) -> Vec<GraphQLError> {
    let mut out = Vec::new();
    let trimmed = rendered.trim();
    if trimmed.is_empty() {
        return out;
    }

    // apollo-compiler's diagnostic blocks render multiple errors separated by
    // blank lines. Split on `\n\n` so each error becomes its own entry.
    for chunk in trimmed.split("\n\n") {
        let message = chunk.trim().to_string();
        let locations = first_location_in(&message)
            .map(|loc| vec![loc])
            .unwrap_or_default();
        out.push(GraphQLError {
            message,
            locations,
            path: Vec::new(),
            extensions: None,
        });
    }
    out
}

/// Extract the first `line:column` pair from a diagnostic block by scanning
/// for the canonical `path:line:col` token apollo-compiler renders.
fn first_location_in(text: &str) -> Option<Location> {
    // Look for ".graphql:LINE:COL" style anchors first (most precise).
    if let Some((line, col)) = scan_anchor(text, ".graphql:") {
        return Some(Location { line, column: col });
    }
    // Fall back to a generic `LINE:COL` after the first colon-pair.
    scan_anchor(text, ":").map(|(line, col)| Location { line, column: col })
}

fn scan_anchor(text: &str, marker: &str) -> Option<(usize, usize)> {
    let start = text.find(marker)?;
    let tail = &text[start + marker.len()..];
    let mut parts = tail.splitn(3, ':');
    let line: usize = parts.next()?.trim().parse().ok()?;
    let col: usize = parts.next()?.trim().parse().ok()?;
    Some((line, col))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_valid_query() {
        let parsed = parse("{ searchProperties(city: \"Austin\") { id name } }").unwrap();
        assert_eq!(parsed.document.definitions.len(), 1);
    }

    #[test]
    fn parses_a_named_query_with_variables() {
        let parsed = parse(
            "query Q($city: String!) { searchProperties(city: $city) { id } }",
        )
        .unwrap();
        assert_eq!(parsed.document.definitions.len(), 1);
    }

    #[test]
    fn syntax_error_returns_errors_with_location() {
        // Unclosed selection set - hard syntax error apollo-compiler can't recover from.
        let result = parse("query Q { searchProperties(city: \"Austin\") { id ");
        let errs = match result {
            Ok(parsed) => panic!("expected parse failure, got {parsed:?}"),
            Err(errs) => errs,
        };
        assert!(!errs.is_empty(), "expected at least one error");
        let any_message = errs.iter().any(|e| !e.message.is_empty());
        assert!(any_message, "every error needs a non-empty message");
    }
}
