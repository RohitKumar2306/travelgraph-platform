package com.travelgraph.review.auth

import org.springframework.http.HttpStatus
import org.springframework.stereotype.Component
import org.springframework.web.server.ResponseStatusException

data class UserContext(
    val userId: String,
    val loyaltyTier: String,
    val email: String
) {
    val isAnonymous: Boolean
        get() = userId == ANONYMOUS_USER_ID

    fun requireAuthenticated(): UserContext {
        if (isAnonymous) {
            throw ResponseStatusException(HttpStatus.UNAUTHORIZED, "authentication required")
        }
        return this
    }

    companion object {
        const val ANONYMOUS_USER_ID = "anonymous"

        fun anonymous(): UserContext = UserContext(
            userId = ANONYMOUS_USER_ID,
            loyaltyTier = "anonymous",
            email = ""
        )
    }
}

@Component
class UserContextHolder {
    private val current = ThreadLocal.withInitial { UserContext.anonymous() }

    fun current(): UserContext = current.get()

    internal fun withContext(userContext: UserContext): AutoCloseable {
        val previous = current.get()
        current.set(userContext)
        return AutoCloseable { current.set(previous) }
    }
}
