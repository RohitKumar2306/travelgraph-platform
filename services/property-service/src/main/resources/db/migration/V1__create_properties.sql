-- V1: Property catalog schema + seed.
-- Schema is created by the Postgres init script; Flyway only creates tables here.

CREATE TABLE properties (
    id          UUID PRIMARY KEY,
    name        VARCHAR(255)     NOT NULL,
    description TEXT             NOT NULL,
    location    VARCHAR(255)     NOT NULL,
    city        VARCHAR(128)     NOT NULL,
    country     VARCHAR(128)     NOT NULL,
    rating      REAL             NOT NULL CHECK (rating >= 0 AND rating <= 5)
);

CREATE INDEX idx_properties_city ON properties (LOWER(city));

-- Amenities are normalized into a join table (JPA @ElementCollection).
-- Keeps Postgres-native, queryable, and avoids the ambiguity of array/JSON columns.
CREATE TABLE property_amenities (
    property_id UUID         NOT NULL REFERENCES properties(id) ON DELETE CASCADE,
    amenity     VARCHAR(64)  NOT NULL,
    PRIMARY KEY (property_id, amenity)
);

CREATE INDEX idx_property_amenities_property_id ON property_amenities (property_id);

-- ---------------------------------------------------------------------------
-- Seed: 20 properties across 5 cities (4 each).
-- IDs are stable so other services / tests can reference them deterministically.
-- ---------------------------------------------------------------------------

-- Austin, TX, USA
INSERT INTO properties (id, name, description, location, city, country, rating) VALUES
  ('11111111-1111-1111-1111-000000000001', 'Lady Bird Loft',          'Modern downtown loft overlooking Lady Bird Lake.',         '1100 Cesar Chavez St',     'Austin',    'USA', 4.6),
  ('11111111-1111-1111-1111-000000000002', 'South Congress Suites',   'Boutique hotel on the iconic South Congress strip.',       '1500 S Congress Ave',      'Austin',    'USA', 4.4),
  ('11111111-1111-1111-1111-000000000003', 'Hill Country Hideaway',   'Quiet bungalow with a deck and barbecue pit.',             '8200 Cuernavaca Dr',       'Austin',    'USA', 4.7),
  ('11111111-1111-1111-1111-000000000004', 'East Side Bunkhouse',     'Industrial-chic studio in East Austin near food trucks.',  '2100 E 6th St',            'Austin',    'USA', 4.2);

-- Seattle, WA, USA
INSERT INTO properties (id, name, description, location, city, country, rating) VALUES
  ('22222222-2222-2222-2222-000000000001', 'Pike Place Penthouse',    'Penthouse with views of the market and Elliott Bay.',      '85 Pike St',               'Seattle',   'USA', 4.8),
  ('22222222-2222-2222-2222-000000000002', 'Capitol Hill Cottage',    'Quiet cottage minutes from Volunteer Park.',               '1500 15th Ave E',          'Seattle',   'USA', 4.5),
  ('22222222-2222-2222-2222-000000000003', 'Ballard Boathouse',       'Floating home on the Ship Canal.',                         '5500 Seaview Ave NW',      'Seattle',   'USA', 4.6),
  ('22222222-2222-2222-2222-000000000004', 'Queen Anne View',         'Two-bedroom unit with Space Needle views.',                '500 Queen Anne Ave N',     'Seattle',   'USA', 4.3);

-- Lisbon, Portugal
INSERT INTO properties (id, name, description, location, city, country, rating) VALUES
  ('33333333-3333-3333-3333-000000000001', 'Alfama Tile House',       'Restored historic apartment in the oldest quarter.',       'R. dos Remedios 14',       'Lisbon',    'Portugal', 4.9),
  ('33333333-3333-3333-3333-000000000002', 'Bairro Alto Studio',      'Compact studio in the heart of nightlife district.',       'R. da Atalaia 50',         'Lisbon',    'Portugal', 4.2),
  ('33333333-3333-3333-3333-000000000003', 'Belem Riverside',         'River-facing flat near the Jeronimos Monastery.',          'Av. Brasilia',             'Lisbon',    'Portugal', 4.5),
  ('33333333-3333-3333-3333-000000000004', 'Principe Real Garden',    'Garden apartment in a leafy uptown neighborhood.',         'Praca do Principe Real',   'Lisbon',    'Portugal', 4.7);

-- Tokyo, Japan
INSERT INTO properties (id, name, description, location, city, country, rating) VALUES
  ('44444444-4444-4444-4444-000000000001', 'Shibuya Sky Suite',       'High-floor suite over the Shibuya scramble crossing.',     '2-24-12 Shibuya',          'Tokyo',     'Japan', 4.8),
  ('44444444-4444-4444-4444-000000000002', 'Asakusa Ryokan',          'Traditional ryokan with tatami rooms near Senso-ji.',      '1-30-3 Asakusa',           'Tokyo',     'Japan', 4.6),
  ('44444444-4444-4444-4444-000000000003', 'Ginza Capsule Plus',      'Upgraded capsule hotel with private pods.',                '6-12-1 Ginza',             'Tokyo',     'Japan', 4.1),
  ('44444444-4444-4444-4444-000000000004', 'Shimokita Loft',          'Indie-vibe loft near Shimokitazawa cafes.',                '2-15-7 Kitazawa',          'Tokyo',     'Japan', 4.4);

