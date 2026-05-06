package com.travelgraph.pricing.graphql

import com.expediagroup.graphql.generator.annotations.GraphQLDescription
import com.expediagroup.graphql.generator.scalars.ID
import com.travelgraph.pricing.service.PriceCalculator

/**
 * GraphQL `Price` type returned by the pricing subgraph.
 *
 * Monetary fields are exposed as decimal-formatted strings (e.g. "1234.56")
 * rather than floats. This is the standard pattern for money over GraphQL:
 * Float scalars don't preserve precision and BigDecimal isn't a built-in
 * graphql-kotlin scalar, so we serialize at the boundary.
 *
 * `amount` is the pre-tax, post-discount nightly subtotal across all nights.
 * `totalAmount` is what the guest is charged (`amount + taxes`).
 */
@GraphQLDescription("Quoted price for a property over a specific stay window.")
data class PriceQuote(
    @GraphQLDescription("Property the quote is for.")
    val propertyId: ID,
    @GraphQLDescription("ISO-4217 currency code.")
    val currency: String,
    @GraphQLDescription("Number of nights in the stay window (checkOut - checkIn).")
    val nights: Int,
    @GraphQLDescription("Pre-tax subtotal after any loyalty discount has been applied. Decimal string.")
    val amount: String,
    @GraphQLDescription("Tax owed on the discounted subtotal. Decimal string.")
    val taxes: String,
    @GraphQLDescription("Loyalty discount applied to the pre-tax subtotal. Decimal string.")
    val discount: String,
    @GraphQLDescription("Final amount the guest will be charged (amount + taxes). Decimal string.")
    val totalAmount: String
)

fun PriceCalculator.Quote.toView(): PriceQuote = PriceQuote(
    propertyId = ID(propertyId.toString()),
    currency = currency,
    nights = nights,
    amount = amount.toPlainString(),
    taxes = taxes.toPlainString(),
    discount = discount.toPlainString(),
    totalAmount = totalAmount.toPlainString()
)
