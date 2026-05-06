package com.travelgraph.property.graphql

import com.expediagroup.graphql.generator.annotations.GraphQLDescription
import com.expediagroup.graphql.generator.scalars.ID
import com.expediagroup.graphql.server.operations.Query
import com.travelgraph.property.domain.PropertyRepository
import graphql.schema.DataFetchingEnvironment
import org.springframework.data.domain.PageRequest
import org.springframework.stereotype.Component
import java.util.UUID
import java.util.concurrent.CompletableFuture

/**
 * Top-level GraphQL queries exposed by the property subgraph.
 * graphql-kotlin auto-discovers any [Query] bean in the configured packages.
 */
@Component
class PropertyQueries(
    private val repository: PropertyRepository
) : Query {

    @GraphQLDescription("Look up a single property by its unique id.")
    fun property(env: DataFetchingEnvironment, id: ID): CompletableFuture<PropertyView?> {
        // Route through the DataLoader so that repeated calls within a single
        // operation (and, soon, federated `_entities` resolution) batch together.
        val loader = env.getDataLoader<UUID, PropertyView?>(PropertyDataLoader.NAME)
            ?: error("PropertyDataLoader not registered")
        return loader.load(UUID.fromString(id.value))
    }

    @GraphQLDescription("Search properties in a given city, ordered by rating descending.")
    fun searchProperties(
        @GraphQLDescription("City name. Match is case-insensitive.")
        city: String,
        @GraphQLDescription("Maximum number of results (1..100). Defaults to 20.")
        limit: Int = 20
    ): List<PropertyView> {
        val capped = limit.coerceIn(1, 100)
        return repository
            .findAllByCityIgnoreCaseOrderByRatingDesc(city, PageRequest.of(0, capped))
            .map { it.toView() }
    }
}
