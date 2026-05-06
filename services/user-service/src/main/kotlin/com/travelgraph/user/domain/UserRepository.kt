package com.travelgraph.user.domain

import org.springframework.data.jpa.repository.JpaRepository
import org.springframework.data.jpa.repository.Query
import org.springframework.data.repository.query.Param
import java.util.UUID

interface UserRepository : JpaRepository<User, UUID> {

    @Query("SELECT u FROM User u WHERE u.id IN :ids")
    fun findAllByIds(@Param("ids") ids: Collection<UUID>): List<User>
}
