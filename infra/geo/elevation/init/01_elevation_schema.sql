CREATE EXTENSION IF NOT EXISTS postgis;
CREATE EXTENSION IF NOT EXISTS postgis_raster;

-- One row a block of the one grid, by its place on it: a block two shards'
-- aprons share is the same samples, so it is stored once.
CREATE TABLE IF NOT EXISTS public.elevation_raster (
    rid SERIAL PRIMARY KEY,
    rast RASTER NOT NULL,
    bx INTEGER NOT NULL,
    by INTEGER NOT NULL,
    UNIQUE (bx, by)
);

CREATE INDEX IF NOT EXISTS idx_elevation_raster_rast_convexhull
ON public.elevation_raster USING GIST (ST_ConvexHull(rast));

-- A shard's warp, cut into blocks, before they join the grid.
CREATE TABLE IF NOT EXISTS public.elevation_stage (
    rid SERIAL PRIMARY KEY,
    rast RASTER,
    filename TEXT
);

-- One row a shard, written once its raster is whole.
CREATE TABLE IF NOT EXISTS elevation_catalog (
    shard TEXT PRIMARY KEY,
    data_date DATE NOT NULL,
    file_name TEXT NOT NULL,
    status VARCHAR(20) NOT NULL,
    imported_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    total_count BIGINT NOT NULL DEFAULT 0
);
