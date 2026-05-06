package com.travelgraph.composer

import com.fasterxml.jackson.module.kotlin.readValue
import java.nio.file.Files
import java.nio.file.Path
import java.util.concurrent.TimeUnit

/**
 * Invokes `node compose.mjs` with the fetched SDLs and parses the JSON
 * response.
 *
 * The Node script is shipped at `<helperRoot>/compose.mjs` together with a
 * pre-installed `node_modules/`. The Dockerfile builds this directory once
 * during image build; for local runs the operator can `npm install` inside
 * `schema-registry/composer/src/main/resources` and pass `--helper-root`.
 */
internal class ComposerRunner(
    private val nodeBin: String,
    private val helperRoot: Path,
) {
    private val mapper = jsonMapper()

    fun compose(subgraphs: List<FetchedSubgraph>, output: Path): ComposeOutcome {
        val script = helperRoot.resolve("compose.mjs")
        require(Files.isRegularFile(script)) {
            "compose.mjs not found at $script - either pass --helper-root or " +
                "run `npm install` inside schema-registry/composer/src/main/resources " +
                "and copy compose.mjs + node_modules into a node-helper directory."
        }

        val tmp = Files.createTempFile("subgraphs", ".json")
        try {
            val payload = subgraphs.map { mapOf("name" to it.name, "url" to it.url, "sdl" to it.sdl) }
            Files.write(tmp, mapper.writeValueAsBytes(payload))

            val process = ProcessBuilder(
                nodeBin,
                script.toString(),
                tmp.toString(),
                output.toString()
            )
                .redirectErrorStream(false)
                .start()

            val finished = process.waitFor(60, TimeUnit.SECONDS)
            if (!finished) {
                process.destroyForcibly()
                return ComposeOutcome.Failure(
                    listOf(ComposeError("compose.mjs timed out after 60s", null))
                )
            }

            val stdout = process.inputStream.bufferedReader().readText()
            val stderr = process.errorStream.bufferedReader().readText()
            if (stderr.isNotBlank()) System.err.println(stderr)

            return when (process.exitValue()) {
                0 -> ComposeOutcome.Success(output)
                2 -> {
                    val parsed: ComposerErrorPayload = mapper.readValue(stdout)
                    ComposeOutcome.Failure(parsed.errors)
                }
                else -> ComposeOutcome.Failure(
                    listOf(ComposeError("compose.mjs exited with code ${process.exitValue()}: ${stdout.trim()}", null))
                )
            }
        } finally {
            runCatching { Files.deleteIfExists(tmp) }
        }
    }
}

internal sealed interface ComposeOutcome {
    data class Success(val outputPath: Path) : ComposeOutcome
    data class Failure(val errors: List<ComposeError>) : ComposeOutcome
}

internal data class ComposerErrorPayload(
    val ok: Boolean = false,
    val errors: List<ComposeError> = emptyList(),
)

internal data class ComposeError(
    val message: String = "",
    val code: String? = null,
)
