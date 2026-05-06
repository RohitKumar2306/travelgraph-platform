package com.travelgraph.user.domain

import jakarta.persistence.CollectionTable
import jakarta.persistence.Column
import jakarta.persistence.ElementCollection
import jakarta.persistence.Entity
import jakarta.persistence.EnumType
import jakarta.persistence.Enumerated
import jakarta.persistence.FetchType
import jakarta.persistence.Id
import jakarta.persistence.JoinColumn
import jakarta.persistence.Table
import java.time.Instant
import java.util.UUID

@Entity
@Table(name = "users")
class User(
    @Id
    @Column(name = "id", nullable = false, updatable = false)
    var id: UUID = UUID.randomUUID(),

    @Column(name = "name", nullable = false)
    var name: String = "",

    @Column(name = "email", nullable = false, unique = true)
    var email: String = "",

    @Column(name = "loyalty_status", nullable = false, length = 16)
    @Enumerated(EnumType.STRING)
    var loyaltyStatus: LoyaltyStatus = LoyaltyStatus.BRONZE,

    @Column(name = "preferred_currency", nullable = false, length = 3)
    var preferredCurrency: String = "USD",

    @ElementCollection(fetch = FetchType.EAGER)
    @CollectionTable(
        name = "user_saved_properties",
        joinColumns = [JoinColumn(name = "user_id")]
    )
    @Column(name = "property_id")
    var savedPropertyIds: MutableSet<UUID> = mutableSetOf(),

    @Column(name = "created_at", nullable = false)
    var createdAt: Instant = Instant.now()
)

enum class LoyaltyStatus { BRONZE, SILVER, GOLD, PLATINUM }
