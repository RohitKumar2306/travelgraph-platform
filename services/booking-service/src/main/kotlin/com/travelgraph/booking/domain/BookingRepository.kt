package com.travelgraph.booking.domain

import org.springframework.data.jpa.repository.JpaRepository
import org.springframework.data.jpa.repository.Query
import org.springframework.data.repository.query.Param
import java.time.LocalDate
import java.util.UUID

interface BookingRepository : JpaRepository<Booking, UUID> {

    fun findByIdempotencyKey(idempotencyKey: String): Booking?

    fun findByUserIdOrderByCheckInDesc(userId: UUID): List<Booking>

    @Query("SELECT b FROM Booking b WHERE b.id IN :ids")
    fun findAllByIds(@Param("ids") ids: Collection<UUID>): List<Booking>

    /**
     * Returns CONFIRMED or PENDING bookings for the given property whose
     * date window overlaps the [checkIn, checkOut) range. Used both by the
     * availability check and by the create-booking conflict guard.
     *
     * Half-open interval semantics: `b.checkOut > checkIn AND b.checkIn < checkOut`.
     */
    @Query(
        """
        SELECT b FROM Booking b
        WHERE b.propertyId = :propertyId
          AND b.status <> com.travelgraph.booking.domain.BookingStatus.CANCELLED
          AND b.checkOut > :checkIn
          AND b.checkIn  < :checkOut
        """
    )
    fun findOverlapping(
        @Param("propertyId") propertyId: UUID,
        @Param("checkIn") checkIn: LocalDate,
        @Param("checkOut") checkOut: LocalDate
    ): List<Booking>

    /** Same as [findOverlapping] but only for a specific user, for the conflict guard. */
    @Query(
        """
        SELECT b FROM Booking b
        WHERE b.userId = :userId
          AND b.propertyId = :propertyId
          AND b.status <> com.travelgraph.booking.domain.BookingStatus.CANCELLED
          AND b.checkOut > :checkIn
          AND b.checkIn  < :checkOut
        """
    )
    fun findOverlappingForUser(
        @Param("userId") userId: UUID,
        @Param("propertyId") propertyId: UUID,
        @Param("checkIn") checkIn: LocalDate,
        @Param("checkOut") checkOut: LocalDate
    ): List<Booking>
}
