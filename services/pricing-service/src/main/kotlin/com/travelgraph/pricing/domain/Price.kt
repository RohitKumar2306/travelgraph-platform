package com.travelgraph.pricing.domain

import jakarta.persistence.Column
import jakarta.persistence.Entity
import jakarta.persistence.EnumType
import jakarta.persistence.Enumerated
import jakarta.persistence.Id
import jakarta.persistence.Table
import java.math.BigDecimal
import java.util.UUID

/**
 * Baseline pricing record for a single property.
 *
 * Only invariants of the property live here (base nightly rate, tax, currency,
 * season tier). Day-of-week, holiday, and loyalty modifiers are *policy* and
 * are computed by [com.travelgraph.pricing.service.PriceCalculator] at query
 * time so they can be tuned without DB writes.
 */
@Entity
@Table(name = "prices")
class Price(
    @Id
    @Column(name = "id", nullable = false, updatable = false)
    var id: UUID = UUID.randomUUID(),

    @Column(name = "property_id", nullable = false, unique = true)
    var propertyId: UUID = UUID.randomUUID(),

    @Column(name = "base_price", nullable = false, precision = 12, scale = 2)
    var basePrice: BigDecimal = BigDecimal.ZERO,

    @Column(name = "tax_rate", nullable = false, precision = 5, scale = 4)
    var taxRate: BigDecimal = BigDecimal.ZERO,

    @Column(name = "currency", nullable = false, length = 3)
    var currency: String = "USD",

    @Column(name = "season", nullable = false, length = 16)
    @Enumerated(EnumType.STRING)
    var season: Season = Season.REGULAR
)

enum class Season { LOW, REGULAR, HIGH, PEAK }
