package com.travelgraph.booking.config

import org.springframework.boot.context.properties.ConfigurationProperties

/**
 * Static room inventory configuration. In a real system this would live in
 * its own subgraph or a remote inventory service; for the demo it is enough
 * to surface the shape of the data without hand-rolling a separate store.
 */
@ConfigurationProperties(prefix = "travelgraph.booking")
data class BookingProperties(
    val inventory: List<RoomInventory> = emptyList()
) {
    data class RoomInventory(
        val code: String,
        val name: String,
        val capacity: Int,
        val total: Int
    )
}
