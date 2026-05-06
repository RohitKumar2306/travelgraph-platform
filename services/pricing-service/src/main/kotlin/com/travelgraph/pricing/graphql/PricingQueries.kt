package com.travelgraph.pricing.graphql

import com.expediagroup.graphql.generator.annotations.GraphQLDescription
import com.expediagroup.graphql.generator.scalars.ID
import com.expediagroup.graphql.server.operations.Query
import com.travelgraph.pricing.domain.Price
import com.travelgraph.pricing.service.PriceCalculator
import graphql.schema.DataFetchingEnvironment
import org.springframework.stereotype.Component
import java.time.LocalDate
import java.util.UUID
import java.util.concurrent.CompletableFuture

@Component
class PricingQueries(
    private val calculator: PriceCalculator
) : Query {

    @GraphQLDescription(
        "Quote a stay for a property. checkIn / checkOut are ISO local dates " +
            "(YYYY-MM-DD). loyaltyTier is optional and one of BRONZE, SILVER, GOLD, PLATINUM."
    )
    fun price(
        env: DataFetchingEnvironment,
        propertyId: ID,
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

        val propertyUuid = UUID.fromString(propertyId.value)
        return loader.load(propertyUuid).thenApply { price ->
            price?.let {
                calculator.quote(it, parsedCheckIn, parsedCheckOut, loyaltyTier).toView()
            }
        }
    }
}
