package com.travelgraph.review.schema

import com.fasterxml.jackson.databind.JsonNode
import org.springframework.beans.factory.annotation.Value
import org.springframework.boot.context.event.ApplicationReadyEvent
import org.springframework.context.event.EventListener
import org.springframework.stereotype.Component
import org.springframework.web.reactive.function.client.WebClient
import java.time.Instant
import javax.crypto.Mac
import javax.crypto.spec.SecretKeySpec

@Component
class SchemaPublisher(
    @Value("\${server.port}") private val port: Int,
    @Value("\${SCHEMA_REGISTRY_URL:http://schema-registry:8090}") private val registryUrl: String,
    @Value("\${SCHEMA_OWNER_TEAM:review-platform}") private val ownerTeam: String,
    @Value("\${travelgraph.identity.signature-secret:\${TRAVELGRAPH_IDENTITY_SECRET:travelgraph-dev-identity-secret}}") private val signatureSecret: String
) {
    private val webClient = WebClient.create()

    @EventListener(ApplicationReadyEvent::class)
    fun publish() {
        repeat(30) { attempt ->
            val published = runCatching {
                val sdl = webClient.post()
                    .uri("http://localhost:$port/graphql")
                    .header("content-type", "application/json")
                    .headers { signedHeaders().forEach(it::set) }
                    .bodyValue(mapOf("query" to "{ _service { sdl } }"))
                    .retrieve()
                    .bodyToMono(JsonNode::class.java)
                    .block()
                    ?.at("/data/_service/sdl")
                    ?.asText()
                    ?: return@runCatching false
                webClient.post()
                    .uri("$registryUrl/schemas")
                    .bodyValue(mapOf("serviceName" to "review-service", "version" to Instant.now().toString(), "ownerTeam" to ownerTeam, "sdl" to sdl))
                    .retrieve()
                    .toBodilessEntity()
                    .block()
                true
            }.getOrElse {
                if (attempt == 29) println("schema publication failed for review-service: ${it.message}")
                false
            }
            if (published) return
            Thread.sleep(1_000)
        }
    }

    private fun signedHeaders(): Map<String, String> {
        val mac = Mac.getInstance("HmacSHA256")
        mac.init(SecretKeySpec(signatureSecret.toByteArray(), "HmacSHA256"))
        val signature = mac.doFinal("anonymous\nanonymous\n".toByteArray()).joinToString("") { "%02x".format(it.toInt() and 0xff) }
        return mapOf("x-user-id" to "anonymous", "x-user-tier" to "anonymous", "x-user-email" to "", "x-identity-signature" to signature)
    }
}
