package com.travelgraph.pricing

import com.travelgraph.pricing.config.PricingProperties
import org.springframework.boot.autoconfigure.SpringBootApplication
import org.springframework.boot.context.properties.EnableConfigurationProperties
import org.springframework.boot.runApplication

@SpringBootApplication
@EnableConfigurationProperties(PricingProperties::class)
class PricingServiceApplication

fun main(args: Array<String>) {
    runApplication<PricingServiceApplication>(*args)
}
