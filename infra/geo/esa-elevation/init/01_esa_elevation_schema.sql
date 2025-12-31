-- [1] 확장팩 설치 (Extensions)
CREATE EXTENSION IF NOT EXISTS postgis;
CREATE EXTENSION IF NOT EXISTS postgis_raster;

-- [2] 메인 데이터 테이블 (Main Data Table: Raster SSOT)
-- Note: raster2pgsql will recreate this table, but explicit schema helps documentation
CREATE TABLE IF NOT EXISTS public.esa_elevation_raster (
    rid SERIAL PRIMARY KEY,
    rast RASTER
);

CREATE INDEX IF NOT EXISTS idx_esa_elevation_raster_rast_convexhull ON public.esa_elevation_raster USING GIST (ST_ConvexHull(rast));

-- [3] 카탈로그 테이블 (Catalog: Metadata & History)
CREATE TABLE IF NOT EXISTS esa_elevation_catalog (
    id SERIAL PRIMARY KEY,
    region_code VARCHAR(20) NOT NULL,
    imported_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    data_date DATE NOT NULL,
    file_name TEXT NOT NULL,
    status VARCHAR(20) DEFAULT 'READY',
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    total_count BIGINT DEFAULT 0,
    -- ON CONFLICT 처리를 위한 유니크 제약조건 추가
    CONSTRAINT uq_esa_catalog_file_name UNIQUE (file_name)
);

-- 카탈로그 인덱스
CREATE INDEX IF NOT EXISTS idx_esa_catalog_region_status ON esa_elevation_catalog (region_code, status);