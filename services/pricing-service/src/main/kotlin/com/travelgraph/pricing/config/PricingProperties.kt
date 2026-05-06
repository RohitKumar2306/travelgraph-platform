package com.travelgraph.pricing.config

import org.springframework.boot.context.properties.ConfigurationProperties
import java.math.BigDecimal
import java.time.LocalDate

/**
 * Tunable pricing policy. Defaults match `application.yml`. Values are
 * decimal multipliers (0.20 = +20%, 0.10 = -10% when used as a discount).
 */
@ConfigurationProperties(prefix = "travelgraph.pricing")
data class PricingProperties(
    /** Per-night uplift applied to Friday and Saturday nights. */
    val weekendUplift: BigDecimal = BigDecimal("0.20"),
    /** Per-night uplift applied to nights overlapping a holiday in [holidays]. */
    val holidayUplift: BigDecimal = BigDecimal("0.35"),
    /** Discount table by loyalty tier name (case-insensitive lookup). */
    val loyalty: Map<String, BigDecimal> = mapOf(
        "BRONZE"   to BigDecimal("0.00"),
        "SILVER"   to BigDecimal("0.05"),
        "GOLD"     to BigDecimal("0.10"),
        "PLATINUM" to BigDecimal("0.15")
    ),
    /** Sorted set of holiday dates. Stored as UTC-naive ISO local dates. */
    val holidays: List<LocalDate> = emptyList()
)
