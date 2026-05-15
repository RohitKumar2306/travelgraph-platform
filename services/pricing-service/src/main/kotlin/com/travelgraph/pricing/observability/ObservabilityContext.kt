package com.travelgraph.pricing.observability

import org.springframework.stereotype.Component

data class ObservabilityContext(val clientName: String, val operationName: String, val userId: String) {
    companion object {
        fun defaults(): ObservabilityContext = ObservabilityContext("__anonymous__", "<anonymous>", "anonymous")
    }
}

@Component
class ObservabilityContextHolder {
    private val current = ThreadLocal.withInitial { ObservabilityContext.defaults() }
    fun current(): ObservabilityContext = current.get()
    internal fun withContext(context: ObservabilityContext): AutoCloseable {
        val previous = current.get()
        current.set(context)
        return AutoCloseable { current.set(previous) }
    }
}
