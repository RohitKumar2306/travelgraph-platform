package com.travelgraph.booking.graphql

import com.expediagroup.graphql.generator.annotations.GraphQLDescription
import com.expediagroup.graphql.generator.annotations.GraphQLName
import com.expediagroup.graphql.generator.scalars.ID
import com.travelgraph.booking.domain.Booking
import com.travelgraph.booking.domain.BookingStatus

@GraphQLName("Booking")
@GraphQLDescription("A confirmed, pending, or cancelled booking for a property.")
data class BookingView(
    @GraphQLDescription("Globally unique booking identifier.")
    val id: ID,
    @GraphQLDescription("Property the booking is for.")
    val propertyId: ID,
    @GraphQLDescription("Guest making the booking.")
    val userId: ID,
    @GraphQLDescription("Inclusive ISO local date of check-in.")
    val checkIn: String,
    @GraphQLDescription("Exclusive ISO local date of check-out.")
    val checkOut: String,
    @GraphQLDescription("Lifecycle state of the booking.")
    val status: BookingStatus,
    @GraphQLDescription("Total amount the guest is/was charged. Decimal string.")
    val totalAmount: String,
    @GraphQLDescription("ISO-4217 currency code.")
    val currency: String,
    @GraphQLDescription("Client-supplied idempotency token.")
    val idempotencyKey: String,
    @GraphQLDescription("Server-side creation timestamp (RFC-3339).")
    val createdAt: String
) : CreateBookingPayload, CancelBookingPayload

fun Booking.toView(): BookingView = BookingView(
    id = ID(id.toString()),
    propertyId = ID(propertyId.toString()),
    userId = ID(userId.toString()),
    checkIn = checkIn.toString(),
    checkOut = checkOut.toString(),
    status = status,
    totalAmount = totalAmount.toPlainString(),
    currency = currency,
    idempotencyKey = idempotencyKey,
    createdAt = createdAt.toString()
)
