package com.travelgraph.review.graphql

import com.expediagroup.graphql.generator.annotations.GraphQLDescription
import com.expediagroup.graphql.generator.scalars.ID

@GraphQLDescription("Result of attempting to add a review.")
sealed interface AddReviewPayload

@GraphQLDescription("Returned when the review failed schema validation (e.g. rating out of range, empty comment).")
data class ReviewValidationError(
    @GraphQLDescription("Field that failed validation.")
    val field: String,
    @GraphQLDescription("Human-readable explanation.")
    val message: String
) : AddReviewPayload

@GraphQLDescription("Returned when the user already has a review for this property.")
data class DuplicateReviewError(
    @GraphQLDescription("Property the existing review is for.")
    val propertyId: ID,
    @GraphQLDescription("User that already reviewed.")
    val userId: ID,
    @GraphQLDescription("Id of the existing review.")
    val existingReviewId: ID,
    @GraphQLDescription("Human-readable explanation.")
    val message: String
) : AddReviewPayload

@GraphQLDescription("Returned when a review mutation is attempted without an authenticated identity.")
data class AuthenticationRequiredError(
    @GraphQLDescription("Human-readable explanation.")
    val message: String = "authentication required"
) : AddReviewPayload

@GraphQLDescription("Aggregate review statistics for a property.")
data class ReviewSummary(
    @GraphQLDescription("Property the summary is for.")
    val propertyId: ID,
    @GraphQLDescription("Total number of reviews.")
    val count: Int,
    @GraphQLDescription("Mean rating across all reviews. 0.0 if no reviews exist.")
    val averageRating: Double
)
