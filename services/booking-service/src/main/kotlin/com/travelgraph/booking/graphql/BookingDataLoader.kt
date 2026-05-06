package com.travelgraph.booking.graphql

import com.expediagroup.graphql.dataloader.KotlinDataLoader
import com.travelgraph.booking.domain.BookingRepository
import graphql.GraphQLContext
import org.dataloader.BatchLoader
import org.dataloader.DataLoader
import org.dataloader.DataLoaderFactory
import org.dataloader.DataLoaderOptions
import org.springframework.stereotype.Component
import java.util.UUID
import java.util.concurrent.CompletableFuture

@Component
class BookingDataLoader(
    private val repository: BookingRepository
) : KotlinDataLoader<UUID, BookingView?> {

    override val dataLoaderName: String = NAME

    override fun getDataLoader(graphQLContext: GraphQLContext): DataLoader<UUID, BookingView?> {
        val batch = BatchLoader<UUID, BookingView?> { ids ->
            CompletableFuture.supplyAsync {
                val byId = repository.findAllByIds(ids).associateBy { it.id }
                ids.map { id -> byId[id]?.toView() }
            }
        }
        return DataLoaderFactory.newDataLoader(batch, DataLoaderOptions.newOptions())
    }

    companion object {
        const val NAME = "BookingDataLoader"
    }
}
