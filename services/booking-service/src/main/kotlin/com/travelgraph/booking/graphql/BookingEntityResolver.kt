package com.travelgraph.booking.graphql

import com.expediagroup.graphql.generator.federation.execution.FederatedTypePromiseResolver
import graphql.schema.DataFetchingEnvironment
import org.springframework.stereotype.Component
import java.util.UUID
import java.util.concurrent.CompletableFuture

/**
 * Resolves `Booking` entities for the federation `_entities` query, batching
 * via [BookingDataLoader].
 */
@Component
class BookingEntityResolver : FederatedTypePromiseResolver<BookingView?> {
    override val typeName: String = "Booking"

    override fun resolve(
        environment: DataFetchingEnvironment,
        representation: Map<String, Any>
    ): CompletableFuture<BookingView?> {
        val rawId = representation["id"]?.toString() ?: return CompletableFuture.completedFuture(null)
        val uuid = runCatching { UUID.fromString(rawId) }.getOrNull()
            ?: return CompletableFuture.completedFuture(null)
        val loader = environment.getDataLoader<UUID, BookingView?>(BookingDataLoader.NAME)
            ?: error("BookingDataLoader not registered")
        return loader.load(uuid)
    }
}
