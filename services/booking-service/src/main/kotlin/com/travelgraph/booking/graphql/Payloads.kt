package com.travelgraph.booking.graphql

import com.expediagroup.graphql.generator.annotations.GraphQLDescription
import com.expediagroup.graphql.generator.scalars.ID

/**
 * Errors-as-data: every mutation returns a payload union whose variants are
 * either the success type or a typed error type. This is a Context.md
 * non-negotiable - callers must `... on FooError { message }` rather than
 * sniff the GraphQL `errors` array.
 *
 * graphql-kotlin maps an empty Kotlin `sealed interface` to a GraphQL
 * `union`; each direct variant becomes a member of that union.
 */

// ---------- createBooking ---------------------------------------------------

@GraphQLDescription("Result of attempting to create a booking.")
sealed interface CreateBookingPayload

@GraphQLDescription("Returned when an overlapping non-cancelled booking already exists for this user + property.")
data class BookingConflictError(
    @GraphQLDescription("Human-readable explanation of the conflict.")
    val message: String,
    @GraphQLDescription("Id of the booking that is in conflict.")
    val conflictingBookingId: ID
) : CreateBookingPayload

@GraphQLDescription("Returned when the property has no available rooms for the requested window.")
data class PropertyUnavailableError(
    @GraphQLDescription("Property that has no availability.")
    val propertyId: ID,
    @GraphQLDescription("Why the property is unavailable for the window.")
    val reason: String
) : CreateBookingPayload

// ---------- cancelBooking ---------------------------------------------------

@GraphQLDescription("Result of attempting to cancel a booking.")
sealed interface CancelBookingPayload

@GraphQLDescription("Returned when the booking id does not exist.")
data class BookingNotFoundError(
    @GraphQLDescription("Booking id that was looked up.")
    val bookingId: ID,
    @GraphQLDescription("Human-readable explanation.")
    val message: String
) : CancelBookingPayload

@GraphQLDescription("Returned when the booking is already cancelled.")
data class BookingAlreadyCancelledError(
    @GraphQLDescription("Booking id that was already cancelled.")
    val bookingId: ID,
    @GraphQLDescription("Human-readable explanation.")
    val message: String
) : CancelBookingPayload
