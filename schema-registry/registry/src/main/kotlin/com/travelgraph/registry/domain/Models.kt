package com.travelgraph.registry.domain

import jakarta.persistence.Column
import jakarta.persistence.Entity
import jakarta.persistence.GeneratedValue
import jakarta.persistence.GenerationType
import jakarta.persistence.Id
import jakarta.persistence.Table
import java.time.OffsetDateTime

@Entity
@Table(name = "schema_versions")
class SchemaVersion(
    @Id @GeneratedValue(strategy = GenerationType.IDENTITY)
    var id: Long? = null,
    @Column(name = "service_name", nullable = false)
    var serviceName: String,
    @Column(nullable = false)
    var version: String,
    @Column(name = "owner_team", nullable = false)
    var ownerTeam: String,
    @Column(nullable = false, columnDefinition = "TEXT")
    var sdl: String,
    @Column(name = "created_at", nullable = false)
    var createdAt: OffsetDateTime = OffsetDateTime.now()
)

@Entity
@Table(name = "supergraph_snapshots")
class SupergraphSnapshot(
    @Id @GeneratedValue(strategy = GenerationType.IDENTITY)
    var id: Long? = null,
    @Column(nullable = false, columnDefinition = "TEXT")
    var sdl: String,
    @Column(name = "created_at", nullable = false)
    var createdAt: OffsetDateTime = OffsetDateTime.now()
)

@Entity
@Table(name = "field_usage_events")
class FieldUsageEvent(
    @Id @GeneratedValue(strategy = GenerationType.IDENTITY)
    var id: Long? = null,
    @Column(name = "service_name", nullable = false)
    var serviceName: String,
    @Column(name = "type_name", nullable = false)
    var typeName: String,
    @Column(name = "field_name", nullable = false)
    var fieldName: String,
    @Column(name = "field_path", nullable = false)
    var fieldPath: String,
    @Column(name = "operation_name", nullable = false)
    var operationName: String,
    @Column(name = "client_name", nullable = false)
    var clientName: String,
    @Column(name = "client_version", nullable = false)
    var clientVersion: String,
    @Column(name = "occurred_at", nullable = false)
    var occurredAt: OffsetDateTime = OffsetDateTime.now()
)
