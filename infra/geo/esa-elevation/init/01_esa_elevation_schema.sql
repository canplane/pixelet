-- [1] Extensions
CREATE EXTENSION IF NOT EXISTS postgis;
CREATE EXTENSION IF NOT EXISTS postgis_raster;

-- [2] Main data table (primary raster store)
-- Note: raster2pgsql may recreate this table, but the explicit schema helps documentation.
CREATE TABLE IF NOT EXISTS public.esa_elevation_raster (
    rid SERIAL PRIMARY KEY,
    rast RASTER
);

CREATE INDEX IF NOT EXISTS idx_esa_elevation_raster_rast_convexhull
ON public.esa_elevation_raster USING GIST (ST_ConvexHull(rast));

-- [3] Catalog table (metadata and ingestion history)
CREATE TABLE IF NOT EXISTS esa_elevation_catalog (
    id SERIAL PRIMARY KEY,
    region_code VARCHAR(20) NOT NULL,
    imported_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    data_date DATE NOT NULL,
    file_name TEXT NOT NULL,
    status VARCHAR(20) DEFAULT 'READY',
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    total_count BIGINT DEFAULT 0,
    -- Unique key for ON CONFLICT handling
    CONSTRAINT uq_esa_catalog_file_name UNIQUE (file_name)
);

-- Catalog index
CREATE INDEX IF NOT EXISTS idx_esa_catalog_region_status ON esa_elevation_catalog (region_code, status);