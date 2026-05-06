-- V1: Users + saved-property junction table.
-- savedPropertyIds is a List<UUID> field on the entity, modelled as a join
-- table for queryability and clean cascade semantics.

CREATE TABLE users (
    id                  UUID         PRIMARY KEY,
    name                VARCHAR(128) NOT NULL,
    email               VARCHAR(255) NOT NULL UNIQUE,
    loyalty_status      VARCHAR(16)  NOT NULL CHECK (loyalty_status IN ('BRONZE', 'SILVER', 'GOLD', 'PLATINUM')),
    preferred_currency  VARCHAR(3)   NOT NULL,
    created_at          TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_users_email           ON users (LOWER(email));
CREATE INDEX idx_users_loyalty_status  ON users (loyalty_status);

CREATE TABLE user_saved_properties (
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    property_id UUID NOT NULL,
    PRIMARY KEY (user_id, property_id)
);

CREATE INDEX idx_user_saved_properties_user_id ON user_saved_properties (user_id);

-- Seed: 10 users with mixed loyalty tiers and preferred currencies.
-- IDs are stable so booking + review services can reference them directly.
INSERT INTO users (id, name, email, loyalty_status, preferred_currency) VALUES
    ('80000000-0000-0000-0000-000000000001', 'Alice Adams',     'alice.adams@example.com',     'BRONZE',   'USD'),
    ('80000000-0000-0000-0000-000000000002', 'Bao Bui',          'bao.bui@example.com',         'SILVER',   'EUR'),
    ('80000000-0000-0000-0000-000000000003', 'Carlos Castro',   'carlos.castro@example.com',   'GOLD',     'USD'),
    ('80000000-0000-0000-0000-000000000004', 'Daniela Davis',   'daniela.davis@example.com',   'PLATINUM', 'GBP'),
    ('80000000-0000-0000-0000-000000000005', 'Eitan Eshel',     'eitan.eshel@example.com',     'BRONZE',   'USD'),
    ('80000000-0000-0000-0000-000000000006', 'Fatima Farouk',   'fatima.farouk@example.com',   'SILVER',   'USD'),
    ('80000000-0000-0000-0000-000000000007', 'Gita Gupta',      'gita.gupta@example.com',      'GOLD',     'INR'),
    ('80000000-0000-0000-0000-000000000008', 'Hiroshi Hatake',  'hiroshi.hatake@example.com',  'PLATINUM', 'JPY'),
    ('80000000-0000-0000-0000-000000000009', 'Ines Ito',        'ines.ito@example.com',        'BRONZE',   'EUR'),
    ('80000000-0000-0000-0000-000000000010', 'Jamal Johnson',   'jamal.johnson@example.com',   'SILVER',   'USD');

-- Seeded saved properties for a few users (cross-references property-service ids).
INSERT INTO user_saved_properties (user_id, property_id) VALUES
    ('80000000-0000-0000-0000-000000000001', '11111111-1111-1111-1111-000000000003'),
    ('80000000-0000-0000-0000-000000000001', '22222222-2222-2222-2222-000000000001'),
    ('80000000-0000-0000-0000-000000000004', '33333333-3333-3333-3333-000000000001'),
    ('80000000-0000-0000-0000-000000000004', '55555555-5555-5555-5555-000000000001'),
    ('80000000-0000-0000-0000-000000000007', '44444444-4444-4444-4444-000000000002'),
    ('80000000-0000-0000-0000-000000000010', '11111111-1111-1111-1111-000000000001'),
    ('80000000-0000-0000-0000-000000000010', '55555555-5555-5555-5555-000000000004');
