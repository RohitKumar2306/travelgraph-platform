package com.travelgraph.review.graphql

import com.travelgraph.review.service.ReviewService
import org.springframework.stereotype.Component

/**
 * Spring -> static bridge for [ReviewService]. See
 * `pricing-service/PricingBeans.kt` for the rationale; the same pattern
 * lets [PropertyExtension] reach the application's review service from a
 * graphql-kotlin field method without piping it through `GraphQLContext`.
 */
@Component
class ReviewBeans(injected: ReviewService) {
    init {
        service = injected
    }

    companion object {
        @Volatile
        var service: ReviewService? = null
            private set
    }
}
