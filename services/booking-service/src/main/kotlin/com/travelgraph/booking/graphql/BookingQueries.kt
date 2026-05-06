package com.travelgraph.booking.graphql

import com.expediagroup.graphql.generator.annotations.GraphQLDescription
import com.expediagroup.graphql.generator.scalars.ID
import com.expediagroup.graphql.server.operations.Query
import com.travelgraph.booking.domain.BookingRepository
import com.travelgraph.booking.service.BookingService
import org.springframework.stereotype.Component
import java.time.LocalDate
import java.time.format.DateTimeParseException
import java.util.UUID

@Component
class BookingQueries(
    private val repository: BookingRepository,
    private val service: BookingService
) : Query {

    @GraphQLDescription("All non-deleted bookings for a user, most recent check-in first.")
    fun bookings(
        @GraphQLDescription("User whose bookings should be listed.")
        userId: ID
    ): List<BookingView> =
        repository.findByUserIdOrderByCheckInDesc(UUID.fromString(userId.value)).map { it.toView() }

    @GraphQLDescription("Available rooms by type for a property over a given window.")
    fun availableRooms(
        @GraphQLDescription("Property to query.")
        propertyId: ID,
        @GraphQLDescription("Inclusive ISO local date (YYYY-MM-DD).")
        checkIn: String,
        @GraphQLDescription("Exclusive ISO local date (YYYY-MM-DD).")
        checkOut: String
    ): List<RoomType> {
        val ci = parseDate(checkIn) ?: throw IllegalArgumentException("Invalid checkIn: $checkIn")
        val co = parseDate(checkOut) ?: throw IllegalArgumentException("Invalid checkOut: $checkOut")
        require(co.isAfter(ci)) { "checkOut must be after checkIn" }
        return service.roomsFor(UUID.fromString(propertyId.value), ci, co)
    }

    private fun parseDate(value: String): LocalDate? = try {
        LocalDate.parse(value)
    } catch (_: DateTimeParseException) {
        null
    }
}
