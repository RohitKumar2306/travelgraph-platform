package com.travelgraph.registry.api

import com.fasterxml.jackson.annotation.JsonAlias
import java.time.OffsetDateTime

data class PublishSchemaRequest(
    @JsonAlias("service_name")
    val serviceName: String,
    val version: String,
    val sdl: String,
    @JsonAlias("owner_team")
    val ownerTeam: String
)

data class SchemaResponse(
    val serviceName: String,
    val version: String,
    val ownerTeam: String,
    val sdl: String,
    val createdAt: OffsetDateTime
)

data class ComposeResponse(
    val ok: Boolean,
    val supergraph: String? = null,
    val errors: List<String> = emptyList()
)

data class SchemaCheckRequest(val sdl: String)

data class CheckResponse(
    val lintIssues: List<SchemaIssue>,
    val breakingChanges: List<SchemaIssue>
)

data class SchemaIssue(
    val severity: String,
    val code: String,
    val message: String,
    val typeName: String? = null,
    val fieldName: String? = null,
    val usageByClient: List<UsageBucket> = emptyList()
)

data class UsageEventRequest(
    @JsonAlias("service_name")
    val serviceName: String,
    @JsonAlias("type_name")
    val typeName: String,
    @JsonAlias("field_name")
    val fieldName: String,
    @JsonAlias("field_path")
    val fieldPath: String,
    @JsonAlias("operation_name")
    val operationName: String,
    @JsonAlias("client_name")
    val clientName: String,
    @JsonAlias("client_version")
    val clientVersion: String,
    val timestamp: OffsetDateTime? = null
)

data class UsageBucket(
    val clientName: String,
    val clientVersion: String,
    val count: Long
)

data class UsageResponse(
    val serviceName: String,
    val typeName: String,
    val fieldName: String,
    val since: OffsetDateTime,
    val clients: List<UsageBucket>
)

data class AiReviewRequest(
    @JsonAlias("old_schema")
    val oldSchema: String,
    @JsonAlias("new_schema")
    val newSchema: String,
    @JsonAlias("service_name")
    val serviceName: String,
    @JsonAlias("owner_team")
    val ownerTeam: String
)

data class AiReviewResponse(val markdown: String)
