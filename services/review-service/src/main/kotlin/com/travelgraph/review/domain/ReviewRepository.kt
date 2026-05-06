package com.travelgraph.review.domain

import org.springframework.data.domain.Pageable
import org.springframework.data.jpa.repository.JpaRepository
import org.springframework.data.jpa.repository.Query
import org.springframework.data.repository.query.Param
import java.util.UUID

interface ReviewRepository : JpaRepository<Review, UUID> {

    fun findByPropertyIdOrderByCreatedAtDesc(propertyId: UUID, pageable: Pageable): List<Review>

    fun findByPropertyIdAndUserId(propertyId: UUID, userId: UUID): Review?

    @Query("SELECT r FROM Review r WHERE r.propertyId IN :propertyIds")
    fun findAllByPropertyIds(@Param("propertyIds") propertyIds: Collection<UUID>): List<Review>

    @Query(
        """
        SELECT r.propertyId AS propertyId,
               COUNT(r)     AS count,
               AVG(r.rating) AS averageRating
        FROM Review r
        WHERE r.propertyId IN :propertyIds
        GROUP BY r.propertyId
        """
    )
    fun aggregateByPropertyIds(@Param("propertyIds") propertyIds: Collection<UUID>): List<ReviewAggregateRow>

    /** JPA projection for [aggregateByPropertyIds]. */
    interface ReviewAggregateRow {
        val propertyId: UUID
        val count: Long
        val averageRating: Double
    }
}
