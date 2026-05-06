package com.travelgraph.review.graphql

import com.expediagroup.graphql.generator.annotations.GraphQLDescription
import com.expediagroup.graphql.generator.annotations.GraphQLName
import com.expediagroup.graphql.generator.scalars.ID
import com.travelgraph.review.domain.Review

@GraphQLName("Review")
@GraphQLDescription("A guest review of a property.")
data class ReviewView(
    @GraphQLDescription("Globally unique review identifier.")
    val id: ID,
    @GraphQLDescription("Reviewed property.")
    val propertyId: ID,
    @GraphQLDescription("Author of the review.")
    val userId: ID,
    @GraphQLDescription("Star rating, 1..5.")
    val rating: Int,
    @GraphQLDescription("Free-form comment.")
    val comment: String,
    @GraphQLDescription("Server-side creation timestamp (RFC-3339).")
    val createdAt: String
) : AddReviewPayload

fun Review.toView(): ReviewView = ReviewView(
    id = ID(id.toString()),
    propertyId = ID(propertyId.toString()),
    userId = ID(userId.toString()),
    rating = rating,
    comment = comment,
    createdAt = createdAt.toString()
)
