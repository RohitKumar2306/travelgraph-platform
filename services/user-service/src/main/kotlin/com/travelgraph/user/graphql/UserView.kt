package com.travelgraph.user.graphql

import com.expediagroup.graphql.generator.annotations.GraphQLDescription
import com.expediagroup.graphql.generator.annotations.GraphQLName
import com.expediagroup.graphql.generator.federation.directives.FieldSet
import com.expediagroup.graphql.generator.federation.directives.KeyDirective
import com.expediagroup.graphql.generator.scalars.ID
import com.travelgraph.user.domain.LoyaltyStatus
import com.travelgraph.user.domain.User

@KeyDirective(fields = FieldSet("id"))
@GraphQLName("User")
@GraphQLDescription("A guest of the platform, with loyalty status and currency preference.")
data class UserView(
    @GraphQLDescription("Globally unique user identifier.")
    val id: ID,
    @GraphQLDescription("Display name.")
    val name: String,
    @GraphQLDescription("Login email. Unique platform-wide.")
    val email: String,
    @GraphQLDescription("Loyalty tier the user is on right now.")
    val loyaltyStatus: LoyaltyStatus,
    @GraphQLDescription("ISO-4217 currency the user prefers to see prices in.")
    val preferredCurrency: String,
    @GraphQLDescription("Properties the user has saved for later.")
    val savedPropertyIds: List<ID>
)

fun User.toView(): UserView = UserView(
    id = ID(id.toString()),
    name = name,
    email = email,
    loyaltyStatus = loyaltyStatus,
    preferredCurrency = preferredCurrency,
    savedPropertyIds = savedPropertyIds.map { ID(it.toString()) }
)
