-- [1] 확장팩 설치 (Extensions)
CREATE EXTENSION IF NOT EXISTS postgis;
CREATE EXTENSION IF NOT EXISTS hstore;

-- [2] 메인 데이터 테이블 (Main Data Table: Vector SSOT)
-- 가장 정밀한 원본 형상(High-fidelity)을 보존하는 메인 저장소
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

-- [3] 도로 일반화 테이블 (Low-zoom Optimization Subset)
-- 목적: 줌 아웃 시 렌더링 성능 확보를 위한 단순화된 도로 형상 데이터 (Source: planet_osm_roads)
CREATE TABLE IF NOT EXISTS osm_roads (
    osm_id BIGINT PRIMARY KEY,
    tags HSTORE,
    way GEOMETRY(Geometry, 3857),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_osm_roads_way ON osm_roads USING GIST (way);

-- [4] 카탈로그 테이블 (Catalog: Metadata & History)
CREATE TABLE IF NOT EXISTS osm_features_catalog (
    id SERIAL PRIMARY KEY,
    region_code VARCHAR(20) NOT NULL,
    imported_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    data_date DATE NOT NULL,
    file_name TEXT NOT NULL,
    status VARCHAR(20) DEFAULT 'READY',
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    total_count BIGINT DEFAULT 0,
    -- ON CONFLICT 처리를 위한 유니크 제약조건 추가
    CONSTRAINT uq_osm_catalog_file_name UNIQUE (file_name)
);

-- 카탈로그 인덱스
CREATE INDEX IF NOT EXISTS idx_features_catalog_region_status ON osm_features_catalog (region_code, status);
