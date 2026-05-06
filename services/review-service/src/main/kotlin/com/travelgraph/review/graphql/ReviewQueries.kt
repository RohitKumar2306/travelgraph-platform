package com.travelgraph.review.graphql

import com.expediagroup.graphql.generator.annotations.GraphQLDescription
import com.expediagroup.graphql.generator.scalars.ID
import com.expediagroup.graphql.server.operations.Query
import com.travelgraph.review.domain.ReviewRepository
import com.travelgraph.review.service.ReviewService
import org.springframework.data.domain.PageRequest
import org.springframework.stereotype.Component
import java.util.UUID

@Component
class ReviewQueries(
    private val repository: ReviewRepository,
    private val service: ReviewService
) : Query {

    @GraphQLDescription("Reviews for a property, most recent first.")
    fun reviews(
        @GraphQLDescription("Property to fetch reviews for.")
        propertyId: ID,
        @GraphQLDescription("Maximum number of reviews to return (1..100). Defaults to 10.")
        limit: Int = 10
    ): List<ReviewView> {
        val capped = limit.coerceIn(1, 100)
        return repository
            .findByPropertyIdOrderByCreatedAtDesc(UUID.fromString(propertyId.value), PageRequest.of(0, capped))
            .map { it.toView() }
    }

    @GraphQLDescription("Aggregate review summary (count + average rating) for a property.")
    fun reviewSummary(propertyId: ID): ReviewSummary =
        service.summaryFor(UUID.fromString(propertyId.value))
}
