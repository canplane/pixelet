CREATE EXTENSION IF NOT EXISTS postgis;
CREATE EXTENSION IF NOT EXISTS postgis_raster;

-- One row a block of the one grid, a tile of the pyramid at `BLOCK_ZOOM`
-- (pxl-geo-bake elevation.rs), under that tile's name: a block two shards'
-- aprons share is the same samples, so it is stored once.
CREATE TABLE IF NOT EXISTS public.elevation_raster (
    tile TEXT PRIMARY KEY,
    rast RASTER NOT NULL
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
    version TEXT NOT NULL,
    imported_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    total_count BIGINT NOT NULL DEFAULT 0
);
