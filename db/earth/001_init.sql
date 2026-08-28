-- Schema for the Fossall deep-time Earth education database.
-- Apply against database `earth` on cluster `postgreputest`.
-- See docs/DATABASE.md and docs/EARTH.md.
--
-- Intentionally portable: geometries are JSONB (GeoJSON), not PostGIS.
-- Try pg_trgm for taxon search; skip if the extension is not allowed.

CREATE SCHEMA IF NOT EXISTS earth;

DO $$
BEGIN
    CREATE EXTENSION IF NOT EXISTS pg_trgm;
EXCEPTION WHEN OTHERS THEN
    RAISE NOTICE 'pg_trgm not available; taxon search will use ILIKE';
END
$$;

-- ── Provenance ───────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS earth.source (
    id SMALLSERIAL PRIMARY KEY,
    code TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    url TEXT,
    license TEXT NOT NULL,
    citation TEXT NOT NULL,
    retrieved_at TIMESTAMPTZ
);

CREATE TABLE IF NOT EXISTS earth.source_record (
    id BIGSERIAL PRIMARY KEY,
    source_id SMALLINT NOT NULL REFERENCES earth.source (id),
    external_id TEXT NOT NULL,
    UNIQUE (source_id, external_id)
);

-- ── Geologic time (ICS ranks) ────────────────────────────────────────

