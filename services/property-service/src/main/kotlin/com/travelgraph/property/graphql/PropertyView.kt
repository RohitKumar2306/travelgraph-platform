package com.travelgraph.property.graphql

import com.expediagroup.graphql.generator.annotations.GraphQLDescription
import com.expediagroup.graphql.generator.annotations.GraphQLName
import com.expediagroup.graphql.generator.federation.directives.FieldSet
import com.expediagroup.graphql.generator.federation.directives.KeyDirective
import com.expediagroup.graphql.generator.scalars.ID
import com.travelgraph.property.domain.Property

/**
 * GraphQL-facing projection of [Property].
 *
 * Kept separate from the JPA entity so:
 *  - Hibernate proxies / lazy-init don't leak into schema generation.
 *  - The over-the-wire shape can evolve independently of the table layout.
 *  - `id` is exposed as the GraphQL `ID!` scalar via graphql-kotlin's
 *    [com.expediagroup.graphql.generator.scalars.ID] wrapper.
 *
 * Federation: this subgraph **owns** the `Property` entity, so we annotate
 * it with `@key(fields: "id")`. Other subgraphs (pricing, review) extend
 * `Property` with their own fields by re-declaring the type with the same
 * `@key` and an `@extends` marker.
 */
@KeyDirective(fields = FieldSet("id"))
@GraphQLName("Property")
@GraphQLDescription("A bookable property in the TravelGraph catalog.")
data class PropertyView(
    @GraphQLDescription("Globally unique property identifier.")
    val id: ID,
    @GraphQLDescription("Human-readable property name.")
    val name: String,
    @GraphQLDescription("Long-form marketing description.")
    val description: String,
    @GraphQLDescription("Street address or specific location label.")
    val location: String,
    @GraphQLDescription("City the property is located in.")
    val city: String,
    @GraphQLDescription("Country the property is located in.")
    val country: String,
    @GraphQLDescription("Average guest rating, 0.0 - 5.0.")
    val rating: Float,
    @GraphQLDescription("Tagged amenities offered at this property.")
    val amenities: List<String>
)

fun Property.toView(): PropertyView = PropertyView(
    id = ID(id.toString()),
    name = name,
    description = description,
    location = location,
    city = city,
    country = country,
    rating = rating,
    amenities = amenities.sorted()
)
