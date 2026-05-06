package com.travelgraph.user.graphql

import com.expediagroup.graphql.generator.annotations.GraphQLDescription
import com.expediagroup.graphql.generator.scalars.ID
import com.expediagroup.graphql.server.operations.Query
import graphql.schema.DataFetchingEnvironment
import org.springframework.stereotype.Component
import java.util.UUID
import java.util.concurrent.CompletableFuture

@Component
class UserQueries : Query {

    @GraphQLDescription("Look up a user by id. Returns null when not found.")
    fun user(env: DataFetchingEnvironment, id: ID): CompletableFuture<UserView?> {
        val loader = env.getDataLoader<UUID, UserView?>(UserDataLoader.NAME)
            ?: error("UserDataLoader not registered")
        val uuid = runCatching { UUID.fromString(id.value) }.getOrNull()
            ?: return CompletableFuture.completedFuture(null)
        return loader.load(uuid)
    }

    @GraphQLDescription(
        "The currently-identified user, derived from the `x-user-id` request header. " +
            "Returns null if the header is missing or doesn't match a known user."
    )
    fun me(env: DataFetchingEnvironment): CompletableFuture<UserView?> {
        val userId = env.graphQlContext.get<String?>(UserContextFactory.USER_ID_KEY)
            ?: return CompletableFuture.completedFuture(null)
        val uuid = runCatching { UUID.fromString(userId) }.getOrNull()
            ?: return CompletableFuture.completedFuture(null)
        val loader = env.getDataLoader<UUID, UserView?>(UserDataLoader.NAME)
            ?: error("UserDataLoader not registered")
        return loader.load(uuid)
    }
}
