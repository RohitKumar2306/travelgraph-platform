package com.travelgraph.property.graphql

import com.expediagroup.graphql.dataloader.KotlinDataLoader
import com.travelgraph.property.domain.PropertyRepository
import graphql.GraphQLContext
import org.dataloader.BatchLoader
import org.dataloader.DataLoader
import org.dataloader.DataLoaderFactory
import org.dataloader.DataLoaderOptions
import org.springframework.stereotype.Component
import java.util.UUID
import java.util.concurrent.CompletableFuture

/**
 * Batch-loads [PropertyView] by id list to prevent N+1 queries when many
 * `Property` entities are resolved within a single GraphQL operation
 * (e.g. via `_entities` once federation is enabled, or when nested fields
 * fan out across multiple parents).
 */
@Component
class PropertyDataLoader(
    private val repository: PropertyRepository
) : KotlinDataLoader<UUID, PropertyView?> {

    override val dataLoaderName: String = NAME

    override fun getDataLoader(graphQLContext: GraphQLContext): DataLoader<UUID, PropertyView?> {
        val batchLoader = BatchLoader<UUID, PropertyView?> { ids ->
            CompletableFuture.supplyAsync {
                val byId = repository.findAllByIds(ids).associateBy { it.id }
                ids.map { id -> byId[id]?.toView() }
            }
        }
        return DataLoaderFactory.newDataLoader(batchLoader, DataLoaderOptions.newOptions())
    }

    companion object {
        const val NAME = "PropertyDataLoader"
    }
}
