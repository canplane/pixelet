# Geo-Infrastructure Operational Guide

본 문서는 지리 정보 시스템(GIS) 인프라 구성 및 OSM 데이터 인제스션(Ingestion) 절차를 기술합니다.

## 1. Database Topology

| Service     | Port | Role                        | Stack          |
| :---------- | :--- | :-------------------------- | :------------- |
| **db-main** | 5432 | Business logic & Metadata   | PostgreSQL 16  |
| **db-geo**  | 5433 | Spatial data & Calculations | PostGIS 16-3.4 |

## 2. Data Management Rules

- **Standard**: ISO 3166-1 alpha-2 국가 코드를 사용함.
- **Storage**: `infra/osm-data/[COUNTRY_CODE]/` 경로에 원본 `.pbf` 파일을 보관함.
- **Audit**: `osm_data_catalog` 테이블을 통해 데이터 상태 및 주입 이력을 관리함.

## 3. Ingestion Workflow

### Step 1: Data Preparation

대상 지역의 `.pbf` 데이터를 다운로드하여 `infra/osm-data/[COUNTRY_CODE]/` 경로에 배치함.

### Step 2: Schema Initialization

공간 연산 확장 및 기초 테이블 스키마를 생성함.

```bash
docker exec -i db-geo psql -U admin -d geo < infra/postgres/init/01_osm_schema.sql
docker exec -i db-geo psql -U admin -d geo < infra/postgres/init/02_osm_catalog.sql
```

### Step 3: Ingestion Audit Start

데이터 주입 시작 전 상태를 `PROCESSING`으로 기록함.

```sql
INSERT INTO osm_data_catalog (country_code, data_date, file_name, status)
VALUES ('KR', CURRENT_DATE, 'south-korea-latest.osm.pbf', 'PROCESSING');
```

### Step 4: Raw Data Load (osm2pgsql)

`osm2pgsql`을 사용하여 로우 데이터를 임시 테이블(`planet_osm_*`)에 로드함.

```bash
docker run --rm -v $(pwd)/infra/osm-data:/data \
  --network=pixelet-network \
  openstreetmap/osm2pgsql:latest \
  -U admin -d geo -H db-geo \
  --slim --cache 1000 \
  --style /data/default.style \
  /data/[COUNTRY_CODE]/latest.osm.pbf
```

### Step 5: Data Migration & Refinement

임시 테이블 데이터를 최종 테이블로 이관함. 이 과정에서 좌표계 일치 작업 및 데이터 정제 로직을 수행함.

```sql
-- 1. Buildings: SRID 3857 변환, levels 데이터 정제, 중복 ID 처리
INSERT INTO osm_buildings (id, osm_id, name, building_type, levels, geom)
SELECT osm_id, osm_id, name, building,
       CASE WHEN "building:levels" ~ '^[0-9]+$' THEN "building:levels"::int ELSE 1 END,
       way
FROM planet_osm_polygon
WHERE building IS NOT NULL
ON CONFLICT (id) DO NOTHING;

-- 2. Roads: highway_type 매핑 및 oneway 데이터 변환
INSERT INTO osm_roads (id, osm_id, name, highway_type, oneway, geom)
SELECT osm_id, osm_id, name, highway,
       CASE WHEN oneway = 'yes' THEN true ELSE false END,
       way
FROM planet_osm_line
WHERE highway IS NOT NULL
ON CONFLICT (id) DO NOTHING;

-- 3. POIs: 핵심 태그 필터링 및 명칭 미보유 데이터 제외
INSERT INTO osm_pois (id, osm_id, name, poi_type, geom)
SELECT osm_id, osm_id, name,
       COALESCE(amenity, shop, tourism, leisure),
       way
FROM planet_osm_point
WHERE (amenity IS NOT NULL OR shop IS NOT NULL OR tourism IS NOT NULL OR leisure IS NOT NULL)
  AND name IS NOT NULL
ON CONFLICT (id) DO NOTHING;
```

### Step 6: Post-Processing & Audit Finish

이관 완료 후 임시 테이블을 제거하고 카탈로그 상태를 `COMPLETED`로 업데이트함.

```sql
-- 임시 테이블 제거
DROP TABLE IF EXISTS planet_osm_point, planet_osm_line, planet_osm_polygon, planet_osm_roads, osm2pgsql_properties;

-- 상태 업데이트
UPDATE osm_data_catalog
SET status = 'COMPLETED', updated_at = CURRENT_TIMESTAMP
WHERE country_code = 'KR' AND status = 'PROCESSING';
```

## 4. Maintenance Notes

- SRID Consistency: 모든 공간 데이터는 EPSG:3857 규격을 유지해야 함.

- Data Volume: `.pbf` 원본 파일은 형상 관리(Git) 대상에서 제외함.

- Idempotency: 모든 SQL 스크립트는 재실행 시 에러가 발생하지 않도록 `IF NOT EXISTS` 또는 `ON CONFLICT` 구문을 적용함.

## 5. Security & Sensitive Information

- **Credentials**: DB 접속 정보(비밀번호 등)는 절대 스크립트나 문서에 직접 기재하지 않음.
- **Environment Variables**: 주입 전 아래 환경 변수가 설정되어 있어야 함.
  ```bash
  export PGPASSWORD='your_secure_password'
  ```
- **Access Control**: `db-geo`는 외부 네트워크 접속을 차단하고 프로젝트 내부 네트워크(`pixelet-network`)에서만 접근 가능하도록 설정함.
