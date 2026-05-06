package com.travelgraph.user.graphql

import com.expediagroup.graphql.generator.federation.execution.FederatedTypePromiseResolver
import graphql.schema.DataFetchingEnvironment
import org.springframework.stereotype.Component
import java.util.UUID
import java.util.concurrent.CompletableFuture

/**
 * Resolves `User` entities for the federation `_entities` query, batching
 * via [UserDataLoader].
 */
@Component
class UserEntityResolver : FederatedTypePromiseResolver<UserView?> {
    override val typeName: String = "User"

    override fun resolve(
        environment: DataFetchingEnvironment,
        representation: Map<String, Any>
    ): CompletableFuture<UserView?> {
        val rawId = representation["id"]?.toString() ?: return CompletableFuture.completedFuture(null)
        val uuid = runCatching { UUID.fromString(rawId) }.getOrNull()
            ?: return CompletableFuture.completedFuture(null)
        val loader = environment.getDataLoader<UUID, UserView?>(UserDataLoader.NAME)
            ?: error("UserDataLoader not registered")
        return loader.load(uuid)
    }
}
