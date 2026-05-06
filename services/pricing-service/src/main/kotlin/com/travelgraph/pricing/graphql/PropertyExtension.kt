package com.travelgraph.pricing.graphql

import com.expediagroup.graphql.generator.annotations.GraphQLDescription
import com.expediagroup.graphql.generator.annotations.GraphQLName
import com.expediagroup.graphql.generator.federation.directives.ExtendsDirective
import com.expediagroup.graphql.generator.federation.directives.ExternalDirective
import com.expediagroup.graphql.generator.federation.directives.FieldSet
import com.expediagroup.graphql.generator.federation.directives.KeyDirective
import com.expediagroup.graphql.generator.scalars.ID
import com.travelgraph.pricing.domain.Price
import com.travelgraph.pricing.service.PriceCalculator
import graphql.schema.DataFetchingEnvironment
import java.time.LocalDate
import java.util.UUID
import java.util.concurrent.CompletableFuture

/**
 * Pricing's view of the federated `Property` entity.
 *
 * Pricing does NOT own `Property` (the property subgraph does). It only
 * **extends** it with a pricing field. We re-declare the type with the same
 * `@key(fields: "id")` and mark it `@extends`, treating `id` as `@external`
 * so the schema reads:
 *
 * ```graphql
 * type Property @key(fields: "id") @extends {
 *   id: ID! @external
 *   price(checkIn: String, checkOut: String, loyaltyTier: String): Price
 * }
 * ```
 *
 * The router will call `_entities(representations: [{__typename: "Property", id: "..."}, ...])`
 * on this subgraph whenever a query selects `Property.price`, and graphql-kotlin
 * will instantiate this class via [PropertyExtensionResolver].
 */
@KeyDirective(fields = FieldSet("id"))
@ExtendsDirective
@GraphQLName("Property")
@GraphQLDescription("Pricing-side extension of the Property entity. Only the price field is owned here.")
class PropertyExtension(
    @ExternalDirective
    @GraphQLDescription("Globally unique property identifier (owned by the property subgraph).")
    val id: ID
) {

    @GraphQLDescription(
        "Calculated price quote for the given stay window. Defaults to a one-night stay starting today."
    )
    fun price(
        env: DataFetchingEnvironment,
        @GraphQLDescription("Inclusive ISO local date (YYYY-MM-DD). Defaults to today.")
        checkIn: String? = null,
        @GraphQLDescription("Exclusive ISO local date (YYYY-MM-DD). Defaults to checkIn + 1 night.")
        checkOut: String? = null,
        @GraphQLDescription("Loyalty tier of the requesting guest. Optional.")
        loyaltyTier: String? = null
    ): CompletableFuture<PriceQuote?> {
        val parsedCheckIn = PriceCalculator.parseDateOrNull(checkIn) ?: LocalDate.now()
        val parsedCheckOut = PriceCalculator.parseDateOrNull(checkOut) ?: parsedCheckIn.plusDays(1)

        val loader = env.getDataLoader<UUID, Price?>(PricingDataLoader.NAME)
            ?: error("PricingDataLoader not registered")
        val calculator = PricingBeans.calculator
            ?: error("PriceCalculator was not initialized; PricingBeans bridge missing")

        val uuid = runCatching { UUID.fromString(id.value) }.getOrNull()
            ?: return CompletableFuture.completedFuture(null)

        return loader.load(uuid).thenApply { price ->
            price?.let { calculator.quote(it, parsedCheckIn, parsedCheckOut, loyaltyTier).toView() }
        }
    }
}
