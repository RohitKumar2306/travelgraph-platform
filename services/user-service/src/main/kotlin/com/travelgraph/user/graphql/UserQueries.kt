package com.travelgraph.user.graphql

import com.expediagroup.graphql.generator.annotations.GraphQLDescription
import com.expediagroup.graphql.generator.scalars.ID
import com.expediagroup.graphql.server.operations.Query
import com.travelgraph.user.auth.UserContextHolder
import graphql.schema.DataFetchingEnvironment
import org.springframework.stereotype.Component
import java.util.UUID
import java.util.concurrent.CompletableFuture

@Component
class UserQueries(private val userContextHolder: UserContextHolder) : Query {

    @GraphQLDescription("Look up a user by id. Returns null when not found.")
    fun user(env: DataFetchingEnvironment, id: ID): CompletableFuture<UserView?> {
        val loader = env.getDataLoader<UUID, UserView?>(UserDataLoader.NAME)
            ?: error("UserDataLoader not registered")
        val uuid = runCatching { UUID.fromString(id.value) }.getOrNull()
            ?: return CompletableFuture.completedFuture(null)
        return loader.load(uuid)
    }

    @GraphQLDescription(
        "The currently-identified user, derived from the verified router identity context. " +
            "Returns null for anonymous requests or if the id doesn't match a known user."
    )
    fun me(env: DataFetchingEnvironment): CompletableFuture<UserView?> {
        val userId = userContextHolder.current().takeUnless { it.isAnonymous }?.userId
            ?: return CompletableFuture.completedFuture(null)
        val uuid = runCatching { UUID.fromString(userId) }.getOrNull()
            ?: return CompletableFuture.completedFuture(null)
        val loader = env.getDataLoader<UUID, UserView?>(UserDataLoader.NAME)
            ?: error("UserDataLoader not registered")
        return loader.load(uuid)
    }
}
