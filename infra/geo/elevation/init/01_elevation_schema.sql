CREATE EXTENSION IF NOT EXISTS postgis;
CREATE EXTENSION IF NOT EXISTS postgis_raster;

-- A shard's raster tiles carry the file they came from, its name.
CREATE TABLE IF NOT EXISTS public.elevation_raster (
    rid SERIAL PRIMARY KEY,
    rast RASTER,
    filename TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_elevation_raster_rast_convexhull
ON public.elevation_raster USING GIST (ST_ConvexHull(rast));
CREATE INDEX IF NOT EXISTS idx_elevation_raster_filename ON public.elevation_raster (filename);

-- One row a shard, written once its raster is whole.
CREATE TABLE IF NOT EXISTS elevation_catalog (
    shard TEXT PRIMARY KEY,
    data_date DATE NOT NULL,
    file_name TEXT NOT NULL,
    status VARCHAR(20) NOT NULL,
    imported_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    total_count BIGINT NOT NULL DEFAULT 0
);
