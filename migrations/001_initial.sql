-- NHC Advisory Parser — initial schema
-- Requires: PostgreSQL 14+ with PostGIS 3.x extension

CREATE EXTENSION IF NOT EXISTS postgis;

-- ─────────────────────────────────────────────────────────────────────────────
-- fstadv — Forecast/Advisory
-- ─────────────────────────────────────────────────────────────────────────────

CREATE TABLE fstadv_advisory (
    id                    BIGSERIAL PRIMARY KEY,

    -- Storm identity
    storm_id              TEXT        NOT NULL,   -- e.g. "EP042025"
    basin                 TEXT        NOT NULL,   -- "EP", "AL", "CP"
    storm_number          SMALLINT    NOT NULL,
    year                  SMALLINT    NOT NULL,

    -- Advisory metadata
    advisory_number       TEXT        NOT NULL,
    issued_utc            TIMESTAMPTZ NOT NULL,
    storm_name            TEXT        NOT NULL,
    status                TEXT        NOT NULL,

    -- Current position (PostGIS geography for distance calculations)
    lat                   DOUBLE PRECISION NOT NULL,
    lon                   DOUBLE PRECISION NOT NULL,
    position_geog         GEOGRAPHY(POINT, 4326) GENERATED ALWAYS AS (
                              ST_SetSRID(ST_MakePoint(lon, lat), 4326)::geography
                          ) STORED,
    position_accuracy_nm  INTEGER     NOT NULL DEFAULT 0,

    -- Movement
    movement_degrees      INTEGER     NOT NULL,
    movement_cardinal     TEXT        NOT NULL,
    movement_kt           SMALLINT    NOT NULL,

    -- Intensity
    pressure_mb           INTEGER     NOT NULL,
    max_wind_kt           INTEGER     NOT NULL,
    gust_kt               INTEGER     NOT NULL,

    -- Previous position (for track reconstruction)
    prev_time             TEXT,
    prev_lat              DOUBLE PRECISION,
    prev_lon              DOUBLE PRECISION,
    prev_geog             GEOGRAPHY(POINT, 4326) GENERATED ALWAYS AS (
                              CASE WHEN prev_lat IS NOT NULL AND prev_lon IS NOT NULL
                              THEN ST_SetSRID(ST_MakePoint(prev_lon, prev_lat), 4326)::geography
                              END
                          ) STORED,

    next_advisory_utc     TEXT,
    forecaster            TEXT        NOT NULL,

    -- Ingestion metadata
    ingested_at           TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT fstadv_unique UNIQUE (storm_id, advisory_number, issued_utc)
);

CREATE INDEX idx_fstadv_storm_id   ON fstadv_advisory (storm_id);
CREATE INDEX idx_fstadv_issued_utc ON fstadv_advisory (issued_utc DESC);
CREATE INDEX idx_fstadv_position   ON fstadv_advisory USING GIST (position_geog);

-- ─────────────────────────────────────────────────────────────────────────────
-- wind_radii — Current wind radii attached to an fstadv advisory
-- ─────────────────────────────────────────────────────────────────────────────

CREATE TABLE wind_radii (
    id                BIGSERIAL PRIMARY KEY,
    storm_id          TEXT        NOT NULL,
    advisory_number   TEXT        NOT NULL,
    issued_utc        TIMESTAMPTZ NOT NULL,
    threshold_kt      SMALLINT    NOT NULL,  -- 34, 50, or 64
    ne_nm             INTEGER     NOT NULL,
    se_nm             INTEGER     NOT NULL,
    sw_nm             INTEGER     NOT NULL,
    nw_nm             INTEGER     NOT NULL,

    CONSTRAINT wind_radii_unique UNIQUE (storm_id, advisory_number, issued_utc, threshold_kt)
);

CREATE INDEX idx_wind_radii_advisory ON wind_radii (storm_id, advisory_number);

-- ─────────────────────────────────────────────────────────────────────────────
-- forecast_point — Forecast/outlook track points from fstadv
-- ─────────────────────────────────────────────────────────────────────────────

CREATE TABLE forecast_point (
    id                BIGSERIAL PRIMARY KEY,
    storm_id          TEXT        NOT NULL,
    advisory_number   TEXT        NOT NULL,
    issued_utc        TIMESTAMPTZ NOT NULL,

    valid_time        TEXT        NOT NULL,  -- "DD/HHmmZ"
    is_outlook        BOOLEAN     NOT NULL DEFAULT FALSE,
    status            TEXT        NOT NULL,

    lat               DOUBLE PRECISION,
    lon               DOUBLE PRECISION,
    point_geog        GEOGRAPHY(POINT, 4326) GENERATED ALWAYS AS (
                          CASE WHEN lat IS NOT NULL AND lon IS NOT NULL
                          THEN ST_SetSRID(ST_MakePoint(lon, lat), 4326)::geography
                          END
                      ) STORED,

    max_wind_kt       INTEGER,
    gust_kt           INTEGER,

    CONSTRAINT forecast_point_unique UNIQUE (storm_id, advisory_number, issued_utc, valid_time)
);

