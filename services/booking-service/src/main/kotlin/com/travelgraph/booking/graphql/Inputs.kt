package com.travelgraph.booking.graphql

import com.expediagroup.graphql.generator.annotations.GraphQLDescription
import com.expediagroup.graphql.generator.scalars.ID

@GraphQLDescription("Input for createBooking.")
data class CreateBookingInput(
    @GraphQLDescription("Property to book.")
    val propertyId: ID,
    @GraphQLDescription("Guest creating the booking.")
    val userId: ID,
    @GraphQLDescription("Inclusive ISO local date (YYYY-MM-DD).")
    val checkIn: String,
    @GraphQLDescription("Exclusive ISO local date (YYYY-MM-DD).")
    val checkOut: String,
    @GraphQLDescription("Pre-quoted total. The booking service trusts this until phase 3 wires it to the pricing subgraph through federation.")
    val totalAmount: String,
    @GraphQLDescription("ISO-4217 currency code.")
    val currency: String,
    @GraphQLDescription("Client-supplied idempotency key. Reusing the same key is safe and returns the existing booking.")
    val idempotencyKey: String
)

@GraphQLDescription("Input for cancelBooking.")
data class CancelBookingInput(
    @GraphQLDescription("Booking to cancel.")
    val bookingId: ID
)

@GraphQLDescription("Room category offered at a property.")
data class RoomType(
    @GraphQLDescription("Stable code, e.g. STANDARD / DELUXE / SUITE.")
    val code: String,
    @GraphQLDescription("Human-readable category name.")
    val name: String,
    @GraphQLDescription("Maximum guest count for this room type.")
    val capacity: Int,
    @GraphQLDescription("Total physical rooms of this type at the property.")
    val total: Int,
    @GraphQLDescription("Rooms of this type available for the queried date window.")
    val available: Int
)
