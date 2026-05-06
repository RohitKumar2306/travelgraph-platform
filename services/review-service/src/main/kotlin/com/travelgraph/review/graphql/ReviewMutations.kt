package com.travelgraph.review.graphql

import com.expediagroup.graphql.generator.annotations.GraphQLDescription
import com.expediagroup.graphql.server.operations.Mutation
import com.travelgraph.review.service.ReviewService
import org.springframework.stereotype.Component
import java.util.UUID

@Component
class ReviewMutations(private val service: ReviewService) : Mutation {

    @GraphQLDescription(
        "Add a review for a property. One review per (user, property) is enforced. " +
            "Returns a typed payload union: Review on success, ReviewValidationError or " +
            "DuplicateReviewError on failure."
    )
    fun addReview(input: AddReviewInput): AddReviewPayload =
        service.add(
            propertyId = UUID.fromString(input.propertyId.value),
            userId = UUID.fromString(input.userId.value),
            rating = input.rating,
            comment = input.comment
        )
}
