package com.travelgraph.pricing.graphql

import com.travelgraph.pricing.service.PriceCalculator
import org.springframework.stereotype.Component

/**
 * Spring -> static bridge for [PriceCalculator].
 *
 * Federation entity classes (e.g. [PropertyExtension]) are not Spring beans;
 * graphql-kotlin instantiates them per-`_entities` request via the
 * [FederatedTypePromiseResolver][com.expediagroup.graphql.generator.federation.execution.FederatedTypePromiseResolver].
 * Field methods on those classes still need access to the application's
 * services (here, the price calculator). The Spring-managed `GraphQLContext`
 * is the canonical place for this, but for a single-bean dependency the
 * static-bridge pattern keeps the resolver code small and easy to read.
 *
 * The bridge is initialized in this `@Component`'s constructor, which Spring
 * eagerly instantiates at startup, so [calculator] is always populated by
 * the time the GraphQL endpoint serves its first request.
 */
@Component
class PricingBeans(injected: PriceCalculator) {
    init {
        calculator = injected
    }

    companion object {
        @Volatile
        var calculator: PriceCalculator? = null
            private set
    }
}
