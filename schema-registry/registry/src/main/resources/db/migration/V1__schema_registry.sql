CREATE TABLE schema_versions (
    id           BIGSERIAL PRIMARY KEY,
    service_name VARCHAR(128) NOT NULL,
    version      VARCHAR(128) NOT NULL,
    owner_team   VARCHAR(128) NOT NULL,
    sdl          TEXT NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT uq_schema_service_version UNIQUE (service_name, version)
);

CREATE INDEX idx_schema_versions_service_created ON schema_versions (service_name, created_at DESC);

CREATE TABLE supergraph_snapshots (
    id         BIGSERIAL PRIMARY KEY,
    sdl        TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE field_usage_events (
    id             BIGSERIAL PRIMARY KEY,
    service_name   VARCHAR(128) NOT NULL,
    type_name      VARCHAR(128) NOT NULL,
    field_name     VARCHAR(128) NOT NULL,
    field_path     VARCHAR(512) NOT NULL,
    operation_name VARCHAR(256) NOT NULL,
    client_name    VARCHAR(256) NOT NULL,
    client_version VARCHAR(128) NOT NULL,
    occurred_at    TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_field_usage_lookup ON field_usage_events (service_name, type_name, field_name, occurred_at DESC);
