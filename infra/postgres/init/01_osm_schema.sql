-- [1] 공간 연산 확장팩 설치
CREATE EXTENSION IF NOT EXISTS postgis;

-- [2] 건물 데이터 테이블 (Polygon)
-- 3D 렌더링이나 지도에서 면(Area)으로 보이는 것들을 저장합니다.
CREATE TABLE IF NOT EXISTS osm_buildings (
    id BIGINT PRIMARY KEY,
    name TEXT,
    building_type TEXT, 
    levels INTEGER DEFAULT 1,     -- 건물 층수 (3D 높이 계산용)
    tags JSONB,                    -- 기타 비정형 데이터 (JSON 형태)
    geom GEOMETRY(Polygon, 4326),  -- 지도상의 좌표 데이터
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

-- [3] 도로 및 경로 데이터 테이블 (LineString)
-- 길찾기나 지도에서 선(Line)으로 보이는 것들을 저장합니다.
CREATE TABLE IF NOT EXISTS osm_roads (
    id BIGINT PRIMARY KEY,
    name TEXT,
    highway_type TEXT, 
    oneway BOOLEAN DEFAULT FALSE,
    tags JSONB,
    geom GEOMETRY(LineString, 4326),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

-- [4] 관심 지점(POI) 데이터 테이블 (Point)
-- 음식점, 지하철역 등 점(Point)으로 보이는 것들을 저장합니다.
CREATE TABLE IF NOT EXISTS osm_pois (
    id BIGINT PRIMARY KEY,
    name TEXT,
    poi_type TEXT,      -- cafe, restaurant, station 등
    tags JSONB,
    geom GEOMETRY(Point, 4326),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

-- [5] 공간 인덱스 생성 (성능의 핵심)
-- 이 인덱스가 있어야 수만 개의 건물 중 '내 주변' 것만 0.001초 만에 찾아냅니다.
CREATE INDEX idx_buildings_geom ON osm_buildings USING GIST (geom);
CREATE INDEX idx_roads_geom ON osm_roads USING GIST (geom);
CREATE INDEX idx_pois_geom ON osm_pois USING GIST (geom);