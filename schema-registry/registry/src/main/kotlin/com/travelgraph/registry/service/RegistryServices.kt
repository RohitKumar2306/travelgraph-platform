package com.travelgraph.registry.service

import com.fasterxml.jackson.databind.ObjectMapper
import com.travelgraph.registry.RegistryProperties
import com.travelgraph.registry.api.AiReviewRequest
import com.travelgraph.registry.api.AiReviewResponse
import com.travelgraph.registry.api.ComposeResponse
import com.travelgraph.registry.domain.FieldUsageEvent
import com.travelgraph.registry.domain.FieldUsageEventRepository
import com.travelgraph.registry.domain.SchemaVersion
import com.travelgraph.registry.domain.SchemaVersionRepository
import com.travelgraph.registry.domain.SupergraphSnapshot
import com.travelgraph.registry.domain.SupergraphSnapshotRepository
import org.springframework.http.MediaType
import org.springframework.stereotype.Service
import org.springframework.web.client.RestClient
import java.nio.file.Files
import java.time.OffsetDateTime

@Service
class SupergraphService(
    private val schemas: SchemaVersionRepository,
    private val snapshots: SupergraphSnapshotRepository,
    private val properties: RegistryProperties,
    private val mapper: ObjectMapper
) {
    fun composeLatest(): ComposeResponse {
        val latest = schemas.latestForEveryService().sortedBy { it.serviceName }
        if (latest.isEmpty()) return ComposeResponse(false, errors = listOf("No schemas have been published."))
        val input = Files.createTempFile("travelgraph-subgraphs", ".json")
        val output = Files.createTempFile("travelgraph-supergraph", ".graphql")
        return try {
            val payload = latest.map {
                val serviceName = it.serviceName
                mapOf(
                    "name" to serviceName.removeSuffix("-service").substringBefore("-"),
                    "url" to "http://$serviceName:${portFor(serviceName)}/graphql",
                    "sdl" to it.sdl
                )
            }
            Files.write(input, mapper.writeValueAsBytes(payload))
            val process = ProcessBuilder(
                properties.composer.nodeBin,
                "${properties.composer.helperRoot}/compose.mjs",
                input.toString(),
                output.toString()
            ).redirectErrorStream(false).start()
            if (!process.waitFor(60, java.util.concurrent.TimeUnit.SECONDS)) {
                process.destroyForcibly()
                return ComposeResponse(false, errors = listOf("Composition timed out."))
            }
            val stdout = process.inputStream.bufferedReader().readText()
            val stderr = process.errorStream.bufferedReader().readText()
            if (process.exitValue() != 0) {
                return ComposeResponse(false, errors = listOf(stdout.ifBlank { stderr }.trim()))
            }
            val sdl = Files.readString(output)
            snapshots.save(SupergraphSnapshot(sdl = sdl))
            ComposeResponse(true, supergraph = sdl)
        } finally {
            Files.deleteIfExists(input)
            Files.deleteIfExists(output)
        }
    }

    private fun portFor(serviceName: String): Int = when (serviceName) {
        "property-service" -> 8081
        "pricing-service" -> 8082
        "booking-service" -> 8083
        "user-service" -> 8084
        "review-service" -> 8085
        else -> 8080
    }
}

@Service
class UsageService(private val usage: FieldUsageEventRepository) {
    fun record(events: List<FieldUsageEvent>) {
        usage.saveAll(events)
    }

    fun byClient(serviceName: String, typeName: String, fieldName: String, since: OffsetDateTime) =
        usage.usageByClient(serviceName, typeName, fieldName, since)
}

@Service
class AiReviewClient(private val properties: RegistryProperties) {
    private val rest = RestClient.builder().baseUrl(properties.ai.assistantUrl).build()

    fun review(service: SchemaVersion?, proposedSdl: String): AiReviewResponse =
        runCatching {
            rest.post()
                .uri("/review")
                .contentType(MediaType.APPLICATION_JSON)
                .body(
                    AiReviewRequest(
                        oldSchema = service?.sdl ?: "",
                        newSchema = proposedSdl,
                        serviceName = service?.serviceName ?: "unknown",
                        ownerTeam = service?.ownerTeam ?: "unknown"
                    )
                )
                .retrieve()
                .body(AiReviewResponse::class.java)
                ?: AiReviewResponse("AI review unavailable")
        }.getOrElse {
            AiReviewResponse("AI review unavailable")
        }
}
