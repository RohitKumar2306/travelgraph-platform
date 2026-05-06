package com.travelgraph.composer

import com.fasterxml.jackson.dataformat.toml.TomlMapper
import com.fasterxml.jackson.databind.ObjectMapper
import com.fasterxml.jackson.module.kotlin.jacksonObjectMapper
import com.fasterxml.jackson.module.kotlin.readValue
import java.nio.file.Files
import java.nio.file.Path
import kotlin.io.path.absolute
import kotlin.io.path.exists
import kotlin.system.exitProcess

/**
 * `travelgraph-composer` - reads each subgraph's `_service { sdl }`,
 * detects ownership conflicts, and composes a federated supergraph via
 * `@apollo/composition` (called through a small Node helper).
 *
 * Why a Kotlin CLI + Node sidecar?
 *   * Apollo's official composer is JS-only. A pure Kotlin port would mean
 *     re-implementing federation composition - not a maintainable target.
 *   * Wrapping it in a typed Kotlin CLI gives operators a single binary,
 *     consistent argument vocabulary (TOML config, named subgraphs, exit
 *     codes), and lets us run lightweight Kotlin-side checks (e.g.
 *     ownership conflicts) before paying for the Node round-trip.
 *   * Cosmo's composition is in Go and would force a different runtime; this
 *     way we stay aligned with Apollo Federation v2, which the Phase 3 router
 *     plans against.
 *
 * Usage:
 *   travelgraph-composer [--config FILE] [--subgraph name=url ...]
 *                        [--output PATH] [--node BIN] [--helper-root DIR] [-v]
 */
fun main(args: Array<String>) {
    val parsed = parseArgs(args)
    if (parsed.help) {
        printUsage()
        return
    }
    val verbose = parsed.verbose
    fun debug(msg: String) { if (verbose) println("[debug] $msg") }
    fun info(msg: String) = println(msg)

    val subgraphs = resolveSubgraphs(parsed)
    if (subgraphs.isEmpty()) {
        System.err.println("no subgraphs specified - pass --subgraph or supply a --config TOML file")
        exitProcess(64)
    }
    info("composing ${subgraphs.size} subgraphs: ${subgraphs.joinToString { it.name }}")

    val fetcher = SubgraphFetcher()
    val withSdl = subgraphs.map { sg ->
        debug("fetching SDL from ${sg.name} (${sg.url})")
        val sdl = fetcher.fetchSdl(sg.url)
            ?: run {
                System.err.println("failed to fetch _service.sdl from ${sg.name} (${sg.url})")
                exitProcess(70)
            }
        FetchedSubgraph(sg.name, sg.url, sdl)
    }

    // Quick local sanity: ownership conflict detection for top-level operation
    // fields. Apollo composer also catches these but our message points at the
    // offending subgraphs without requiring federation error codes.
    val conflicts = detectOwnershipConflicts(withSdl)
    if (conflicts.isNotEmpty()) {
        System.err.println("composition aborted - top-level field ownership conflicts:")
        conflicts.forEach { (field, owners) ->
            System.err.println("  - field \"$field\" is claimed by: ${owners.joinToString()}")
        }
        exitProcess(2)
    }

    val helperDir = resolveHelperRoot(parsed.helperRoot)
    val output = parsed.output.absolute()
    Files.createDirectories(output.parent ?: Path.of("."))
    val outcome = ComposerRunner(parsed.nodeBin, helperDir).compose(withSdl, output)
    when (outcome) {
        is ComposeOutcome.Success -> {
            println("composition succeeded -> ${outcome.outputPath}")
        }
        is ComposeOutcome.Failure -> {
            System.err.println("composition failed:")
            outcome.errors.forEach { e ->
                System.err.println("  - [${e.code ?: "ERROR"}] ${e.message}")
            }
            exitProcess(2)
        }
    }
}

// ---- argument parsing -----------------------------------------------------

