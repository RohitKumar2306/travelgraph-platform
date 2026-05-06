package com.travelgraph.review.domain

import jakarta.persistence.Column
import jakarta.persistence.Entity
import jakarta.persistence.Id
import jakarta.persistence.Table
import jakarta.persistence.UniqueConstraint
import java.time.Instant
import java.util.UUID

@Entity
@Table(
    name = "reviews",
    uniqueConstraints = [UniqueConstraint(name = "uq_reviews_property_user", columnNames = ["property_id", "user_id"])]
)
class Review(
    @Id
    @Column(name = "id", nullable = false, updatable = false)
    var id: UUID = UUID.randomUUID(),

    @Column(name = "property_id", nullable = false)
    var propertyId: UUID = UUID.randomUUID(),

    @Column(name = "user_id", nullable = false)
    var userId: UUID = UUID.randomUUID(),

    @Column(name = "rating", nullable = false)
    var rating: Int = 5,

    @Column(name = "comment", nullable = false, length = 2048)
    var comment: String = "",

    @Column(name = "created_at", nullable = false)
    var createdAt: Instant = Instant.now()
)
