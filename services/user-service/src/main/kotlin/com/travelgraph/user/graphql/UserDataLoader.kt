package com.travelgraph.user.graphql

import com.expediagroup.graphql.dataloader.KotlinDataLoader
import com.travelgraph.user.domain.UserRepository
import graphql.GraphQLContext
import org.dataloader.BatchLoader
import org.dataloader.DataLoader
import org.dataloader.DataLoaderFactory
import org.dataloader.DataLoaderOptions
import org.springframework.stereotype.Component
import java.util.UUID
import java.util.concurrent.CompletableFuture

@Component
class UserDataLoader(
    private val repository: UserRepository
) : KotlinDataLoader<UUID, UserView?> {

    override val dataLoaderName: String = NAME

    override fun getDataLoader(graphQLContext: GraphQLContext): DataLoader<UUID, UserView?> {
        val batch = BatchLoader<UUID, UserView?> { ids ->
            CompletableFuture.supplyAsync {
                val byId = repository.findAllByIds(ids).associateBy { it.id }
                ids.map { id -> byId[id]?.toView() }
            }
        }
        return DataLoaderFactory.newDataLoader(batch, DataLoaderOptions.newOptions())
    }

    companion object {
        const val NAME = "UserDataLoader"
    }
}
