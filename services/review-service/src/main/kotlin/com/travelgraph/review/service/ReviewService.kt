package com.travelgraph.review.service

import com.expediagroup.graphql.generator.scalars.ID
import com.travelgraph.review.domain.Review
import com.travelgraph.review.domain.ReviewRepository
import com.travelgraph.review.graphql.AddReviewPayload
import com.travelgraph.review.graphql.DuplicateReviewError
import com.travelgraph.review.graphql.ReviewSummary
import com.travelgraph.review.graphql.ReviewValidationError
import com.travelgraph.review.graphql.toView
import org.springframework.dao.DataIntegrityViolationException
import org.springframework.stereotype.Service
import org.springframework.transaction.annotation.Transactional
import java.time.Instant
import java.util.UUID

@Service
class ReviewService(private val repository: ReviewRepository) {

    @Transactional
    fun add(propertyId: UUID, userId: UUID, rating: Int, comment: String): AddReviewPayload {
        if (rating !in MIN_RATING..MAX_RATING) {
            return ReviewValidationError(
                field = "rating",
                message = "rating must be between $MIN_RATING and $MAX_RATING (was $rating)."
            )
        }
        if (comment.isBlank()) {
            return ReviewValidationError(
                field = "comment",
                message = "comment must not be empty."
            )
        }

        // Pre-flight duplicate check is the fast path; the unique constraint
        // below is what catches the concurrent-insert race.
        repository.findByPropertyIdAndUserId(propertyId, userId)?.let { existing ->
            return DuplicateReviewError(
                propertyId = ID(propertyId.toString()),
                userId = ID(userId.toString()),
                existingReviewId = ID(existing.id.toString()),
                message = "User has already reviewed this property."
            )
        }

        val review = Review(
            id = UUID.randomUUID(),
            propertyId = propertyId,
            userId = userId,
            rating = rating,
            comment = comment.trim(),
            createdAt = Instant.now()
        )

        return try {
            repository.saveAndFlush(review).toView()
        } catch (_: DataIntegrityViolationException) {
            val existing = repository.findByPropertyIdAndUserId(propertyId, userId)
            DuplicateReviewError(
                propertyId = ID(propertyId.toString()),
                userId = ID(userId.toString()),
                existingReviewId = ID((existing?.id ?: UUID(0, 0)).toString()),
                message = "User has already reviewed this property."
            )
        }
    }

    fun summaryFor(propertyId: UUID): ReviewSummary {
        val rows = repository.aggregateByPropertyIds(listOf(propertyId))
        val row = rows.firstOrNull()
        return ReviewSummary(
            propertyId = ID(propertyId.toString()),
            count = row?.count?.toInt() ?: 0,
            averageRating = row?.averageRating ?: 0.0
        )
    }

    companion object {
        const val MIN_RATING = 1
        const val MAX_RATING = 5
    }
}