CREATE INDEX idx_forecast_point_advisory ON forecast_point (storm_id, advisory_number);
CREATE INDEX idx_forecast_point_geog     ON forecast_point USING GIST (point_geog);

-- ─────────────────────────────────────────────────────────────────────────────
-- public_advisory — Public Advisory (TCP)
-- ─────────────────────────────────────────────────────────────────────────────

CREATE TABLE public_advisory (
    id                BIGSERIAL PRIMARY KEY,
    storm_id          TEXT        NOT NULL,
    advisory_number   TEXT        NOT NULL,
    issued_utc        TIMESTAMPTZ NOT NULL,

    storm_name        TEXT        NOT NULL,
    status            TEXT        NOT NULL,

    lat               DOUBLE PRECISION NOT NULL,
    lon               DOUBLE PRECISION NOT NULL,
    position_geog     GEOGRAPHY(POINT, 4326) GENERATED ALWAYS AS (
                          ST_SetSRID(ST_MakePoint(lon, lat), 4326)::geography
                      ) STORED,

    movement_degrees  INTEGER     NOT NULL,
    movement_cardinal TEXT        NOT NULL,
    movement_mph      INTEGER     NOT NULL,
    movement_kmh      INTEGER     NOT NULL,

    pressure_mb       INTEGER     NOT NULL,
    max_wind_mph      INTEGER     NOT NULL,
    max_wind_kmh      INTEGER     NOT NULL,
    gust_mph          INTEGER     NOT NULL,
    gust_kmh          INTEGER     NOT NULL,

    headline          TEXT,
    next_advisory_utc TEXT,
    forecaster        TEXT        NOT NULL,

    ingested_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT public_advisory_unique UNIQUE (storm_id, advisory_number, issued_utc)
);

CREATE INDEX idx_public_advisory_storm_id ON public_advisory (storm_id);
CREATE INDEX idx_public_advisory_issued   ON public_advisory (issued_utc DESC);

-- ─────────────────────────────────────────────────────────────────────────────
-- discus_advisory — Forecast Discussion (TCD)
-- ─────────────────────────────────────────────────────────────────────────────

CREATE TABLE discus_advisory (
    id                BIGSERIAL PRIMARY KEY,
    storm_id          TEXT        NOT NULL,
    advisory_number   TEXT        NOT NULL,
    issued_utc        TIMESTAMPTZ NOT NULL,

    storm_name        TEXT        NOT NULL,
    status            TEXT        NOT NULL,
    body              TEXT        NOT NULL,
    forecaster        TEXT        NOT NULL,

    ingested_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT discus_advisory_unique UNIQUE (storm_id, advisory_number, issued_utc)
);

CREATE INDEX idx_discus_advisory_storm_id ON discus_advisory (storm_id);

-- ─────────────────────────────────────────────────────────────────────────────
-- wind_prob_entry — Wind Speed Probabilities (PWS / wndprb)
-- ─────────────────────────────────────────────────────────────────────────────

CREATE TABLE wind_prob_entry (
    id                BIGSERIAL PRIMARY KEY,
    storm_id          TEXT        NOT NULL,
    advisory_number   TEXT        NOT NULL,
    issued_utc        TIMESTAMPTZ NOT NULL,

    threshold_kt      SMALLINT    NOT NULL,  -- 34, 50, or 64
    location          TEXT        NOT NULL,

    lat               DOUBLE PRECISION NOT NULL,
    lon               DOUBLE PRECISION NOT NULL,
    location_geog     GEOGRAPHY(POINT, 4326) GENERATED ALWAYS AS (
                          ST_SetSRID(ST_MakePoint(lon, lat), 4326)::geography
                      ) STORED,

    -- Cumulative probabilities by window; 0 encodes "X" (< 1 %)
    p12               SMALLINT    NOT NULL,
    p24               SMALLINT    NOT NULL,
    p36               SMALLINT    NOT NULL,
    p48               SMALLINT    NOT NULL,
    p72               SMALLINT    NOT NULL,
    p96               SMALLINT    NOT NULL,
    p120              SMALLINT    NOT NULL,

    ingested_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT wind_prob_entry_unique UNIQUE (storm_id, advisory_number, issued_utc, threshold_kt, location)
);

CREATE INDEX idx_wind_prob_entry_advisory  ON wind_prob_entry (storm_id, advisory_number);
CREATE INDEX idx_wind_prob_entry_geog      ON wind_prob_entry USING GIST (location_geog);
CREATE INDEX idx_wind_prob_entry_threshold ON wind_prob_entry (threshold_kt);

-- ─────────────────────────────────────────────────────────────────────────────
-- Useful views
-- ─────────────────────────────────────────────────────────────────────────────

-- Latest advisory per storm + product type
CREATE VIEW latest_fstadv AS
SELECT DISTINCT ON (storm_id)
    *
FROM fstadv_advisory
ORDER BY storm_id, issued_utc DESC;

-- Full track for any storm (issued positions only, not forecasts)
CREATE VIEW storm_track AS
SELECT
    storm_id,
    storm_name,
    advisory_number,
    issued_utc,
    status,
    lat,
    lon,
    position_geog,
    max_wind_kt,
    pressure_mb
FROM fstadv_advisory
ORDER BY storm_id, issued_utc;
