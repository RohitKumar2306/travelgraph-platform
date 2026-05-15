package com.travelgraph.registry.service

data class SdlDocument(
    val types: Map<String, TypeDef>,
    val enums: Map<String, EnumDef>,
    val unions: Map<String, UnionDef>
)

data class TypeDef(
    val name: String,
    val kind: String,
    val hasKey: Boolean,
    val description: Boolean,
    val fields: Map<String, FieldDef>
)

data class FieldDef(
    val name: String,
    val type: String,
    val required: Boolean,
    val description: Boolean
)

data class EnumDef(val name: String, val values: Set<String>)
data class UnionDef(val name: String, val members: Set<String>)

object SdlParser {
    fun parse(sdl: String): SdlDocument {
        val types = mutableMapOf<String, TypeDef>()
        val enums = mutableMapOf<String, EnumDef>()
        val unions = mutableMapOf<String, UnionDef>()
        val lines = sdl.lines()
        var pendingDescription = false
        var i = 0
        while (i < lines.size) {
            val raw = lines[i]
            val line = raw.trim()
            if (line.startsWith("\"\"\"") || line.startsWith("\"")) {
                pendingDescription = true
                i++
                continue
            }

            val union = Regex("""^union\s+(\w+)\s*=\s*(.+)$""").find(line)
            if (union != null) {
                unions[union.groupValues[1]] = UnionDef(
                    union.groupValues[1],
                    union.groupValues[2].split("|").map { it.trim() }.filter { it.isNotBlank() }.toSet()
                )
                pendingDescription = false
                i++
                continue
            }

            val enumMatch = Regex("""^enum\s+(\w+)""").find(line)
            if (enumMatch != null) {
                val name = enumMatch.groupValues[1]
                val values = mutableSetOf<String>()
                i++
                while (i < lines.size && !lines[i].trim().startsWith("}")) {
                    val v = lines[i].trim().substringBefore(" ").substringBefore("@")
                    if (v.matches(Regex("""[A-Z][A-Z0-9_]*"""))) values += v
                    i++
                }
                enums[name] = EnumDef(name, values)
                pendingDescription = false
                i++
                continue
            }

            val typeMatch = Regex("""^(type|interface|input)\s+(\w+)([^{]*)\{?""").find(line)
            if (typeMatch != null) {
                val kind = typeMatch.groupValues[1]
                val name = typeMatch.groupValues[2]
                val suffix = typeMatch.groupValues[3]
                val hasKey = suffix.contains("@key")
                val typeDescription = pendingDescription
                val fields = mutableMapOf<String, FieldDef>()
                pendingDescription = false
                i++
                while (i < lines.size && !lines[i].trim().startsWith("}")) {
                    val fieldLine = lines[i].trim()
                    if (fieldLine.startsWith("\"\"\"") || fieldLine.startsWith("\"")) {
                        pendingDescription = true
                        i++
                        continue
                    }
                    val field = Regex("""^(\w+)\s*(?:\([^)]*\))?\s*:\s*([^@#]+)""").find(fieldLine)
                    if (field != null) {
                        val fieldName = field.groupValues[1]
                        val typeRef = field.groupValues[2].trim()
                        fields[fieldName] = FieldDef(
                            name = fieldName,
                            type = typeRef.removeSuffix("!"),
                            required = typeRef.endsWith("!"),
                            description = pendingDescription
                        )
                        pendingDescription = false
                    }
                    i++
                }
                types[name] = TypeDef(name, kind, hasKey, typeDescription, fields)
            }
            pendingDescription = false
            i++
        }
        return SdlDocument(types, enums, unions)
    }
}
