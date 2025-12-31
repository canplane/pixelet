-- [1] 확장팩 확인 (Extensions)
-- osm-features와 동일하게 PostGIS 의존성 확인
CREATE EXTENSION IF NOT EXISTS postgis;

-- [2] 메인 데이터 테이블 (Main Data Table: Coastline SSOT)
-- Source: land-polygons-split-3857 (Shapefile)
-- fid: Shapefile의 고유 ID를 매핑하거나 자동 생성
-- geom: 3857 좌표계의 MultiPolygon
CREATE TABLE IF NOT EXISTS osm_coastline_polygons (
    fid SERIAL PRIMARY KEY,
    geom GEOMETRY(MultiPolygon, 3857),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_osm_coastline_geom ON osm_coastline_polygons USING GIST (geom);

-- [3] 카탈로그 테이블 (Catalog: Metadata & History)
-- osm_features_catalog와 대칭되는 구조
CREATE TABLE IF NOT EXISTS osm_coastline_catalog (
    id SERIAL PRIMARY KEY,
    region_code VARCHAR(20) NOT NULL,
    imported_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    data_date DATE NOT NULL,
    file_name TEXT NOT NULL,
    status VARCHAR(20) DEFAULT 'READY',
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    total_count BIGINT DEFAULT 0,
    -- ON CONFLICT 처리를 위한 유니크 제약조건
    CONSTRAINT uq_coastline_catalog_file_name UNIQUE (file_name)
);

-- 카탈로그 인덱스
CREATE INDEX IF NOT EXISTS idx_coastline_catalog_region_status ON osm_coastline_catalog (region_code, status);