private data class CliArgs(
    val configFile: Path?,
    val inlineSubgraphs: List<String>,
    val output: Path,
    val nodeBin: String,
    val helperRoot: Path?,
    val verbose: Boolean,
    val help: Boolean,
)

private fun parseArgs(args: Array<String>): CliArgs {
    var configFile: Path? = null
    val inlineSubgraphs = mutableListOf<String>()
    var output: Path = Path.of("supergraph.graphql")
    var nodeBin: String = "node"
    var helperRoot: Path? = null
    var verbose = false
    var help = false

    var i = 0
    while (i < args.size) {
        when (val a = args[i]) {
            "--config" -> { configFile = Path.of(requireArg(args, ++i, a)) }
            "--subgraph" -> { inlineSubgraphs += requireArg(args, ++i, a) }
            "--output" -> { output = Path.of(requireArg(args, ++i, a)) }
            "--node" -> { nodeBin = requireArg(args, ++i, a) }
            "--helper-root" -> { helperRoot = Path.of(requireArg(args, ++i, a)) }
            "-v", "--verbose" -> verbose = true
            "-h", "--help" -> help = true
            else -> {
                System.err.println("unknown argument: $a")
                exitProcess(64)
            }
        }
        i++
    }
    return CliArgs(configFile, inlineSubgraphs, output, nodeBin, helperRoot, verbose, help)
}

private fun requireArg(args: Array<String>, idx: Int, flag: String): String {
    if (idx >= args.size) {
        System.err.println("$flag requires a value")
        exitProcess(64)
    }
    return args[idx]
}

private fun printUsage() {
    println(
        """
        travelgraph-composer - compose subgraph SDLs into a federated supergraph.graphql.

        Usage:
          travelgraph-composer [options]

        Options:
          --config FILE            TOML manifest of subgraphs (defaults to ./composer.toml).
          --subgraph name=url      Inline subgraph entry. May be repeated. Overrides --config.
          --output PATH            Output supergraph SDL path (default: ./supergraph.graphql).
          --node BIN               Node executable (default: `node`).
          --helper-root DIR        Directory with compose.mjs + node_modules.
                                   Defaults to TRAVELGRAPH_COMPOSER_HELPER or ./node-helper.
          -v, --verbose            Debug logging.
          -h, --help               This help.
        """.trimIndent()
    )
}

// ---- manifest parsing -----------------------------------------------------

private fun resolveSubgraphs(args: CliArgs): List<SubgraphRef> {
    val cliEntries = args.inlineSubgraphs.mapNotNull { raw ->
        val eq = raw.indexOf('=')
        if (eq <= 0) {
            System.err.println("ignoring malformed --subgraph value: \"$raw\" (expected name=url)")
            null
        } else SubgraphRef(name = raw.substring(0, eq), url = raw.substring(eq + 1))
    }
    if (cliEntries.isNotEmpty()) return cliEntries

    val candidate = args.configFile ?: Path.of("composer.toml").takeIf { it.exists() }
    if (candidate == null) return emptyList()
    if (!candidate.exists()) {
        System.err.println("config file not found: ${candidate.absolute()}")
        exitProcess(66)
    }

    val toml = TomlMapper.builder().build()
    val parsed: ManifestFile = toml.readValue(Files.readAllBytes(candidate))
    return parsed.subgraphs.map { SubgraphRef(it.name, it.url) }
}

private fun resolveHelperRoot(override: Path?): Path {
    override?.let { return it.absolute() }
    System.getenv("TRAVELGRAPH_COMPOSER_HELPER")?.takeIf { it.isNotBlank() }
        ?.let { return Path.of(it).absolute() }
    return Path.of("node-helper").absolute()
}

// ---- shared types ---------------------------------------------------------

internal data class ManifestFile(val subgraphs: List<ManifestEntry> = emptyList())
internal data class ManifestEntry(val name: String = "", val url: String = "")

internal data class SubgraphRef(val name: String, val url: String)
internal data class FetchedSubgraph(val name: String, val url: String, val sdl: String)

internal fun jsonMapper(): ObjectMapper = jacksonObjectMapper()
