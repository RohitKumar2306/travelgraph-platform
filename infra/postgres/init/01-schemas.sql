-- Bootstrap per-service schemas inside a single shared Postgres instance.
-- Each subgraph owns its schema; Flyway runs migrations against its own schema only.
-- This mirrors a real federated platform where each service owns its data.

CREATE SCHEMA IF NOT EXISTS property_schema;
CREATE SCHEMA IF NOT EXISTS pricing_schema;
CREATE SCHEMA IF NOT EXISTS booking_schema;
CREATE SCHEMA IF NOT EXISTS user_schema;
CREATE SCHEMA IF NOT EXISTS review_schema;

GRANT ALL ON SCHEMA property_schema TO travelgraph;
GRANT ALL ON SCHEMA pricing_schema  TO travelgraph;
GRANT ALL ON SCHEMA booking_schema  TO travelgraph;
GRANT ALL ON SCHEMA user_schema     TO travelgraph;
GRANT ALL ON SCHEMA review_schema   TO travelgraph;
