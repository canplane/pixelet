-- [1] 확장팩 설치
CREATE EXTENSION IF NOT EXISTS postgis;
CREATE EXTENSION IF NOT EXISTS hstore;

-- [2] 단일 통합 테이블 (모든 OSM 데이터의 SSOT)
CREATE TABLE IF NOT EXISTS osm_elements (
    osm_id BIGINT PRIMARY KEY,
    tags HSTORE,
    way GEOMETRY(Geometry, 3857),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

-- [3] 인덱스 최적화
CREATE INDEX IF NOT EXISTS idx_osm_elements_way ON osm_elements USING GIST (way);
CREATE INDEX IF NOT EXISTS idx_osm_elements_tags ON osm_elements USING GIN (tags);

-- [4] 카탈로그 (메타데이터 및 주입 상태)
CREATE TABLE IF NOT EXISTS osm_data_catalog (
    id SERIAL PRIMARY KEY,
    country_code VARCHAR(2) NOT NULL,
    imported_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    data_date DATE NOT NULL,
    file_name TEXT NOT NULL,
    total_count BIGINT DEFAULT 0,
    status VARCHAR(20) DEFAULT 'READY',
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_catalog_country_status ON osm_data_catalog (country_code, status);