package com.travelgraph.review.graphql

import com.expediagroup.graphql.generator.annotations.GraphQLDescription
import com.expediagroup.graphql.generator.scalars.ID

@GraphQLDescription("Input for addReview.")
data class AddReviewInput(
    @GraphQLDescription("Property being reviewed.")
    val propertyId: ID,
    @GraphQLDescription("Author of the review.")
    val userId: ID,
    @GraphQLDescription("Star rating, must be 1..5.")
    val rating: Int,
    @GraphQLDescription("Free-form comment, must be non-empty.")
    val comment: String
)
