package com.travelgraph.review.graphql

import com.expediagroup.graphql.dataloader.KotlinDataLoader
import com.travelgraph.review.domain.ReviewRepository
import graphql.GraphQLContext
import org.dataloader.BatchLoader
import org.dataloader.DataLoader
import org.dataloader.DataLoaderFactory
import org.dataloader.DataLoaderOptions
import org.springframework.stereotype.Component
import java.util.UUID
import java.util.concurrent.CompletableFuture

/**
 * Batches review lookups by propertyId. Returns an ordered list of reviews
 * for each property id; resolvers that fan out across many properties get
 * a single SQL roundtrip.
 */
@Component
class ReviewByPropertyDataLoader(
    private val repository: ReviewRepository
) : KotlinDataLoader<UUID, List<ReviewView>> {

    override val dataLoaderName: String = NAME

    override fun getDataLoader(graphQLContext: GraphQLContext): DataLoader<UUID, List<ReviewView>> {
        val batch = BatchLoader<UUID, List<ReviewView>> { propertyIds ->
            CompletableFuture.supplyAsync {
                val byProperty = repository.findAllByPropertyIds(propertyIds)
                    .groupBy { it.propertyId }
                propertyIds.map { id -> byProperty[id].orEmpty().sortedByDescending { it.createdAt }.map { it.toView() } }
            }
        }
        return DataLoaderFactory.newDataLoader(batch, DataLoaderOptions.newOptions())
    }

    companion object {
        const val NAME = "ReviewByPropertyDataLoader"
    }
}
