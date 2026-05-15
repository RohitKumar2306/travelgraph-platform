package com.travelgraph.booking.observability

import io.micrometer.core.instrument.MeterRegistry
import org.slf4j.MDC
import org.springframework.stereotype.Component
import org.springframework.web.server.ServerWebExchange
import org.springframework.web.server.WebFilter
import org.springframework.web.server.WebFilterChain
import reactor.core.publisher.Mono

@Component
class ObservabilityFilter(private val meterRegistry: MeterRegistry, private val contextHolder: ObservabilityContextHolder) : WebFilter {
    override fun filter(exchange: ServerWebExchange, chain: WebFilterChain): Mono<Void> {
        if (!exchange.request.path.value().endsWith("/graphql")) return chain.filter(exchange)
        val context = ObservabilityContext(exchange.request.headers.getFirst("apollographql-client-name") ?: "__anonymous__", exchange.request.headers.getFirst("x-operation-name") ?: "<anonymous>", exchange.request.headers.getFirst("x-user-id") ?: "anonymous")
        MDC.put("client_name", context.clientName)
        MDC.put("operation_name", context.operationName)
        MDC.put("user_id", context.userId)
        val scope = contextHolder.withContext(context)
        return chain.filter(exchange).doFinally {
            scope.close()
            MDC.remove("client_name")
            MDC.remove("operation_name")
            MDC.remove("user_id")
            meterRegistry.counter("booking_requests_total", "client_name", context.clientName, "operation_name", context.operationName, "status", exchange.response.statusCode?.value()?.toString() ?: "200").increment()
        }
    }
}
