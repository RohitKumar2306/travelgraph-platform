use apollo_compiler::ast::{Definition, Document, Selection};
use serde::Serialize;
use std::collections::HashMap;

use crate::plan::ExecutionPlan;
use crate::supergraph::SupergraphCatalog;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageEvent {
    pub service_name: String,
    pub type_name: String,
    pub field_name: String,
    pub field_path: String,
    pub operation_name: String,
    pub client_name: String,
    pub client_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
}

pub fn collect_usage_events(
    document: &Document,
    plan: &ExecutionPlan,
    catalog: &SupergraphCatalog,
    operation_name: &str,
    client_name: &str,
    client_version: &str,
) -> Vec<UsageEvent> {
    let top_level_entity: HashMap<&str, &str> = plan
        .field_fetches
        .iter()
        .filter_map(|fetch| {
            fetch
                .entity_type
                .as_deref()
                .map(|type_name| (fetch.response_key.as_str(), type_name))
        })
        .collect();
    let mut events = Vec::new();
    for def in &document.definitions {
        let Definition::OperationDefinition(op) = def else { continue };
        if let Some(requested) = plan.operation_name.as_deref() {
            if op.name.as_ref().map(|n| n.as_str()) != Some(requested) {
                continue;
            }
        }
        for selection in &op.selection_set {
            let Selection::Field(field) = selection else { continue };
            let response_key = field.alias.as_ref().unwrap_or(&field.name).as_str();
            let Some(type_name) = top_level_entity.get(response_key).copied() else {
                continue;
            };
            collect_entity_fields(
                type_name,
                response_key,
                &field.selection_set,
                catalog,
                operation_name,
                client_name,
                client_version,
                &mut events,
            );
        }
    }
    events
}

fn collect_entity_fields(
    type_name: &str,
    path: &str,
    selections: &[Selection],
    catalog: &SupergraphCatalog,
    operation_name: &str,
    client_name: &str,
    client_version: &str,
    events: &mut Vec<UsageEvent>,
) {
    let Some(entity) = catalog.entity_types.get(type_name) else {
        return;
    };
    for selection in selections {
        let Selection::Field(field) = selection else { continue };
        let field_name = field.name.as_str();
        if field_name == "__typename" {
            continue;
        }
        if let Some(owner) = entity.field_owners.get(field_name) {
            events.push(UsageEvent {
                service_name: format!("{owner}-service"),
                type_name: type_name.to_string(),
                field_name: field_name.to_string(),
                field_path: format!("{path}.{field_name}"),
                operation_name: operation_name.to_string(),
                client_name: client_name.to_string(),
                client_version: client_version.to_string(),
                timestamp: None,
            });
        }
    }
}
