package com.travelgraph.composer

/**
 * A small, schema-aware detector that surfaces top-level field ownership
 * conflicts (two subgraphs both declaring `Query.searchProperties`, etc.)
 * before invoking the composer.
 *
 * Apollo composition will catch this too, but its error code (e.g.
 * `INVALID_FIELD_SHARING`) is opaque without context. By failing here with
 * an explicit "claimed by [a, b]" message we save the operator a step.
 *
 * Heuristic: we scan each subgraph's SDL for blocks declaring `type Query`
 * or `extend type Query` (and the same for `Mutation`) and collect every
 * top-level field name. Object types that re-declare federation entities
 * with `@key` are not flagged - those are *expected* to repeat across
 * subgraphs.
 */
internal fun detectOwnershipConflicts(
    subgraphs: List<FetchedSubgraph>
): List<Pair<String, List<String>>> {
    val claims = mutableMapOf<String, MutableList<String>>()
    for (s in subgraphs) {
        val rootFields = extractRootFields(s.sdl)
        for (field in rootFields) {
            claims.getOrPut(field) { mutableListOf() }.add(s.name)
        }
    }
    return claims
        .filter { (_, owners) -> owners.distinct().size > 1 }
        .map { (field, owners) -> field to owners.distinct() }
}

/**
 * Returns root fields declared by this subgraph, prefixed with their root
 * type, e.g. "Query.searchProperties". The `_service` and `_entities`
 * federation fields are always skipped.
 */
private fun extractRootFields(sdl: String): List<String> {
    val out = mutableListOf<String>()
    listOf("Query", "Mutation").forEach { rootType ->
        val blocks = findRootBlocks(sdl, rootType)
        for (body in blocks) {
            for (line in body.lineSequence()) {
                val trimmed = line.trim().takeIf { it.isNotEmpty() } ?: continue
                if (trimmed.startsWith("#") || trimmed.startsWith("\"")) continue
                // Field declarations look like `fieldName(args): Type` or `fieldName: Type`.
                val name = trimmed.substringBefore('(').substringBefore(':').trim()
                if (name.isEmpty()) continue
                if (name == "_service" || name == "_entities") continue
                if (!name.first().isLetter() && name.first() != '_') continue
                out += "$rootType.$name"
            }
        }
    }
    return out
}

private fun findRootBlocks(sdl: String, rootType: String): List<String> {
    val out = mutableListOf<String>()
    val patterns = listOf(
        Regex("(?m)^(?:extend\\s+)?type\\s+$rootType\\b[^{]*\\{"),
    )
    patterns.forEach { regex ->
        var search = sdl
        var offset = 0
        while (true) {
            val m = regex.find(search) ?: break
            val open = m.range.last
            // Find matching close brace.
            var depth = 1
            var i = open + 1
            while (i < search.length && depth > 0) {
                when (search[i]) {
                    '{' -> depth++
                    '}' -> depth--
                }
                i++
            }
            if (depth == 0) {
                out += search.substring(open + 1, i - 1)
                offset += i
                search = search.substring(i)
            } else break
        }
    }
    return out
}
