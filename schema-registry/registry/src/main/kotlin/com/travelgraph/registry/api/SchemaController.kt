package com.travelgraph.registry.api

import com.travelgraph.registry.domain.FieldUsageEvent
import com.travelgraph.registry.domain.FieldUsageEventRepository
import com.travelgraph.registry.domain.SchemaVersion
import com.travelgraph.registry.domain.SchemaVersionRepository
import com.travelgraph.registry.domain.SupergraphSnapshotRepository
import com.travelgraph.registry.service.AiReviewClient
import com.travelgraph.registry.service.SchemaCheckService
import com.travelgraph.registry.service.SupergraphService
import org.springframework.http.HttpStatus
import org.springframework.http.MediaType
import org.springframework.web.bind.annotation.GetMapping
import org.springframework.web.bind.annotation.PathVariable
import org.springframework.web.bind.annotation.PostMapping
import org.springframework.web.bind.annotation.RequestBody
import org.springframework.web.bind.annotation.ResponseStatus
import org.springframework.web.bind.annotation.RestController
import org.springframework.web.server.ResponseStatusException
import java.time.OffsetDateTime

@RestController
class SchemaController(
    private val schemas: SchemaVersionRepository,
    private val supergraphs: SupergraphSnapshotRepository,
    private val usage: FieldUsageEventRepository,
    private val supergraphService: SupergraphService,
    private val checks: SchemaCheckService,
    private val ai: AiReviewClient
) {
    @PostMapping("/schemas")
    @ResponseStatus(HttpStatus.CREATED)
    fun publish(@RequestBody request: PublishSchemaRequest): SchemaResponse {
        val saved = schemas.save(
            SchemaVersion(
                serviceName = request.serviceName,
                version = request.version,
                ownerTeam = request.ownerTeam,
                sdl = request.sdl
            )
        )
        return saved.toResponse()
    }

    @GetMapping("/schemas/{service}")
    fun latest(@PathVariable service: String): SchemaResponse =
        schemas.findFirstByServiceNameOrderByCreatedAtDesc(service)?.toResponse()
            ?: throw ResponseStatusException(HttpStatus.NOT_FOUND, "No schema published for $service")

    @GetMapping("/schemas/{service}/versions")
    fun versions(@PathVariable service: String): List<SchemaResponse> =
        schemas.findAllByServiceNameOrderByCreatedAtDesc(service).map { it.toResponse() }

    @PostMapping("/schemas/{service}/check")
    fun check(@PathVariable service: String, @RequestBody request: SchemaCheckRequest): CheckResponse {
        val previous = schemas.findFirstByServiceNameOrderByCreatedAtDesc(service)
        return CheckResponse(
            lintIssues = checks.lint(request.sdl),
            breakingChanges = checks.breakingChanges(service, previous?.sdl, request.sdl)
        )
    }

    @PostMapping("/schemas/{service}/ai-review")
    fun aiReview(@PathVariable service: String, @RequestBody request: SchemaCheckRequest): AiReviewResponse =
        ai.review(schemas.findFirstByServiceNameOrderByCreatedAtDesc(service), request.sdl)

    @GetMapping("/supergraph/latest", produces = [MediaType.TEXT_PLAIN_VALUE])
    fun latestSupergraph(): String =
        supergraphs.findFirstByOrderByCreatedAtDesc()?.sdl
            ?: throw ResponseStatusException(HttpStatus.NOT_FOUND, "No supergraph snapshot has been composed.")

    @PostMapping("/supergraph/compose")
    fun compose(): ComposeResponse = supergraphService.composeLatest()

    @PostMapping("/usage")
    @ResponseStatus(HttpStatus.ACCEPTED)
    fun usage(@RequestBody events: List<UsageEventRequest>) {
        usage.saveAll(events.map {
            FieldUsageEvent(
                serviceName = it.serviceName,
                typeName = it.typeName,
                fieldName = it.fieldName,
                fieldPath = it.fieldPath,
                operationName = it.operationName,
                clientName = it.clientName,
                clientVersion = it.clientVersion,
                occurredAt = it.timestamp ?: OffsetDateTime.now()
            )
        })
    }

    @GetMapping("/usage/{service}/{type}/{field}")
    fun usageByClient(
        @PathVariable service: String,
        @PathVariable type: String,
        @PathVariable field: String
    ): UsageResponse {
        val since = OffsetDateTime.now().minusDays(30)
        return UsageResponse(
            serviceName = service,
            typeName = type,
            fieldName = field,
            since = since,
            clients = usage.usageByClient(service, type, field, since)
                .map { UsageBucket(it.clientName, it.clientVersion, it.count) }
        )
    }

    private fun SchemaVersion.toResponse() = SchemaResponse(serviceName, version, ownerTeam, sdl, createdAt)
}
