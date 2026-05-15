package com.travelgraph.registry.service

import com.travelgraph.registry.api.SchemaIssue
import com.travelgraph.registry.api.UsageBucket
import com.travelgraph.registry.domain.FieldUsageEventRepository
import org.springframework.stereotype.Service
import java.time.OffsetDateTime

@Service
class SchemaCheckService(private val usageRepository: FieldUsageEventRepository) {
    fun lint(sdl: String): List<SchemaIssue> {
        val doc = SdlParser.parse(sdl)
        val issues = mutableListOf<SchemaIssue>()
        for (type in doc.types.values) {
            if (!type.name.matches(Regex("""[A-Z][A-Za-z0-9]*"""))) {
                issues += issue("LINT_TYPE_NAMING", "Type names must be PascalCase.", type.name)
            }
            if (!type.description && !type.name.startsWith("_")) {
                issues += issue("LINT_DESCRIPTION", "Type ${type.name} is missing a description.", type.name)
            }
            if (type.kind == "type" && type.fields.containsKey("id") && !type.hasKey && !type.name.endsWith("Payload")) {
                issues += issue("LINT_ENTITY_KEY", "Entity-like type ${type.name} has id but no @key.", type.name)
            }
            for (field in type.fields.values) {
                if (!field.name.matches(Regex("""[_a-z][_A-Za-z0-9]*"""))) {
                    issues += issue("LINT_FIELD_NAMING", "Field ${type.name}.${field.name} must be camelCase.", type.name, field.name)
                }
                if (!field.description && type.name !in setOf("Query", "Mutation")) {
                    issues += issue("LINT_DESCRIPTION", "Field ${type.name}.${field.name} is missing a description.", type.name, field.name)
                }
            }
        }
        doc.types["Mutation"]?.fields?.values?.forEach { mutation ->
            val payloadType = mutation.type.removeSurrounding("[", "]").removeSuffix("!")
            val union = doc.unions[payloadType]
            if (union == null || !payloadType.endsWith("Payload")) {
                issues += issue("LINT_MUTATION_PAYLOAD", "Mutation ${mutation.name} must return a payload union ending in Payload.", "Mutation", mutation.name)
            }
        }
        return issues
    }

    fun breakingChanges(serviceName: String, oldSdl: String?, newSdl: String): List<SchemaIssue> {
        if (oldSdl.isNullOrBlank()) return emptyList()
        val oldDoc = SdlParser.parse(oldSdl)
        val newDoc = SdlParser.parse(newSdl)
        val issues = mutableListOf<SchemaIssue>()
        val since = OffsetDateTime.now().minusDays(30)

        for ((typeName, oldType) in oldDoc.types) {
            val newType = newDoc.types[typeName]
            if (newType == null) {
                issues += withUsage(serviceName, issue("BREAKING_TYPE_REMOVED", "Type $typeName was removed.", typeName), since)
                continue
            }
            for ((fieldName, oldField) in oldType.fields) {
                val newField = newType.fields[fieldName]
                if (newField == null) {
                    issues += withUsage(serviceName, issue("BREAKING_FIELD_REMOVED", "Field $typeName.$fieldName was removed.", typeName, fieldName), since)
                    continue
                }
                if (oldField.type != newField.type) {
                    issues += issue("BREAKING_FIELD_TYPE_CHANGED", "Field $typeName.$fieldName changed type from ${oldField.type} to ${newField.type}.", typeName, fieldName)
                }
                if (!oldField.required && newField.required) {
                    issues += issue("BREAKING_OPTIONAL_TO_REQUIRED", "Field $typeName.$fieldName changed from optional to required.", typeName, fieldName)
                }
            }
        }
        for ((enumName, oldEnum) in oldDoc.enums) {
            val removed = oldEnum.values - (newDoc.enums[enumName]?.values ?: emptySet())
            removed.forEach { value ->
                issues += issue("BREAKING_ENUM_VALUE_REMOVED", "Enum value $enumName.$value was removed.", enumName, value)
            }
        }
        oldDoc.types["Mutation"]?.fields?.keys.orEmpty()
            .minus(newDoc.types["Mutation"]?.fields?.keys.orEmpty())
            .forEach { field ->
                issues += issue("BREAKING_MUTATION_REMOVED", "Mutation $field was removed.", "Mutation", field)
            }
        return issues
    }

    private fun withUsage(serviceName: String, issue: SchemaIssue, since: OffsetDateTime): SchemaIssue {
        val typeName = issue.typeName ?: return issue
        val fieldName = issue.fieldName ?: return issue
        val usage = usageRepository.usageByClient(serviceName, typeName, fieldName, since)
            .map { UsageBucket(it.clientName, it.clientVersion, it.count) }
        return if (usage.isEmpty()) issue else issue.copy(
            severity = "error",
            message = "${issue.message} Recent usage exists in the last 30 days.",
            usageByClient = usage
        )
    }

    private fun issue(code: String, message: String, typeName: String? = null, fieldName: String? = null) =
        SchemaIssue("warning", code, message, typeName, fieldName)
}
