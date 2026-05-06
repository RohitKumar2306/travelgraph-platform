package com.travelgraph.booking.service

import com.travelgraph.booking.config.BookingProperties
import com.travelgraph.booking.domain.Booking
import com.travelgraph.booking.domain.BookingRepository
import com.travelgraph.booking.domain.BookingStatus
import com.travelgraph.booking.graphql.BookingAlreadyCancelledError
import com.travelgraph.booking.graphql.BookingConflictError
import com.travelgraph.booking.graphql.CancelBookingPayload
import com.travelgraph.booking.graphql.BookingNotFoundError
import com.travelgraph.booking.graphql.CreateBookingPayload
import com.travelgraph.booking.graphql.PropertyUnavailableError
import com.travelgraph.booking.graphql.RoomType
import com.travelgraph.booking.graphql.toView
import com.expediagroup.graphql.generator.scalars.ID
import org.springframework.dao.DataIntegrityViolationException
import org.springframework.stereotype.Service
import org.springframework.transaction.annotation.Transactional
import java.math.BigDecimal
import java.time.Instant
import java.time.LocalDate
import java.util.UUID

/**
 * Booking lifecycle.
 *
 * Idempotency is enforced two ways:
 *  1. **Pre-flight lookup** by [Booking.idempotencyKey] - fast path for retries
 *     that arrive after the first request committed.
 *  2. **Unique DB constraint** - covers the race where two requests arrive
 *     concurrently. The second commit gets [DataIntegrityViolationException];
 *     we catch it, look up the surviving row, and return it.
 */
@Service
class BookingService(
    private val repository: BookingRepository,
    private val inventory: BookingProperties
) {

    @Transactional
    fun create(
        propertyId: UUID,
        userId: UUID,
        checkIn: LocalDate,
        checkOut: LocalDate,
        totalAmount: BigDecimal,
        currency: String,
        idempotencyKey: String
    ): CreateBookingPayload {
        require(checkOut.isAfter(checkIn)) { "checkOut must be after checkIn" }

        // 1) idempotent fast-path
        repository.findByIdempotencyKey(idempotencyKey)?.let { return it.toView() }

        // 2) per-user same-property overlap = conflict
        repository.findOverlappingForUser(userId, propertyId, checkIn, checkOut)
            .firstOrNull()
            ?.let {
                return BookingConflictError(
                    message = "User already has an active booking on this property that overlaps the requested window.",
                    conflictingBookingId = ID(it.id.toString())
                )
            }

        // 3) inventory check - any room type must have at least 1 available
        val rooms = roomsFor(propertyId, checkIn, checkOut)
        if (rooms.none { it.available > 0 }) {
            return PropertyUnavailableError(
                propertyId = ID(propertyId.toString()),
                reason = "No rooms available for the requested window."
            )
        }

        val booking = Booking(
            id = UUID.randomUUID(),
            propertyId = propertyId,
            userId = userId,
            checkIn = checkIn,
            checkOut = checkOut,
            status = BookingStatus.CONFIRMED,
            totalAmount = totalAmount,
            currency = currency,
            idempotencyKey = idempotencyKey,
            createdAt = Instant.now()
        )

        return try {
            repository.saveAndFlush(booking).toView()
        } catch (_: DataIntegrityViolationException) {
            // Race: another request won the unique-key insert. Return whatever
            // landed first - same key MUST never produce two bookings.
            repository.findByIdempotencyKey(idempotencyKey)?.toView()
                ?: throw IllegalStateException("Idempotency conflict could not be reconciled for key=$idempotencyKey")
        }
    }

    @Transactional
    fun cancel(bookingId: UUID): CancelBookingPayload {
        val existing = repository.findById(bookingId).orElse(null)
            ?: return BookingNotFoundError(
                bookingId = ID(bookingId.toString()),
                message = "No booking found with id=$bookingId."
            )

        if (existing.status == BookingStatus.CANCELLED) {
            return BookingAlreadyCancelledError(
                bookingId = ID(existing.id.toString()),
                message = "Booking is already cancelled."
            )
        }

        existing.status = BookingStatus.CANCELLED
        return repository.save(existing).toView()
    }

    /**
     * Returns the room-type inventory adjusted for already-booked
     * (CONFIRMED or PENDING) overlap in the [checkIn, checkOut) range.
     *
     * Demo-grade: each booking subtracts 1 from every room type's pool because
     * the seed booking model doesn't track which type was booked. Phase 3 will
     * introduce a real inventory subgraph and this becomes per-type.
     */
    fun roomsFor(propertyId: UUID, checkIn: LocalDate, checkOut: LocalDate): List<RoomType> {
        val overlap = repository.findOverlapping(propertyId, checkIn, checkOut).size
        return inventory.inventory.map { def ->
            RoomType(
                code = def.code,
                name = def.name,
                capacity = def.capacity,
                total = def.total,
                available = (def.total - overlap).coerceAtLeast(0)
            )
        }
    }
}
