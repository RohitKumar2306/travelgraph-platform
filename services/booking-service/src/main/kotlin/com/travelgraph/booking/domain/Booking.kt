package com.travelgraph.booking.domain

import jakarta.persistence.Column
import jakarta.persistence.Entity
import jakarta.persistence.EnumType
import jakarta.persistence.Enumerated
import jakarta.persistence.Id
import jakarta.persistence.Table
import java.math.BigDecimal
import java.time.Instant
import java.time.LocalDate
import java.util.UUID

@Entity
@Table(name = "bookings")
class Booking(
    @Id
    @Column(name = "id", nullable = false, updatable = false)
    var id: UUID = UUID.randomUUID(),

    @Column(name = "property_id", nullable = false)
    var propertyId: UUID = UUID.randomUUID(),

    @Column(name = "user_id", nullable = false)
    var userId: UUID = UUID.randomUUID(),

    @Column(name = "check_in", nullable = false)
    var checkIn: LocalDate = LocalDate.now(),

    @Column(name = "check_out", nullable = false)
    var checkOut: LocalDate = LocalDate.now().plusDays(1),

    @Column(name = "status", nullable = false, length = 16)
    @Enumerated(EnumType.STRING)
    var status: BookingStatus = BookingStatus.PENDING,

    @Column(name = "total_amount", nullable = false, precision = 12, scale = 2)
    var totalAmount: BigDecimal = BigDecimal.ZERO,

    @Column(name = "currency", nullable = false, length = 3)
    var currency: String = "USD",

    /**
     * Client-supplied idempotency token. Unique constraint at the DB level
     * is what makes "create two bookings with the same key" impossible even
     * under concurrent retries.
     */
    @Column(name = "idempotency_key", nullable = false, unique = true, length = 128)
    var idempotencyKey: String = "",

    @Column(name = "created_at", nullable = false)
    var createdAt: Instant = Instant.now()
)

enum class BookingStatus { PENDING, CONFIRMED, CANCELLED }
