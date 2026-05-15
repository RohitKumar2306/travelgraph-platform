package com.travelgraph.registry.domain

import org.springframework.data.jpa.repository.JpaRepository
import org.springframework.data.jpa.repository.Query
import java.time.OffsetDateTime

interface SchemaVersionRepository : JpaRepository<SchemaVersion, Long> {
    fun findFirstByServiceNameOrderByCreatedAtDesc(serviceName: String): SchemaVersion?
    fun findTop2ByServiceNameOrderByCreatedAtDesc(serviceName: String): List<SchemaVersion>
    fun findAllByServiceNameOrderByCreatedAtDesc(serviceName: String): List<SchemaVersion>

    @Query(
        """
        select s from SchemaVersion s
        where s.createdAt in (
          select max(s2.createdAt) from SchemaVersion s2 group by s2.serviceName
        )
        """
    )
    fun latestForEveryService(): List<SchemaVersion>
}

interface SupergraphSnapshotRepository : JpaRepository<SupergraphSnapshot, Long> {
    fun findFirstByOrderByCreatedAtDesc(): SupergraphSnapshot?
}

interface FieldUsageEventRepository : JpaRepository<FieldUsageEvent, Long> {
    @Query(
        """
        select e.clientName as clientName, e.clientVersion as clientVersion, count(e) as count
        from FieldUsageEvent e
        where e.serviceName = :serviceName
          and e.typeName = :typeName
          and e.fieldName = :fieldName
          and e.occurredAt >= :since
        group by e.clientName, e.clientVersion
        order by count(e) desc
        """
    )
    fun usageByClient(serviceName: String, typeName: String, fieldName: String, since: OffsetDateTime): List<FieldUsageCount>

    fun countByServiceNameAndTypeNameAndFieldNameAndOccurredAtAfter(
        serviceName: String,
        typeName: String,
        fieldName: String,
        occurredAt: OffsetDateTime
    ): Long
}

interface FieldUsageCount {
    val clientName: String
    val clientVersion: String
    val count: Long
}
