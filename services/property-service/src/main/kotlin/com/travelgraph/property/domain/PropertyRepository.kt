package com.travelgraph.property.domain

import org.springframework.data.domain.Pageable
import org.springframework.data.jpa.repository.JpaRepository
import org.springframework.data.jpa.repository.Query
import org.springframework.data.repository.query.Param
import java.util.UUID

interface PropertyRepository : JpaRepository<Property, UUID> {

    fun findAllByCityIgnoreCaseOrderByRatingDesc(city: String, pageable: Pageable): List<Property>

    @Query("SELECT p FROM Property p WHERE p.id IN :ids")
    fun findAllByIds(@Param("ids") ids: Collection<UUID>): List<Property>
}