CREATE TABLE IF NOT EXISTS earth.time_rank (
    id SMALLSERIAL PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    level SMALLINT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS earth.time_unit (
    id SERIAL PRIMARY KEY,
    parent_id INTEGER REFERENCES earth.time_unit (id),
    rank_id SMALLINT NOT NULL REFERENCES earth.time_rank (id),
    name TEXT NOT NULL,
    abbrev TEXT,
    color_hex TEXT,
    start_ma DOUBLE PRECISION NOT NULL,
    end_ma DOUBLE PRECISION NOT NULL,
    ics_name TEXT,
    macrostrat_int_id INTEGER UNIQUE,
    CHECK (start_ma >= end_ma),
    UNIQUE (rank_id, name)
);

CREATE INDEX IF NOT EXISTS time_unit_range_idx
    ON earth.time_unit (start_ma, end_ma);
CREATE INDEX IF NOT EXISTS time_unit_parent_idx
    ON earth.time_unit (parent_id);

-- ── Taxonomy ──────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS earth.taxon_rank (
    id SMALLSERIAL PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    level SMALLINT NOT NULL
);

CREATE TABLE IF NOT EXISTS earth.nomenclatural_status (
    id SMALLSERIAL PRIMARY KEY,
    code TEXT NOT NULL UNIQUE,
    description TEXT
);

CREATE TABLE IF NOT EXISTS earth.taxon (
    id BIGSERIAL PRIMARY KEY,
    rank_id SMALLINT REFERENCES earth.taxon_rank (id),
    parent_id BIGINT REFERENCES earth.taxon (id),
    scientific_name TEXT NOT NULL,
    authorship TEXT,
    named_year INTEGER,
    status_id SMALLINT REFERENCES earth.nomenclatural_status (id),
    extant BOOLEAN,
    pbdb_taxon_no BIGINT UNIQUE,
    pbdb_parent_no BIGINT,
    first_app_ma DOUBLE PRECISION,
    last_app_ma DOUBLE PRECISION
);

CREATE INDEX IF NOT EXISTS taxon_parent_idx ON earth.taxon (parent_id);
CREATE INDEX IF NOT EXISTS taxon_scientific_name_lower_idx
    ON earth.taxon (lower(scientific_name));

CREATE TABLE IF NOT EXISTS earth.taxon_name (
    id BIGSERIAL PRIMARY KEY,
    taxon_id BIGINT NOT NULL REFERENCES earth.taxon (id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    name_type TEXT NOT NULL
        CHECK (name_type IN ('synonym', 'vernacular', 'original', 'accepted')),
    language_code TEXT
);

CREATE INDEX IF NOT EXISTS taxon_alias_name_lower_idx
    ON earth.taxon_name (lower(name));

CREATE TABLE IF NOT EXISTS earth.taxon_opinion (
    id BIGSERIAL PRIMARY KEY,
    child_id BIGINT NOT NULL REFERENCES earth.taxon (id),
    parent_id BIGINT REFERENCES earth.taxon (id),
    status_id SMALLINT REFERENCES earth.nomenclatural_status (id),
    source_id SMALLINT NOT NULL REFERENCES earth.source (id),
    published_year INTEGER,
    pbdb_opinion_no BIGINT UNIQUE,
    UNIQUE (child_id, parent_id, source_id)
);

-- ── Rocks, sediments, environments ───────────────────────────────────

CREATE TABLE IF NOT EXISTS earth.rock_class (
    id SMALLSERIAL PRIMARY KEY,
    name TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS earth.lithology (
    id SERIAL PRIMARY KEY,
    class_id SMALLINT REFERENCES earth.rock_class (id),
    parent_id INTEGER REFERENCES earth.lithology (id),
    name TEXT NOT NULL,
    lith_type TEXT,
    lith_group TEXT,
    color_hex TEXT,
    macrostrat_lith_id INTEGER UNIQUE
);

CREATE INDEX IF NOT EXISTS lithology_name_lower_idx
    ON earth.lithology (lower(name));

CREATE TABLE IF NOT EXISTS earth.lithology_attribute (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    attr_type TEXT,
    macrostrat_id INTEGER UNIQUE
);

CREATE TABLE IF NOT EXISTS earth.environment (
    id SERIAL PRIMARY KEY,
    parent_id INTEGER REFERENCES earth.environment (id),
    name TEXT NOT NULL,
    env_type TEXT,
    env_class TEXT,
    color_hex TEXT,
    macrostrat_env_id INTEGER UNIQUE
);

CREATE INDEX IF NOT EXISTS environment_name_lower_idx
    ON earth.environment (lower(name));

CREATE TABLE IF NOT EXISTS earth.lithostrat_rank (
    id SMALLSERIAL PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    code TEXT NOT NULL UNIQUE,
    level SMALLINT NOT NULL
);

CREATE TABLE IF NOT EXISTS earth.strat_unit (
    id BIGSERIAL PRIMARY KEY,
    parent_id BIGINT REFERENCES earth.strat_unit (id),
    rank_id SMALLINT REFERENCES earth.lithostrat_rank (id),
    name TEXT NOT NULL,
    t_age_ma DOUBLE PRECISION,
    b_age_ma DOUBLE PRECISION,
    macrostrat_strat_id INTEGER UNIQUE
);

CREATE INDEX IF NOT EXISTS strat_unit_name_lower_idx
    ON earth.strat_unit (lower(name));
CREATE INDEX IF NOT EXISTS strat_unit_parent_idx
    ON earth.strat_unit (parent_id);

CREATE TABLE IF NOT EXISTS earth.strat_unit_lithology (
    strat_unit_id BIGINT NOT NULL REFERENCES earth.strat_unit (id) ON DELETE CASCADE,
    lithology_id INTEGER NOT NULL REFERENCES earth.lithology (id),
    PRIMARY KEY (strat_unit_id, lithology_id)
);

-- ── Fossil collections and occurrences ────────────────────────────────

CREATE TABLE IF NOT EXISTS earth.collection (
    id BIGSERIAL PRIMARY KEY,
    pbdb_collection_no BIGINT UNIQUE,
    name TEXT,
    lat DOUBLE PRECISION,
    lng DOUBLE PRECISION,
    paleolat DOUBLE PRECISION,
    paleolng DOUBLE PRECISION,
    country_code TEXT,
    environment_id INTEGER REFERENCES earth.environment (id),
    lithology_id INTEGER REFERENCES earth.lithology (id),
    time_unit_id INTEGER REFERENCES earth.time_unit (id),
    strat_unit_id BIGINT REFERENCES earth.strat_unit (id),
    max_ma DOUBLE PRECISION,
    min_ma DOUBLE PRECISION
);

CREATE INDEX IF NOT EXISTS collection_time_idx
    ON earth.collection (max_ma, min_ma);
CREATE INDEX IF NOT EXISTS collection_paleo_idx
    ON earth.collection (paleolat, paleolng)
    WHERE paleolat IS NOT NULL AND paleolng IS NOT NULL;

CREATE TABLE IF NOT EXISTS earth.occurrence (
    id BIGSERIAL PRIMARY KEY,
    collection_id BIGINT NOT NULL REFERENCES earth.collection (id),
    taxon_id BIGINT NOT NULL REFERENCES earth.taxon (id),
    pbdb_occurrence_no BIGINT UNIQUE
);

CREATE INDEX IF NOT EXISTS occurrence_taxon_idx ON earth.occurrence (taxon_id);
CREATE INDEX IF NOT EXISTS occurrence_collection_idx
    ON earth.occurrence (collection_id);

-- ── Paleogeography ────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS earth.plate_model (
    id SMALLSERIAL PRIMARY KEY,
    code TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    citation TEXT NOT NULL,
    max_ma DOUBLE PRECISION NOT NULL,
    min_ma DOUBLE PRECISION NOT NULL,
    source_id SMALLINT NOT NULL REFERENCES earth.source (id)
);

CREATE TABLE IF NOT EXISTS earth.land_feature_type (
    id SMALLSERIAL PRIMARY KEY,
    code TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS earth.reconstruction (
    id SERIAL PRIMARY KEY,
    plate_model_id SMALLINT NOT NULL REFERENCES earth.plate_model (id),
    time_ma DOUBLE PRECISION NOT NULL,
    time_unit_id INTEGER REFERENCES earth.time_unit (id),
    UNIQUE (plate_model_id, time_ma)
);

CREATE INDEX IF NOT EXISTS reconstruction_time_idx
    ON earth.reconstruction (time_ma);

CREATE TABLE IF NOT EXISTS earth.reconstruction_geometry (
    id BIGSERIAL PRIMARY KEY,
    reconstruction_id INTEGER NOT NULL
        REFERENCES earth.reconstruction (id) ON DELETE CASCADE,
    feature_type_id SMALLINT NOT NULL REFERENCES earth.land_feature_type (id),
    geom JSONB NOT NULL,
    bbox_west DOUBLE PRECISION,
    bbox_south DOUBLE PRECISION,
    bbox_east DOUBLE PRECISION,
    bbox_north DOUBLE PRECISION
);

CREATE INDEX IF NOT EXISTS reconstruction_geometry_recon_idx
    ON earth.reconstruction_geometry (reconstruction_id);

-- ── Lookup rows ───────────────────────────────────────────────────────

INSERT INTO earth.source (code, name, url, license, citation) VALUES
    (
        'ics',
        'International Commission on Stratigraphy',
        'https://stratigraphy.org/chart',
        'CC-BY-4.0',
        'Cohen, K.M., Harper, D.A.T., Gibbard, P.L., 2023/2026, ICS International Chronostratigraphic Chart. https://stratigraphy.org/chart'
    ),
    (
        'macrostrat',
        'Macrostrat',
        'https://macrostrat.org/',
        'CC-BY-4.0',
        'Peters, S.E., Husson, J.M., and Czaplewski, J., 2018, Macrostrat: A platform for geological data integration and deep-time Earth crust research. Geochemistry, Geophysics, Geosystems. https://macrostrat.org/'
    ),
    (
        'pbdb',
        'Paleobiology Database',
        'https://paleobiodb.org/',
        'CC-BY-4.0',
        'The Paleobiology Database, public data service 1.2. https://paleobiodb.org/data1.2/'
    ),
    (
        'gplates',
        'GPlates / EarthByte',
        'https://gwsdoc.gplates.org/',
        'CC-BY-3.0',
        'Cao, X., Collins, A.S., Pisarevsky, S., Flament, N., Li, S., Hasterok, D. and Müller, R.D., 2024. Earth’s tectonic and plate boundary evolution over 1.8 billion years. Geoscience Frontiers 15, 101922. Reconstruction served by the GPlates Web Service.'
    ),
    (
        'naturalearth',
        'Natural Earth',
        'https://www.naturalearthdata.com/',
        'public domain',
        'Natural Earth, public-domain vector map data. https://www.naturalearthdata.com/'
    )
ON CONFLICT (code) DO NOTHING;

INSERT INTO earth.time_rank (name, level) VALUES
    ('eon', 1),
    ('era', 2),
    ('period', 3),
    ('subperiod', 4),
    ('epoch', 5),
    ('age', 6)
ON CONFLICT (name) DO NOTHING;

INSERT INTO earth.taxon_rank (name, level) VALUES
    ('unranked clade', 0),
    ('informal', 1),
    ('kingdom', 10),
    ('subkingdom', 11),
    ('superphylum', 12),
    ('phylum', 13),
    ('subphylum', 14),
    ('superclass', 15),
    ('class', 16),
    ('subclass', 17),
    ('infraclass', 18),
    ('superorder', 19),
    ('order', 20),
    ('suborder', 21),
    ('infraorder', 22),
    ('superfamily', 23),
    ('family', 24),
    ('subfamily', 25),
    ('tribe', 26),
    ('subtribe', 27),
    ('genus', 28),
    ('subgenus', 29),
    ('species', 30),
    ('subspecies', 31)
ON CONFLICT (name) DO NOTHING;

INSERT INTO earth.nomenclatural_status (code, description) VALUES
    ('accepted', 'Accepted / valid name'),
    ('synonym', 'Subjective or objective synonym'),
    ('invalid', 'Invalid or not established'),
    ('nomen_dubium', 'Nomen dubium'),
    ('nomen_nudum', 'Nomen nudum'),
    ('homonym', 'Preoccupied / homonym')
ON CONFLICT (code) DO NOTHING;

INSERT INTO earth.rock_class (name) VALUES
    ('sedimentary'),
    ('igneous'),
    ('metamorphic'),
    ('other')
ON CONFLICT (name) DO NOTHING;

INSERT INTO earth.lithostrat_rank (name, code, level) VALUES
    ('supergroup', 'Sgp', 1),
    ('group', 'Gp', 2),
    ('subgroup', 'Subgp', 3),
    ('formation', 'Fm', 4),
    ('member', 'Mbr', 5),
    ('bed', 'Bed', 6)
ON CONFLICT (name) DO NOTHING;

INSERT INTO earth.land_feature_type (code, name) VALUES
    ('coastline', 'Reconstructed coastline / land'),
    ('continental_polygon', 'Continental polygon'),
    ('shallow_marine', 'Shallow marine'),
    ('mountain', 'Mountainous terrain'),
    ('ice', 'Ice sheet')
ON CONFLICT (code) DO NOTHING;

INSERT INTO earth.plate_model (code, name, citation, max_ma, min_ma, source_id)
SELECT
    'CAO2024',
    'Cao et al. 2024 full-plate model (0–1800 Ma)',
    'Cao, X. et al., 2024. Earth’s tectonic and plate boundary evolution over 1.8 billion years. Geoscience Frontiers 15, 101922.',
    1800,
    0,
    s.id
FROM earth.source s
WHERE s.code = 'gplates'
ON CONFLICT (code) DO NOTHING;

-- Optional trigram index (no-op if the extension was skipped).
DO $$
BEGIN
    EXECUTE 'CREATE INDEX IF NOT EXISTS taxon_name_trgm_idx ON earth.taxon USING gin (scientific_name gin_trgm_ops)';
EXCEPTION WHEN OTHERS THEN
    RAISE NOTICE 'skipping taxon trigram index';
END
$$;

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'earth') THEN
        GRANT USAGE ON SCHEMA earth TO earth;
        GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA earth TO earth;
        GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA earth TO earth;
        ALTER DEFAULT PRIVILEGES IN SCHEMA earth
            GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO earth;
        ALTER DEFAULT PRIVILEGES IN SCHEMA earth
            GRANT USAGE, SELECT ON SEQUENCES TO earth;
    END IF;
END
$$;
