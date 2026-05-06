package com.travelgraph.review.service

import com.travelgraph.review.domain.Review
import com.travelgraph.review.domain.ReviewRepository
import com.travelgraph.review.graphql.DuplicateReviewError
import com.travelgraph.review.graphql.ReviewValidationError
import com.travelgraph.review.graphql.ReviewView
import io.mockk.every
import io.mockk.mockk
import io.mockk.slot
import org.junit.jupiter.api.Test
import org.springframework.dao.DataIntegrityViolationException
import java.util.UUID
import kotlin.test.assertEquals
import kotlin.test.assertIs

class ReviewServiceTest {

    private val propertyId = UUID.fromString("11111111-1111-1111-1111-000000000001")
    private val userA = UUID.fromString("80000000-0000-0000-0000-000000000001")

    private fun newRepo(): ReviewRepository {
        val repo = mockk<ReviewRepository>()
        val store = mutableMapOf<UUID, Review>()
        every { repo.findByPropertyIdAndUserId(any(), any()) } answers {
            val p = firstArg<UUID>(); val u = secondArg<UUID>()
            store.values.firstOrNull { it.propertyId == p && it.userId == u }
        }
        val slot = slot<Review>()
        every { repo.saveAndFlush(capture(slot)) } answers {
            val r = slot.captured
            if (store.values.any { it.propertyId == r.propertyId && it.userId == r.userId && it.id != r.id }) {
                throw DataIntegrityViolationException("uq_reviews_property_user")
            }
            store[r.id] = r
            r
        }
        every { repo.aggregateByPropertyIds(any()) } answers {
            val ids = firstArg<Collection<UUID>>().toSet()
            store.values
                .filter { it.propertyId in ids }
                .groupBy { it.propertyId }
                .map { (pid, list) ->
                    object : ReviewRepository.ReviewAggregateRow {
                        override val propertyId: UUID = pid
                        override val count: Long = list.size.toLong()
                        override val averageRating: Double = list.map { it.rating.toDouble() }.average()
                    }
                }
        }
        return repo
    }

    @Test
    fun `rating below 1 returns ReviewValidationError`() {
        val svc = ReviewService(newRepo())
        val result = svc.add(propertyId, userA, rating = 0, comment = "ok")
        val err = assertIs<ReviewValidationError>(result)
        assertEquals("rating", err.field)
    }

    @Test
    fun `rating above 5 returns ReviewValidationError`() {
        val svc = ReviewService(newRepo())
        val result = svc.add(propertyId, userA, rating = 6, comment = "ok")
        val err = assertIs<ReviewValidationError>(result)
        assertEquals("rating", err.field)
    }

    @Test
    fun `blank comment returns ReviewValidationError`() {
        val svc = ReviewService(newRepo())
        val result = svc.add(propertyId, userA, rating = 5, comment = "   ")
        val err = assertIs<ReviewValidationError>(result)
        assertEquals("comment", err.field)
    }

    @Test
    fun `second review for same user+property returns DuplicateReviewError`() {
        val repo = newRepo()
        val svc = ReviewService(repo)

        val first = svc.add(propertyId, userA, 5, "great")
        val firstView = assertIs<ReviewView>(first)

        val second = svc.add(propertyId, userA, 4, "still great")
        val err = assertIs<DuplicateReviewError>(second)
        assertEquals(firstView.id, err.existingReviewId)
    }

    @Test
    fun `summary aggregates count and average rating correctly`() {
        val repo = newRepo()
        val svc = ReviewService(repo)
        svc.add(propertyId, userA, 5, "ok")
        svc.add(propertyId, UUID.fromString("80000000-0000-0000-0000-000000000002"), 3, "ok")
        svc.add(propertyId, UUID.fromString("80000000-0000-0000-0000-000000000003"), 4, "ok")

        val summary = svc.summaryFor(propertyId)
        assertEquals(3, summary.count)
        assertEquals(4.0, summary.averageRating, 0.0001)
    }
}

private fun assertEquals(expected: Double, actual: Double, delta: Double) {
    require(kotlin.math.abs(expected - actual) <= delta) { "Expected $expected ± $delta, got $actual" }
}
