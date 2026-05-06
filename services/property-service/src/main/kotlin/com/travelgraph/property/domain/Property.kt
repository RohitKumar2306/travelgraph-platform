package com.travelgraph.property.domain

import jakarta.persistence.CollectionTable
import jakarta.persistence.Column
import jakarta.persistence.ElementCollection
import jakarta.persistence.Entity
import jakarta.persistence.FetchType
import jakarta.persistence.Id
import jakarta.persistence.JoinColumn
import jakarta.persistence.Table
import java.util.UUID

/**
 * Persistent representation of a property in the catalog.
 *
 * Owned exclusively by the property subgraph - no other service writes here.
 * Other subgraphs (pricing, review) reference this entity by its [id] via
 * Apollo Federation `@key` directives once federation is wired up.
 */
@Entity
@Table(name = "properties")
class Property(
    @Id
    @Column(name = "id", nullable = false, updatable = false)
    var id: UUID = UUID.randomUUID(),

    @Column(name = "name", nullable = false)
    var name: String = "",

    @Column(name = "description", nullable = false, columnDefinition = "TEXT")
    var description: String = "",

    @Column(name = "location", nullable = false)
    var location: String = "",

    @Column(name = "city", nullable = false)
    var city: String = "",

    @Column(name = "country", nullable = false)
    var country: String = "",

    @Column(name = "rating", nullable = false)
    var rating: Float = 0.0f,

    @ElementCollection(fetch = FetchType.EAGER)
    @CollectionTable(
        name = "property_amenities",
        joinColumns = [JoinColumn(name = "property_id")]
    )
    @Column(name = "amenity")
    var amenities: MutableSet<String> = mutableSetOf()
)
