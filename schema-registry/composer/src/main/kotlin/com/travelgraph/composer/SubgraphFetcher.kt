package com.travelgraph.composer

import com.fasterxml.jackson.module.kotlin.readValue
import java.net.URI
import java.net.http.HttpClient
import java.net.http.HttpRequest
import java.net.http.HttpResponse
import java.time.Duration

/**
 * Fetches `_service { sdl }` from a subgraph over HTTP. Uses the JDK
 * `HttpClient` so the composer has zero runtime deps beyond the JRE.
 */
internal class SubgraphFetcher(
    private val timeout: Duration = Duration.ofSeconds(5),
) {
    private val http: HttpClient = HttpClient.newBuilder().connectTimeout(timeout).build()
    private val mapper = jsonMapper()

    fun fetchSdl(url: String): String? {
        val body = mapper.writeValueAsString(mapOf("query" to "{ _service { sdl } }"))
        val req = HttpRequest.newBuilder(URI.create(url))
            .POST(HttpRequest.BodyPublishers.ofString(body))
            .timeout(timeout)
            .header("content-type", "application/json")
            .header("accept", "application/json")
            .build()
        val resp = runCatching { http.send(req, HttpResponse.BodyHandlers.ofString()) }.getOrElse { e ->
            System.err.println("subgraph fetch error ($url): ${e.message}")
            return null
        }
        if (resp.statusCode() / 100 != 2) {
            System.err.println("subgraph fetch ($url) returned HTTP ${resp.statusCode()}: ${resp.body().take(200)}")
            return null
        }
        val payload: GraphQLResponse = mapper.readValue(resp.body())
        if (payload.errors?.isNotEmpty() == true) {
            System.err.println("subgraph $url returned GraphQL errors: ${payload.errors.joinToString { it.message }}")
            return null
        }
        return payload.data?.service?.sdl
    }
}

internal data class GraphQLResponse(
    val data: ServiceWrapper? = null,
    val errors: List<GraphQLErr>? = null,
)

internal data class ServiceWrapper(@com.fasterxml.jackson.annotation.JsonProperty("_service") val service: ServiceField? = null)
internal data class ServiceField(val sdl: String? = null)
internal data class GraphQLErr(val message: String = "")
