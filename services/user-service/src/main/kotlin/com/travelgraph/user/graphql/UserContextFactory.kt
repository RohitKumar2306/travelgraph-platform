package com.travelgraph.user.graphql

import com.expediagroup.graphql.server.spring.execution.DefaultSpringGraphQLContextFactory
import graphql.GraphQLContext
import org.springframework.stereotype.Component
import org.springframework.web.reactive.function.server.ServerRequest

/**
 * Pulls the caller's identity out of the `x-user-id` request header and puts
 * it in the per-operation [GraphQLContext]. The `me` resolver reads from
 * here.
 *
 * This is a STAND-IN for real auth. Phase 5 wires JWT validation at the
 * router edge and propagates a verified subject claim - at that point the
 * header source is replaced but the resolver-side API stays the same.
 *
 * We extend [DefaultSpringGraphQLContextFactory] (rather than the bare
 * `SpringGraphQLContextFactory`) so that federated-tracing context keys are
 * still populated when federation gets turned on in phase 3.
 */
@Component
class UserContextFactory : DefaultSpringGraphQLContextFactory() {

    override suspend fun generateContext(request: ServerRequest): GraphQLContext {
        val base = super.generateContext(request)
        val userId = request.headers().firstHeader(USER_ID_HEADER)?.takeIf { it.isNotBlank() }
            ?: return base
        return base.put(USER_ID_KEY, userId)
    }

    companion object {
        const val USER_ID_HEADER = "x-user-id"
        const val USER_ID_KEY = "userId"
    }
}
