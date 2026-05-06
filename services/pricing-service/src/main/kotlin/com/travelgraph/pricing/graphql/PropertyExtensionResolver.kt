package com.travelgraph.pricing.graphql

import com.expediagroup.graphql.generator.federation.execution.FederatedTypePromiseResolver
import com.expediagroup.graphql.generator.scalars.ID
import graphql.schema.DataFetchingEnvironment
import org.springframework.stereotype.Component
import java.util.concurrent.CompletableFuture

/**
 * Resolves the federated `Property` entity for the pricing subgraph's
 * `_entities` query.
 *
 * The router supplies representations like `{__typename: "Property", id: "..."}`.
 * We just need to package each `id` into a [PropertyExtension] - the actual
 * pricing lookup happens lazily when the gateway selects the `price` field
 * on the returned object (see [PropertyExtension.price]).
 *
 * This makes `_entities` essentially free: no DB call until a price field
 * is actually selected, at which point [PricingDataLoader] batches loads
 * across all the property IDs in the request.
 */
@Component
class PropertyExtensionResolver : FederatedTypePromiseResolver<PropertyExtension?> {
    override val typeName: String = "Property"

    override fun resolve(
        environment: DataFetchingEnvironment,
        representation: Map<String, Any>
    ): CompletableFuture<PropertyExtension?> {
        val id = representation["id"]?.toString() ?: return CompletableFuture.completedFuture(null)
        return CompletableFuture.completedFuture(PropertyExtension(ID(id)))
    }
}
