package com.travelgraph.property.graphql

import com.expediagroup.graphql.generator.federation.execution.FederatedTypePromiseResolver
import graphql.schema.DataFetchingEnvironment
import org.springframework.stereotype.Component
import java.util.UUID
import java.util.concurrent.CompletableFuture

/**
 * Resolves `Property` entities for the federation `_entities` query.
 *
 * The router calls `_entities(representations: [{__typename: "Property", id: "..."}, ...])`
 * to fetch authoritative data for entities owned by this subgraph. We delegate
 * to [PropertyDataLoader] so the load is batched (one DB query for all the
 * representations in a single GraphQL operation).
 */
@Component
class PropertyEntityResolver : FederatedTypePromiseResolver<PropertyView?> {
    override val typeName: String = "Property"

    override fun resolve(
        environment: DataFetchingEnvironment,
        representation: Map<String, Any>
    ): CompletableFuture<PropertyView?> {
        val rawId = representation["id"]?.toString() ?: return CompletableFuture.completedFuture(null)
        val uuid = runCatching { UUID.fromString(rawId) }.getOrNull()
            ?: return CompletableFuture.completedFuture(null)
        val loader = environment.getDataLoader<UUID, PropertyView?>(PropertyDataLoader.NAME)
            ?: error("PropertyDataLoader not registered")
        return loader.load(uuid)
    }
}
