package com.travelgraph.pricing.service

import com.travelgraph.pricing.config.PricingProperties
import com.travelgraph.pricing.domain.Price
import org.springframework.stereotype.Service
import java.math.BigDecimal
import java.math.RoundingMode
import java.time.DayOfWeek
import java.time.LocalDate
import java.time.format.DateTimeFormatter
import java.time.format.DateTimeParseException

/**
 * Per-stay price calculation.
 *
 * Rules (applied in order, all configurable via [PricingProperties]):
 *
 *  1. **Weekend uplift** - Friday and Saturday nights are charged at
 *     `basePrice * (1 + weekendUplift)`.
 *  2. **Holiday uplift** - any night whose check-in date is in the configured
 *     holiday set gets `basePrice * (1 + holidayUplift)`. If a night is BOTH a
 *     weekend and a holiday the holiday uplift wins (it's the larger one and
 *     stacking would double-charge).
 *  3. **Loyalty discount** - applied as a percentage off the *subtotal*
 *     (sum of nightly rates) before tax. Unknown tiers fall back to 0%.
 *  4. **Tax** - `taxRate` applied to the post-discount subtotal. Tax is
 *     reported separately so clients can show it on the receipt.
 *
 * The output is the [com.travelgraph.pricing.graphql.PriceQuote] shape:
 * `amount` is the pre-tax, post-discount subtotal; `totalAmount` is what the
 * guest will be charged (`amount + taxes`).
 */
@Service
class PriceCalculator(private val rules: PricingProperties) {

    /**
     * @param checkIn  inclusive date of check-in (e.g. `2026-07-03`)
     * @param checkOut exclusive date of check-out (e.g. `2026-07-05` => 2 nights)
     * @param loyaltyTier optional loyalty tier ("SILVER", "GOLD", ...). Case-insensitive.
     */
    fun quote(
        price: Price,
        checkIn: LocalDate,
        checkOut: LocalDate,
        loyaltyTier: String? = null
    ): Quote {
        require(!checkOut.isBefore(checkIn)) { "checkOut ($checkOut) is before checkIn ($checkIn)" }

        val nights = nightsBetween(checkIn, checkOut)
        val holidaySet = rules.holidays.toHashSet()

        val subtotal = (0 until nights).fold(BigDecimal.ZERO) { acc, i ->
            val night = checkIn.plusDays(i.toLong())
            acc + nightlyRate(price.basePrice, night, holidaySet)
        }

        val discountRate = loyaltyTier?.let { tier ->
            rules.loyalty.entries.firstOrNull { it.key.equals(tier, ignoreCase = true) }?.value
        } ?: BigDecimal.ZERO
        val discount = subtotal.multiply(discountRate).setScale(2, RoundingMode.HALF_UP)
        val discounted = subtotal.subtract(discount)

        val taxes = discounted.multiply(price.taxRate).setScale(2, RoundingMode.HALF_UP)
        val total = discounted.add(taxes)

        return Quote(
            propertyId = price.propertyId,
            currency = price.currency,
            nights = nights,
            amount = discounted.setScale(2, RoundingMode.HALF_UP),
            taxes = taxes,
            discount = discount,
            totalAmount = total.setScale(2, RoundingMode.HALF_UP)
        )
    }

    private fun nightlyRate(base: BigDecimal, night: LocalDate, holidays: Set<LocalDate>): BigDecimal {
        // Holiday wins over weekend - they are not stacked on purpose.
        val multiplier = when {
            holidays.contains(night) -> BigDecimal.ONE.add(rules.holidayUplift)
            isWeekendNight(night)    -> BigDecimal.ONE.add(rules.weekendUplift)
            else                     -> BigDecimal.ONE
        }
        return base.multiply(multiplier).setScale(2, RoundingMode.HALF_UP)
    }

    private fun isWeekendNight(date: LocalDate): Boolean =
        date.dayOfWeek == DayOfWeek.FRIDAY || date.dayOfWeek == DayOfWeek.SATURDAY

    private fun nightsBetween(checkIn: LocalDate, checkOut: LocalDate): Int =
        java.time.temporal.ChronoUnit.DAYS.between(checkIn, checkOut).toInt()

    /**
     * Service-internal value object. Translated to the GraphQL `Price` shape
     * by the resolver layer.
     */
    data class Quote(
        val propertyId: java.util.UUID,
        val currency: String,
        val nights: Int,
        val amount: BigDecimal,
        val taxes: BigDecimal,
        val discount: BigDecimal,
        val totalAmount: BigDecimal
    )

    companion object {
        private val ISO_DATE = DateTimeFormatter.ISO_LOCAL_DATE

        /** Best-effort ISO-local-date parser. Returns `null` for unparseable input. */
        fun parseDateOrNull(value: String?): LocalDate? {
            if (value.isNullOrBlank()) return null
            return try {
                LocalDate.parse(value, ISO_DATE)
            } catch (_: DateTimeParseException) {
                null
            }
        }
    }
}
