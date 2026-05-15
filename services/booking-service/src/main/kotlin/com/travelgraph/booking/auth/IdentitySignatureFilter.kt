package com.travelgraph.booking.auth

import org.springframework.beans.factory.annotation.Value
import org.springframework.http.HttpStatus
import org.springframework.stereotype.Component
import org.springframework.web.server.ResponseStatusException
import org.springframework.web.server.ServerWebExchange
import org.springframework.web.server.WebFilter
import org.springframework.web.server.WebFilterChain
import reactor.core.publisher.Mono
import java.security.MessageDigest
import javax.crypto.Mac
import javax.crypto.spec.SecretKeySpec

@Component
class IdentitySignatureFilter(
    private val userContextHolder: UserContextHolder,
    @Value("\${travelgraph.identity.signature-secret:\${TRAVELGRAPH_IDENTITY_SECRET:travelgraph-dev-identity-secret}}")
    private val signatureSecret: String
) : WebFilter {

    override fun filter(exchange: ServerWebExchange, chain: WebFilterChain): Mono<Void> {
        if (!exchange.request.path.value().endsWith("/graphql")) {
            return chain.filter(exchange)
        }

        val userId = exchange.request.headers.getFirst(USER_ID_HEADER)
        val tier = exchange.request.headers.getFirst(USER_TIER_HEADER)
        val email = exchange.request.headers.getFirst(USER_EMAIL_HEADER) ?: ""
        val signature = exchange.request.headers.getFirst(SIGNATURE_HEADER)
        if (userId.isNullOrBlank() || tier.isNullOrBlank() || signature.isNullOrBlank()) {
            return Mono.error(unauthorized())
        }

        val expected = sign(userId, tier, email)
        if (!MessageDigest.isEqual(expected.toByteArray(), signature.toByteArray())) {
            return Mono.error(unauthorized())
        }

        val scope = userContextHolder.withContext(UserContext(userId, tier, email))
        return chain.filter(exchange).doFinally { scope.close() }
    }

    private fun sign(userId: String, tier: String, email: String): String {
        val mac = Mac.getInstance("HmacSHA256")
        mac.init(SecretKeySpec(signatureSecret.toByteArray(), "HmacSHA256"))
        return mac.doFinal("$userId\n$tier\n$email".toByteArray())
            .joinToString("") { "%02x".format(it.toInt() and 0xff) }
    }

    private fun unauthorized(): ResponseStatusException =
        ResponseStatusException(HttpStatus.UNAUTHORIZED, "invalid identity signature")

    companion object {
        const val USER_ID_HEADER = "x-user-id"
        const val USER_TIER_HEADER = "x-user-tier"
        const val USER_EMAIL_HEADER = "x-user-email"
        const val SIGNATURE_HEADER = "x-identity-signature"
    }
}
