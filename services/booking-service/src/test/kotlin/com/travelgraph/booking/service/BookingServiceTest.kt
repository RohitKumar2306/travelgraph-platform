package com.travelgraph.booking.service

import com.travelgraph.booking.config.BookingProperties
import com.travelgraph.booking.domain.Booking
import com.travelgraph.booking.domain.BookingRepository
import com.travelgraph.booking.domain.BookingStatus
import com.travelgraph.booking.graphql.BookingAlreadyCancelledError
import com.travelgraph.booking.graphql.BookingConflictError
import com.travelgraph.booking.graphql.BookingNotFoundError
import com.travelgraph.booking.graphql.BookingView
import com.travelgraph.booking.graphql.PropertyUnavailableError
import io.mockk.every
import io.mockk.mockk
import io.mockk.slot
import io.mockk.verify
import org.junit.jupiter.api.Test
import org.springframework.dao.DataIntegrityViolationException
import java.math.BigDecimal
import java.time.LocalDate
import java.util.Optional
import java.util.UUID
import kotlin.test.assertEquals
import kotlin.test.assertIs

/**
 * Unit tests for the booking service idempotency + payload-union contracts.
 * The repository is mocked with MockK so the test stays offline (no Postgres).
 */
class BookingServiceTest {

    private val inventoryProps = BookingProperties(
        inventory = listOf(
            BookingProperties.RoomInventory("STANDARD", "Standard Room", 2, 10),
            BookingProperties.RoomInventory("DELUXE",   "Deluxe Room",   3, 5),
            BookingProperties.RoomInventory("SUITE",    "Suite",         4, 2)
        )
    )
    private val tinyInventory = BookingProperties(
        inventory = listOf(BookingProperties.RoomInventory("STUDIO", "Studio", 2, 1))
    )

    private val propertyId = UUID.fromString("11111111-1111-1111-1111-000000000001")
    private val userA      = UUID.fromString("80000000-0000-0000-0000-000000000001")
    private val userB      = UUID.fromString("80000000-0000-0000-0000-000000000002")

    private val checkIn = LocalDate.of(2026, 7, 7)
    private val checkOut = LocalDate.of(2026, 7, 9)

    /** Mock that returns whatever was saved on the first matching idempotency key. */
    private fun newRepo(): BookingRepository {
        val repo = mockk<BookingRepository>()
        val store = mutableMapOf<UUID, Booking>()
        every { repo.findByIdempotencyKey(any()) } answers {
            val key = firstArg<String>()
            store.values.firstOrNull { it.idempotencyKey == key }
        }
        every { repo.findOverlappingForUser(any(), any(), any(), any()) } answers {
            val u = firstArg<UUID>(); val p = secondArg<UUID>()
            val ci = thirdArg<LocalDate>(); val co = arg<LocalDate>(3)
            store.values.filter {
                it.userId == u && it.propertyId == p &&
                    it.status != BookingStatus.CANCELLED &&
                    it.checkOut.isAfter(ci) && it.checkIn.isBefore(co)
            }
        }
        every { repo.findOverlapping(any(), any(), any()) } answers {
            val p = firstArg<UUID>(); val ci = secondArg<LocalDate>(); val co = thirdArg<LocalDate>()
            store.values.filter {
                it.propertyId == p &&
                    it.status != BookingStatus.CANCELLED &&
                    it.checkOut.isAfter(ci) && it.checkIn.isBefore(co)
            }
        }
        val bookingSlot = slot<Booking>()
        every { repo.saveAndFlush(capture(bookingSlot)) } answers {
            val b = bookingSlot.captured
            if (store.values.any { it.idempotencyKey == b.idempotencyKey && it.id != b.id }) {
                throw DataIntegrityViolationException("duplicate idempotency_key")
            }
            store[b.id] = b
            b
        }
        every { repo.save(capture(bookingSlot)) } answers {
            val b = bookingSlot.captured
            store[b.id] = b
            b
        }
        every { repo.findById(any()) } answers { Optional.ofNullable(store[firstArg()]) }
        return repo
    }

    @Test
    fun `same idempotency key returns the same booking and never creates a second one`() {
        val repo = newRepo()
        val svc = BookingService(repo, inventoryProps)

        val first = svc.create(propertyId, userA, checkIn, checkOut, BigDecimal("220.00"), "USD", "key-A")
        val second = svc.create(propertyId, userA, checkIn, checkOut, BigDecimal("220.00"), "USD", "key-A")

        val firstView = assertIs<BookingView>(first)
        val secondView = assertIs<BookingView>(second)
        assertEquals(firstView.id, secondView.id)
        verify(exactly = 1) { repo.saveAndFlush(any()) }
    }

    @Test
    fun `different user with overlapping window does not collide - only same-user overlap is a conflict`() {
        val repo = newRepo()
        val svc = BookingService(repo, inventoryProps)

        svc.create(propertyId, userA, checkIn, checkOut, BigDecimal("220.00"), "USD", "key-A")
        val other = svc.create(propertyId, userB, checkIn.plusDays(1), checkOut.plusDays(1),
            BigDecimal("220.00"), "USD", "key-B")
        assertIs<BookingView>(other)
    }

    @Test
    fun `same user overlapping the same property returns BookingConflictError`() {
        val repo = newRepo()
        val svc = BookingService(repo, inventoryProps)

        val first = svc.create(propertyId, userA, checkIn, checkOut,
            BigDecimal("220.00"), "USD", "key-A") as BookingView

        val conflict = svc.create(propertyId, userA, checkIn.plusDays(1), checkOut.plusDays(1),
            BigDecimal("220.00"), "USD", "key-B")
        val err = assertIs<BookingConflictError>(conflict)
        assertEquals(first.id, err.conflictingBookingId)
    }

    @Test
    fun `availability exhaustion returns PropertyUnavailableError`() {
        val repo = newRepo()
        val svc = BookingService(repo, tinyInventory)

        svc.create(propertyId, userA, checkIn, checkOut, BigDecimal("100.00"), "USD", "key-A")
        val result = svc.create(propertyId, userB, checkIn, checkOut, BigDecimal("100.00"), "USD", "key-B")
        assertIs<PropertyUnavailableError>(result)
    }

    @Test
    fun `cancel of unknown booking returns BookingNotFoundError`() {
        val svc = BookingService(newRepo(), inventoryProps)
        assertIs<BookingNotFoundError>(svc.cancel(UUID.randomUUID()))
    }

    @Test
    fun `cancel of already-cancelled booking returns BookingAlreadyCancelledError`() {
        val repo = newRepo()
        val svc = BookingService(repo, inventoryProps)

        val booked = svc.create(propertyId, userA, checkIn, checkOut,
            BigDecimal("100.00"), "USD", "key-A") as BookingView

        assertIs<BookingView>(svc.cancel(UUID.fromString(booked.id.value)))
        assertIs<BookingAlreadyCancelledError>(svc.cancel(UUID.fromString(booked.id.value)))
    }
}
