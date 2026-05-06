package com.travelgraph.pricing.service

import com.travelgraph.pricing.config.PricingProperties
import com.travelgraph.pricing.domain.Price
import com.travelgraph.pricing.domain.Season
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.assertThrows
import java.math.BigDecimal
import java.time.LocalDate
import java.util.UUID
import kotlin.test.assertEquals

/**
 * Acceptance tests for the weekend / holiday / loyalty pricing rules.
 *
 * Each test exercises one rule axis with deterministic dates so failures
 * point at the specific rule that regressed.
 */
class PriceCalculatorTest {

    private val rules = PricingProperties(
        weekendUplift = BigDecimal("0.20"),
        holidayUplift = BigDecimal("0.35"),
        loyalty = mapOf(
            "BRONZE"   to BigDecimal("0.00"),
            "SILVER"   to BigDecimal("0.05"),
            "GOLD"     to BigDecimal("0.10"),
            "PLATINUM" to BigDecimal("0.15")
        ),
        holidays = listOf(
            LocalDate.of(2026, 7, 4),    // Saturday - both holiday and weekend
            LocalDate.of(2026, 12, 25)   // Friday    - both holiday and weekend
        )
    )
    private val calculator = PriceCalculator(rules)

    private val price = Price(
        propertyId = UUID.fromString("11111111-1111-1111-1111-000000000001"),
        basePrice = BigDecimal("100.00"),
        taxRate = BigDecimal("0.1000"),
        currency = "USD",
        season = Season.REGULAR
    )

    /**
     * 2 weeknights, no holidays, no loyalty: pure base price * nights, plus 10% tax.
     * Tue Jul 7 -> Thu Jul 9 2026 = 2 nights.
     * subtotal: 100 * 2          = 200.00
     * discount:                    0.00
     * taxes:    200.00 * 0.10    = 20.00
     * total:    200.00 + 20.00   = 220.00
     */
    @Test
    fun `weeknight stay with no loyalty applies only base price plus tax`() {
        val quote = calculator.quote(
            price = price,
            checkIn = LocalDate.of(2026, 7, 7),
            checkOut = LocalDate.of(2026, 7, 9),
            loyaltyTier = null
        )

        assertEquals(2, quote.nights)
        assertEquals(BigDecimal("200.00"), quote.amount)
        assertEquals(BigDecimal("0.00"), quote.discount)
        assertEquals(BigDecimal("20.00"), quote.taxes)
        assertEquals(BigDecimal("220.00"), quote.totalAmount)
        assertEquals("USD", quote.currency)
    }

    /**
     * 2-night stay covering Friday + Saturday, no holidays, no loyalty.
     * Fri Jul 10 -> Sun Jul 12 2026.
     * Both nights weekend: 100 * 1.20 * 2 = 240.00
     * tax 10%: 24.00
     * total: 264.00
     */
    @Test
    fun `weekend nights apply 20 percent uplift`() {
        val quote = calculator.quote(
            price = price,
            checkIn = LocalDate.of(2026, 7, 10),
            checkOut = LocalDate.of(2026, 7, 12),
            loyaltyTier = null
        )

        assertEquals(2, quote.nights)
        assertEquals(BigDecimal("240.00"), quote.amount)
        assertEquals(BigDecimal("0.00"), quote.discount)
        assertEquals(BigDecimal("24.00"), quote.taxes)
        assertEquals(BigDecimal("264.00"), quote.totalAmount)
    }

    /**
     * 3-night Independence Day stay with PLATINUM loyalty.
     * Fri Jul 3 -> Mon Jul 6 2026.
     *  - Fri Jul 3:  weekend  -> 100 * 1.20 = 120.00
     *  - Sat Jul 4:  holiday  -> 100 * 1.35 = 135.00 (holiday wins over weekend)
     *  - Sun Jul 5:  base     -> 100 * 1.00 = 100.00
     * subtotal: 355.00
     * platinum 15% off: 355.00 * 0.15 = 53.25 -> discounted 301.75
     * tax 10%: 30.18 (HALF_UP)
     * total: 331.93
     */
    @Test
    fun `holiday plus weekend plus platinum loyalty stack correctly`() {
        val quote = calculator.quote(
            price = price,
            checkIn = LocalDate.of(2026, 7, 3),
            checkOut = LocalDate.of(2026, 7, 6),
            loyaltyTier = "platinum" // case-insensitive
        )

        assertEquals(3, quote.nights)
        assertEquals(BigDecimal("301.75"), quote.amount)
        assertEquals(BigDecimal("53.25"), quote.discount)
        assertEquals(BigDecimal("30.18"), quote.taxes)
        assertEquals(BigDecimal("331.93"), quote.totalAmount)
    }

    @Test
    fun `unknown loyalty tier falls back to no discount`() {
        val quote = calculator.quote(
            price = price,
            checkIn = LocalDate.of(2026, 7, 7),
            checkOut = LocalDate.of(2026, 7, 8),
            loyaltyTier = "DIAMOND_DOES_NOT_EXIST"
        )
        assertEquals(BigDecimal("0.00"), quote.discount)
    }

    @Test
    fun `checkOut before checkIn is rejected`() {
        assertThrows<IllegalArgumentException> {
            calculator.quote(
                price = price,
                checkIn = LocalDate.of(2026, 7, 8),
                checkOut = LocalDate.of(2026, 7, 7)
            )
        }
    }
}
