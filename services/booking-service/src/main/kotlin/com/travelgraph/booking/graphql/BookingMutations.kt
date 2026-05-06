package com.travelgraph.booking.graphql

import com.expediagroup.graphql.generator.annotations.GraphQLDescription
import com.expediagroup.graphql.server.operations.Mutation
import com.travelgraph.booking.service.BookingService
import org.springframework.stereotype.Component
import java.math.BigDecimal
import java.time.LocalDate
import java.util.UUID

@Component
class BookingMutations(private val service: BookingService) : Mutation {

    @GraphQLDescription(
        "Create a booking. Idempotent: passing the same idempotencyKey twice " +
            "returns the same booking, never two. Returns a typed payload union."
    )
    fun createBooking(input: CreateBookingInput): CreateBookingPayload {
        val checkIn = LocalDate.parse(input.checkIn)
        val checkOut = LocalDate.parse(input.checkOut)
        val total = BigDecimal(input.totalAmount)
        return service.create(
            propertyId = UUID.fromString(input.propertyId.value),
            userId = UUID.fromString(input.userId.value),
            checkIn = checkIn,
            checkOut = checkOut,
            totalAmount = total,
            currency = input.currency,
            idempotencyKey = input.idempotencyKey
        )
    }

    @GraphQLDescription("Cancel a booking. Returns a typed payload union.")
    fun cancelBooking(input: CancelBookingInput): CancelBookingPayload =
        service.cancel(UUID.fromString(input.bookingId.value))
}
