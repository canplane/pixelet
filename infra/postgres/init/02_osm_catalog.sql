-- [1] 데이터 수집 이력 관리 테이블
CREATE TABLE IF NOT EXISTS osm_data_catalog (
    id SERIAL PRIMARY KEY,
    country_code VARCHAR(2) NOT NULL,    -- ISO 코드 (KR, JP, US 등)
    imported_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP, -- 주입 완료 시점
    data_date DATE NOT NULL,            -- OSM 데이터 실제 생성 일자 (Geofabrik 시점)
    file_name TEXT NOT NULL,            -- 사용된 원본 파일명
    record_count_buildings BIGINT,      -- 주입된 건물 수 (품질 체크용)
    record_count_roads BIGINT,          -- 주입된 도로 수 (품질 체크용)
    status VARCHAR(20) DEFAULT 'READY'  -- READY, PROCESSING, COMPLETED, FAILED
);

-- [2] 국가 코드로 빠른 조회를 위해 인덱스 추가
CREATE INDEX idx_catalog_country ON osm_data_catalog (country_code);