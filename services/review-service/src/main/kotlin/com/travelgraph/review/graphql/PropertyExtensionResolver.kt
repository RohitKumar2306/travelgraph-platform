package com.travelgraph.review.graphql

import com.expediagroup.graphql.generator.federation.execution.FederatedTypePromiseResolver
import com.expediagroup.graphql.generator.scalars.ID
import graphql.schema.DataFetchingEnvironment
import org.springframework.stereotype.Component
import java.util.concurrent.CompletableFuture

/**
 * Resolves the federated `Property` entity for the review subgraph's
 * `_entities` query. Same lazy pattern as the pricing subgraph: we just
 * package the id; the actual review lookup happens when the gateway
 * selects `reviews` or `reviewSummary` on the returned object.
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
