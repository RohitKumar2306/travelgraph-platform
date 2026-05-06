-- V1: Bookings table.
-- idempotency_key is the linchpin: a UNIQUE constraint guarantees that even
-- under retries (network blips, mobile clients), the same key cannot create
-- two bookings - the service catches the violation and returns the existing row.

CREATE TABLE bookings (
    id              UUID            PRIMARY KEY,
    property_id     UUID            NOT NULL,
    user_id         UUID            NOT NULL,
    check_in        DATE            NOT NULL,
    check_out       DATE            NOT NULL,
    status          VARCHAR(16)     NOT NULL CHECK (status IN ('PENDING', 'CONFIRMED', 'CANCELLED')),
    total_amount    NUMERIC(12, 2)  NOT NULL CHECK (total_amount >= 0),
    currency        VARCHAR(3)      NOT NULL,
    idempotency_key VARCHAR(128)    NOT NULL UNIQUE,
    created_at      TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_window CHECK (check_out > check_in)
);

CREATE INDEX idx_bookings_property        ON bookings (property_id);
CREATE INDEX idx_bookings_user            ON bookings (user_id);
CREATE INDEX idx_bookings_property_window ON bookings (property_id, check_in, check_out);

-- Seed bookings: 5 confirmed bookings against the seeded properties + users so
-- the bookings(userId:) smoke query returns data without a manual setup step.
INSERT INTO bookings (id, property_id, user_id, check_in, check_out, status, total_amount, currency, idempotency_key, created_at) VALUES
    ('b0000000-0000-0000-0000-000000000001', '11111111-1111-1111-1111-000000000001', '80000000-0000-0000-0000-000000000001', DATE '2026-06-12', DATE '2026-06-15', 'CONFIRMED',  720.00, 'USD', 'seed-bk-1', '2026-04-01T10:00:00Z'),
    ('b0000000-0000-0000-0000-000000000002', '22222222-2222-2222-2222-000000000001', '80000000-0000-0000-0000-000000000003', DATE '2026-08-20', DATE '2026-08-23', 'CONFIRMED', 1140.00, 'USD', 'seed-bk-2', '2026-04-02T10:00:00Z'),
    ('b0000000-0000-0000-0000-000000000003', '33333333-3333-3333-3333-000000000001', '80000000-0000-0000-0000-000000000004', DATE '2026-09-05', DATE '2026-09-10', 'CONFIRMED',  825.00, 'EUR', 'seed-bk-3', '2026-04-03T10:00:00Z'),
    ('b0000000-0000-0000-0000-000000000004', '44444444-4444-4444-4444-000000000002', '80000000-0000-0000-0000-000000000008', DATE '2026-10-01', DATE '2026-10-04', 'PENDING',    84000.00, 'JPY', 'seed-bk-4', '2026-04-04T10:00:00Z'),
    ('b0000000-0000-0000-0000-000000000005', '55555555-5555-5555-5555-000000000001', '80000000-0000-0000-0000-000000000010', DATE '2026-11-15', DATE '2026-11-19', 'CANCELLED',16800.00, 'ZAR', 'seed-bk-5', '2026-04-05T10:00:00Z');
