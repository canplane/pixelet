-- [1] Extensions
CREATE EXTENSION IF NOT EXISTS postgis;
CREATE EXTENSION IF NOT EXISTS hstore;

-- [2] Main data table (primary vector store)
-- Preserves the highest-fidelity source geometries.
CREATE TABLE IF NOT EXISTS osm_elements (
    osm_id BIGINT,
    type VARCHAR(10), -- 'point', 'line', 'polygon'
    tags HSTORE,
    way GEOMETRY(Geometry, 3857),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT pk_osm_elements PRIMARY KEY (osm_id, type)
);

CREATE INDEX IF NOT EXISTS idx_osm_elements_way ON osm_elements USING GIST (way);
CREATE INDEX IF NOT EXISTS idx_osm_elements_tags ON osm_elements USING GIN (tags);

-- [3] Generalized road table (low-zoom optimization subset)
-- Stores simplified road geometries for zoomed-out rendering.
CREATE TABLE IF NOT EXISTS osm_roads (
    osm_id BIGINT PRIMARY KEY,
    tags HSTORE,
    way GEOMETRY(Geometry, 3857),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_osm_roads_way ON osm_roads USING GIST (way);

-- [4] Catalog table (metadata and ingestion history)
CREATE TABLE IF NOT EXISTS osm_features_catalog (
    id SERIAL PRIMARY KEY,
    region_code VARCHAR(20) NOT NULL,
    imported_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    data_date DATE NOT NULL,
    file_name TEXT NOT NULL,
    status VARCHAR(20) DEFAULT 'READY',
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    total_count BIGINT DEFAULT 0,
    -- Unique key for ON CONFLICT handling
    CONSTRAINT uq_osm_catalog_file_name UNIQUE (file_name)
);

-- Catalog index
CREATE INDEX IF NOT EXISTS idx_features_catalog_region_status ON osm_features_catalog (region_code, status);