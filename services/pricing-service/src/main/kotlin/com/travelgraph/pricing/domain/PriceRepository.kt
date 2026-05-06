package com.travelgraph.pricing.domain

import org.springframework.data.jpa.repository.JpaRepository
import org.springframework.data.jpa.repository.Query
import org.springframework.data.repository.query.Param
import java.util.UUID

interface PriceRepository : JpaRepository<Price, UUID> {

    fun findByPropertyId(propertyId: UUID): Price?

    @Query("SELECT p FROM Price p WHERE p.propertyId IN :ids")
    fun findAllByPropertyIds(@Param("ids") ids: Collection<UUID>): List<Price>
}
