package com.travelgraph.user.observability

import io.micrometer.core.instrument.MeterRegistry
import io.micrometer.core.instrument.DistributionSummary
import org.aspectj.lang.ProceedingJoinPoint
import org.aspectj.lang.annotation.Around
import org.aspectj.lang.annotation.Aspect
import org.springframework.stereotype.Component

@Aspect
@Component
class RepositoryMetricsAspect(private val meterRegistry: MeterRegistry, private val contextHolder: ObservabilityContextHolder) {
    @Around("execution(* com.travelgraph.user.domain..*Repository.*(..))")
    fun timeRepositoryCall(joinPoint: ProceedingJoinPoint): Any? {
        val start = System.nanoTime()
        return try {
            joinPoint.proceed()
        } finally {
            val context = contextHolder.current()
            DistributionSummary.builder("user_db_query_duration_ms").tag("client_name", context.clientName).tag("operation_name", context.operationName).tag("repository", joinPoint.signature.declaringType.simpleName).tag("method", joinPoint.signature.name).publishPercentileHistogram().register(meterRegistry).record((System.nanoTime() - start) / 1_000_000.0)
        }
    }
}