-- Cape Town, South Africa
INSERT INTO properties (id, name, description, location, city, country, rating) VALUES
  ('55555555-5555-5555-5555-000000000001', 'Camps Bay Villa',         'Beachfront villa with infinity pool.',                     '12 Victoria Rd',           'Cape Town', 'South Africa', 4.9),
  ('55555555-5555-5555-5555-000000000002', 'Bo-Kaap Cottage',         'Brightly painted cottage on a historic cobbled street.',   '71 Wale St',               'Cape Town', 'South Africa', 4.5),
  ('55555555-5555-5555-5555-000000000003', 'Waterfront Studio',       'Studio at the V&A Waterfront with harbor views.',          '17 Dock Rd',               'Cape Town', 'South Africa', 4.3),
  ('55555555-5555-5555-5555-000000000004', 'Constantia Wine Estate',  'Cottage on a working wine estate in the southern suburbs.','Groot Constantia Rd',      'Cape Town', 'South Africa', 4.7);

-- Amenities (denormalized seed - only the most relevant tags per property).
INSERT INTO property_amenities (property_id, amenity) VALUES
  ('11111111-1111-1111-1111-000000000001', 'WIFI'),  ('11111111-1111-1111-1111-000000000001', 'AC'),       ('11111111-1111-1111-1111-000000000001', 'KITCHEN'),
  ('11111111-1111-1111-1111-000000000002', 'WIFI'),  ('11111111-1111-1111-1111-000000000002', 'POOL'),     ('11111111-1111-1111-1111-000000000002', 'BAR'),
  ('11111111-1111-1111-1111-000000000003', 'WIFI'),  ('11111111-1111-1111-1111-000000000003', 'BBQ'),      ('11111111-1111-1111-1111-000000000003', 'PARKING'),
  ('11111111-1111-1111-1111-000000000004', 'WIFI'),  ('11111111-1111-1111-1111-000000000004', 'KITCHEN'),  ('11111111-1111-1111-1111-000000000004', 'PETS_OK'),
  ('22222222-2222-2222-2222-000000000001', 'WIFI'),  ('22222222-2222-2222-2222-000000000001', 'GYM'),      ('22222222-2222-2222-2222-000000000001', 'CITY_VIEW'),
  ('22222222-2222-2222-2222-000000000002', 'WIFI'),  ('22222222-2222-2222-2222-000000000002', 'KITCHEN'),  ('22222222-2222-2222-2222-000000000002', 'GARDEN'),
  ('22222222-2222-2222-2222-000000000003', 'WIFI'),  ('22222222-2222-2222-2222-000000000003', 'WATERFRONT'),
  ('22222222-2222-2222-2222-000000000004', 'WIFI'),  ('22222222-2222-2222-2222-000000000004', 'CITY_VIEW'),
  ('33333333-3333-3333-3333-000000000001', 'WIFI'),  ('33333333-3333-3333-3333-000000000001', 'HISTORIC'), ('33333333-3333-3333-3333-000000000001', 'KITCHEN'),
  ('33333333-3333-3333-3333-000000000002', 'WIFI'),  ('33333333-3333-3333-3333-000000000002', 'CITY_CENTER'),
  ('33333333-3333-3333-3333-000000000003', 'WIFI'),  ('33333333-3333-3333-3333-000000000003', 'WATERFRONT'),('33333333-3333-3333-3333-000000000003', 'AC'),
  ('33333333-3333-3333-3333-000000000004', 'WIFI'),  ('33333333-3333-3333-3333-000000000004', 'GARDEN'),   ('33333333-3333-3333-3333-000000000004', 'PETS_OK'),
  ('44444444-4444-4444-4444-000000000001', 'WIFI'),  ('44444444-4444-4444-4444-000000000001', 'AC'),       ('44444444-4444-4444-4444-000000000001', 'CITY_VIEW'),
  ('44444444-4444-4444-4444-000000000002', 'WIFI'),  ('44444444-4444-4444-4444-000000000002', 'TATAMI'),   ('44444444-4444-4444-4444-000000000002', 'ONSEN'),
  ('44444444-4444-4444-4444-000000000003', 'WIFI'),  ('44444444-4444-4444-4444-000000000003', 'AC'),
  ('44444444-4444-4444-4444-000000000004', 'WIFI'),  ('44444444-4444-4444-4444-000000000004', 'KITCHEN'),
  ('55555555-5555-5555-5555-000000000001', 'WIFI'),  ('55555555-5555-5555-5555-000000000001', 'POOL'),     ('55555555-5555-5555-5555-000000000001', 'BEACH'),
  ('55555555-5555-5555-5555-000000000002', 'WIFI'),  ('55555555-5555-5555-5555-000000000002', 'HISTORIC'),
  ('55555555-5555-5555-5555-000000000003', 'WIFI'),  ('55555555-5555-5555-5555-000000000003', 'WATERFRONT'),('55555555-5555-5555-5555-000000000003', 'GYM'),
  ('55555555-5555-5555-5555-000000000004', 'WIFI'),  ('55555555-5555-5555-5555-000000000004', 'GARDEN'),   ('55555555-5555-5555-5555-000000000004', 'WINE_TASTING');
