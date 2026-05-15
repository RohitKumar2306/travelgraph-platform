package com.travelgraph.booking.graphql

import com.expediagroup.graphql.generator.annotations.GraphQLDescription
import com.expediagroup.graphql.server.operations.Mutation
import com.travelgraph.booking.auth.UserContextHolder
import com.travelgraph.booking.service.BookingService
import org.springframework.stereotype.Component
import java.math.BigDecimal
import java.time.LocalDate
import java.util.UUID

@Component
class BookingMutations(
    private val service: BookingService,
    private val userContextHolder: UserContextHolder
) : Mutation {

    @GraphQLDescription(
        "Create a booking. Idempotent: passing the same idempotencyKey twice " +
            "returns the same booking, never two. Returns a typed payload union."
    )
    fun createBooking(input: CreateBookingInput): CreateBookingPayload {
        val userContext = userContextHolder.current()
        if (userContext.isAnonymous) {
            return AuthenticationRequiredError()
        }
        val checkIn = LocalDate.parse(input.checkIn)
        val checkOut = LocalDate.parse(input.checkOut)
        val total = BigDecimal(input.totalAmount)
        return service.create(
            propertyId = UUID.fromString(input.propertyId.value),
            userId = UUID.fromString(userContext.userId),
            checkIn = checkIn,
            checkOut = checkOut,
            totalAmount = total,
            currency = input.currency,
            idempotencyKey = input.idempotencyKey
        )
    }

    @GraphQLDescription("Cancel a booking. Returns a typed payload union.")
    fun cancelBooking(input: CancelBookingInput): CancelBookingPayload {
        if (userContextHolder.current().isAnonymous) {
            return AuthenticationRequiredError()
        }
        return service.cancel(UUID.fromString(input.bookingId.value))
    }
}
