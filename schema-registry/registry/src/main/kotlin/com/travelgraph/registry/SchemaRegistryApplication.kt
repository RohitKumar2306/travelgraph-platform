package com.travelgraph.registry

import org.springframework.boot.autoconfigure.SpringBootApplication
import org.springframework.boot.context.properties.ConfigurationProperties
import org.springframework.boot.context.properties.EnableConfigurationProperties
import org.springframework.boot.runApplication

@SpringBootApplication
@EnableConfigurationProperties(RegistryProperties::class)
class SchemaRegistryApplication

fun main(args: Array<String>) {
    runApplication<SchemaRegistryApplication>(*args)
}

@ConfigurationProperties("travelgraph")
data class RegistryProperties(
    val composer: ComposerProperties = ComposerProperties(),
    val ai: AiProperties = AiProperties()
)

data class ComposerProperties(
    val nodeBin: String = "node",
    val helperRoot: String = "/app/node-helper"
)

data class AiProperties(
    val assistantUrl: String = "http://ai-schema-assistant:8091"
)
