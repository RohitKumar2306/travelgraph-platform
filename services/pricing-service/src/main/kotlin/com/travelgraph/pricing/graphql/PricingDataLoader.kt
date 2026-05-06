package com.travelgraph.pricing.graphql

import com.expediagroup.graphql.dataloader.KotlinDataLoader
import com.travelgraph.pricing.domain.Price
import com.travelgraph.pricing.domain.PriceRepository
import graphql.GraphQLContext
import org.dataloader.BatchLoader
import org.dataloader.DataLoader
import org.dataloader.DataLoaderFactory
import org.dataloader.DataLoaderOptions
import org.springframework.stereotype.Component
import java.util.UUID
import java.util.concurrent.CompletableFuture

/**
 * Batch-loads [Price] rows by propertyId. Required for federation: when the
 * router resolves an `_entities` query for many `Property`s with a `price`
 * field, the loader collapses N round-trips into one.
 */
@Component
class PricingDataLoader(
    private val repository: PriceRepository
) : KotlinDataLoader<UUID, Price?> {

    override val dataLoaderName: String = NAME

    override fun getDataLoader(graphQLContext: GraphQLContext): DataLoader<UUID, Price?> {
        val batchLoader = BatchLoader<UUID, Price?> { propertyIds ->
            CompletableFuture.supplyAsync {
                val byPropertyId = repository
                    .findAllByPropertyIds(propertyIds)
                    .associateBy { it.propertyId }
                propertyIds.map { id -> byPropertyId[id] }
            }
        }
        return DataLoaderFactory.newDataLoader(batchLoader, DataLoaderOptions.newOptions())
    }

    companion object {
        const val NAME = "PricingDataLoader"
    }
}
