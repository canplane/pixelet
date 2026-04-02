-- [1] Extensions
-- Keeps PostGIS dependency explicit, matching the osm-features pipeline.
CREATE EXTENSION IF NOT EXISTS postgis;

-- [2] Main data table (primary coastline store)
-- Source: land-polygons-split-3857 (Shapefile)
-- fid  : source feature identifier or generated surrogate key
-- geom : coastline multipolygon geometry in EPSG:3857
CREATE TABLE IF NOT EXISTS osm_coastline_polygons (
    fid SERIAL PRIMARY KEY,
    geom GEOMETRY(MultiPolygon, 3857),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_osm_coastline_geom ON osm_coastline_polygons USING GIST (geom);

-- [3] Catalog table (metadata and ingestion history)
-- Mirrors the osm_features_catalog structure.
CREATE TABLE IF NOT EXISTS osm_coastline_catalog (
    id SERIAL PRIMARY KEY,
    region_code VARCHAR(20) NOT NULL,
    imported_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    data_date DATE NOT NULL,
    file_name TEXT NOT NULL,
    status VARCHAR(20) DEFAULT 'READY',
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    total_count BIGINT DEFAULT 0,
    -- Unique key for ON CONFLICT handling
    CONSTRAINT uq_coastline_catalog_file_name UNIQUE (file_name)
);

-- Catalog index
CREATE INDEX IF NOT EXISTS idx_coastline_catalog_region_status ON osm_coastline_catalog (region_code, status);