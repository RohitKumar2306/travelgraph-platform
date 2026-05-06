package com.travelgraph.review.graphql

import com.expediagroup.graphql.generator.annotations.GraphQLDescription
import com.expediagroup.graphql.generator.annotations.GraphQLName
import com.expediagroup.graphql.generator.federation.directives.ExtendsDirective
import com.expediagroup.graphql.generator.federation.directives.ExternalDirective
import com.expediagroup.graphql.generator.federation.directives.FieldSet
import com.expediagroup.graphql.generator.federation.directives.KeyDirective
import com.expediagroup.graphql.generator.scalars.ID
import graphql.schema.DataFetchingEnvironment
import java.util.UUID
import java.util.concurrent.CompletableFuture

/**
 * Review's view of the federated `Property` entity.
 *
 * Like pricing, review does not own `Property`; it extends the type owned
 * by the property subgraph with two new fields:
 *
 * ```graphql
 * type Property @key(fields: "id") @extends {
 *   id: ID! @external
 *   reviews(limit: Int = 10): [Review!]!
 *   reviewSummary: ReviewSummary!
 * }
 * ```
 */
@KeyDirective(fields = FieldSet("id"))
@ExtendsDirective
@GraphQLName("Property")
@GraphQLDescription("Review-side extension of the Property entity.")
class PropertyExtension(
    @ExternalDirective
    @GraphQLDescription("Globally unique property identifier (owned by the property subgraph).")
    val id: ID
) {

    @GraphQLDescription("Reviews for this property, most recent first.")
    fun reviews(
        env: DataFetchingEnvironment,
        @GraphQLDescription("Maximum number of reviews to return (1..100). Defaults to 10.")
        limit: Int = 10
    ): CompletableFuture<List<ReviewView>> {
        val capped = limit.coerceIn(1, 100)
        val uuid = runCatching { UUID.fromString(id.value) }.getOrNull()
            ?: return CompletableFuture.completedFuture(emptyList())
        val loader = env.getDataLoader<UUID, List<ReviewView>>(ReviewByPropertyDataLoader.NAME)
            ?: error("ReviewByPropertyDataLoader not registered")
        return loader.load(uuid).thenApply { reviews -> reviews.take(capped) }
    }

    @GraphQLDescription("Aggregate review statistics for this property.")
    fun reviewSummary(): CompletableFuture<ReviewSummary> {
        val service = ReviewBeans.service
            ?: error("ReviewService was not initialized; ReviewBeans bridge missing")
        val uuid = runCatching { UUID.fromString(id.value) }.getOrNull()
            ?: return CompletableFuture.completedFuture(
                ReviewSummary(propertyId = id, count = 0, averageRating = 0.0)
            )
        return CompletableFuture.completedFuture(service.summaryFor(uuid))
    }
}
