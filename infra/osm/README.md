# Geo-Infrastructure Operational Guide

본 문서는 지리 정보 시스템(GIS) 인프라 구성 및 OSM data ingestion 절차를 기술합니다.

## 1. Database Topology

| Service     | Port | Role                        | Stack          |
| :---------- | :--- | :-------------------------- | :------------- |
| **db-main** | 5432 | Business logic & Metadata   | PostgreSQL 16  |
| **db-geo**  | 5433 | Spatial data & Calculations | PostGIS 16-3.4 |

## 2. Data Management Rules

- **Standard**: ISO 3166-1 alpha-2 국가 코드를 사용함.
- **Storage**: `infra/osm/countries/[COUNTRY_CODE]/raw` 경로에 원본 `.pbf` 파일을 보관함.
- **Audit**: `osm_data_catalog` 테이블을 통해 데이터 상태 및 주입 이력을 관리함.

## 3. How to Run (Quick Start)

이 섹션은 표준 자동화 스크립트를 사용하여 전체 파이프라인을 실행하는 방법을 다룹니다.

### Prerequisites (macOS)

데이터 인프라 구축을 위해 맥 로컬 환경에 아래 도구들이 설치되어 있어야 합니다.

```bash
# 통합 설치 명령어
brew install osm2pgsql gdal postgis jq envsubst
```

- **osm2pgsql**: OSM PBF 데이터 적재 엔진
- **gdal**: 지형 데이터(GeoTIFF) 변환 및 처리 (Warping, VRT)
- **postgis**: DB 주입용 클라이언트 도구 (`raster2pgsql`) 포함
- **jq / envsubst**: 스크립트 내 JSON 파싱 및 환경 변수 치환용 유틸리티

주입 프로세스 시작 전, 아래의 준비 사항을 확인하십시오.

- **Raw Data**: `infra/osm/countries/[COUNTRY_CODE]/raw` 경로에 대상 지역의 `.pbf` 파일이 배치되어 있어야 합니다. (파일명은 `latest.osm.pbf`로 통일 권장)
  ```bash
  # 예: 대한민국 데이터 준비
  mkdir -p infra/osm/countries/KR/raw && cd infra/osm/countries/KR/raw
  curl -L -O https://download.geofabrik.de/asia/south-korea-latest.osm.pbf
  mv south-korea-latest.osm.pbf latest.osm.pbf
  ```
- **Environment**: 프로젝트 루트의 `.env` 파일에 `GEO_DB_` 관련 설정이 완료되어 있어야 합니다.
- **OSM Tool (Strongly Recommended)**: 데이터 주입 속도와 안정성을 위해 `osm2pgsql` 로컬 설치를 강력히 권장합니다.
  - **Mac**: `brew install osm2pgsql`
  - **Windows/Linux**: [공식 설치 가이드](https://osm2pgsql.org/doc/install.html) 참조
  - _로컬 미설치 시 스크립트가 도커 이미지 실행을 시도하지만, 네트워크 환경에 따라 이미지 Pull 에러가 발생할 수 있습니다._

### Execution Steps

현재 위치(`infra/osm/`)를 기준으로 실행하는 명령입니다.

```bash
# 1. 기존 볼륨 및 컨테이너 제거 (Clean Start가 필요한 경우)
# 상위 폴더의 설정을 참조하며 루트의 .env를 로드합니다.
docker compose -f ../docker-compose.yml --env-file ../../.env down -v

# 2. Geo Database 컨테이너 가동
# 프로젝트 전체 환경 변수를 주입하여 DB 엔진을 활성화합니다.
docker compose -f ../docker-compose.yml --env-file ../../.env up -d db-geo

# 3. 통합 주입 스크립트 실행
# scripts 폴더 내의 실행 권한을 부여하고 주입을 시작합니다.
chmod +x scripts/setup-geo-db.sh
./scripts/setup-geo-db.sh
```

## 4. Ingestion Workflow (Manual / Internal Logic)

이 섹션은 `setup-geo-db.sh` 스크립트가 내부적으로 수행하는 단계별 논리적 절차를 기술합니다. 수동 복구나 특정 단계 분석 시 참조하십시오.

### Step 1: Schema Initialization (Clean Start)

기존 데이터를 완전히 소거하고 공간 연산 확장 및 통합 테이블 스키마를 생성함.

```bash
# 레거시 및 기존 데이터 완전 소거 (Cascade 옵션 필수)
docker exec -i pixelet-db-geo psql -U pixelet_geo_admin -d pixelet_geo -c "DROP TABLE IF EXISTS osm_elements, osm_data_catalog CASCADE;"

# 스키마 및 통합 카탈로그 생성
docker exec -i pixelet-db-geo psql -U pixelet_geo_admin -d pixelet_geo < infra/osm/init/01_osm_schema.sql
```

### Step 2: Raw Data Load (osm2pgsql)

osm2pgsql을 사용하여 원본 데이터를 임시 테이블로 수집함.

- `--multi-geometry`: 다중 기하 정보를 하나의 행으로 처리하지 않고 분리하여 저장.

- `--hstore-all`: OSM의 모든 태그를 hstore 컬럼에 보존.

```bash
# 로컬 osm2pgsql 실행 (스크립트 표준)
osm2pgsql -U pixelet_geo_admin -d pixelet_geo -H localhost -P 5433 \
  --slim --cache 2000 \
  --hstore-all --extra-attributes \
  --multi-geometry \
  --number-processes 4 \
  --style infra/osm/default.style \
  infra/osm/countries/KR/raw/latest.osm.pbf
```

### Step 3: Audit Initialization

데이터 적재가 성공적으로 시작되었음을 기록하기 위해 상태를 `PROCESSING`으로 전환함.

```sql
UPDATE osm_data_catalog SET status = 'PROCESSING'
WHERE status = 'READY' AND country_code = 'KR';
```

### Step 4: Raw-to-Unified Migration (Sequential Merge)

임시 테이블(`planet_osm_*`)의 데이터를 `osm_elements`로 통합 이관함.

- ID 충돌 방지: 소스 테이블 내 중복 ID는 `DISTINCT ON`으로 필터링.

- 태그 병합(Tag Merging): 동일 ID가 다른 테이블(예: Line과 Polygon)에 존재할 경우, 기존 태그에 새로운 태그를 병합(`||`)하여 데이터 손실을 방지함.

`osm2pgsql`을 사용하여 원본 데이터를 임시 테이블로 수집함. (`--hstore-all` 옵션을 사용하여 모든 태그 전수 보관)

```sql
-- 1. Roads (지형 뼈대 우선 주입)
INSERT INTO osm_elements (osm_id, tags, way)
SELECT DISTINCT ON (osm_id) osm_id, tags, way FROM planet_osm_roads
ORDER BY osm_id
ON CONFLICT (osm_id) DO NOTHING;

-- 2. Polygon (면 데이터 주입 및 태그 병합)
INSERT INTO osm_elements (osm_id, tags, way)
SELECT DISTINCT ON (osm_id) osm_id, tags, way FROM planet_osm_polygon
ORDER BY osm_id
ON CONFLICT (osm_id) DO UPDATE SET tags = osm_elements.tags || EXCLUDED.tags;

-- 3. Line (선 데이터 주입 및 태그 병합)
INSERT INTO osm_elements (osm_id, tags, way)
SELECT DISTINCT ON (osm_id) osm_id, tags, way FROM planet_osm_line
ORDER BY osm_id
ON CONFLICT (osm_id) DO UPDATE SET tags = osm_elements.tags || EXCLUDED.tags;

-- 4. Point (점 데이터 주입 및 태그 병합)
INSERT INTO osm_elements (osm_id, tags, way)
SELECT DISTINCT ON (osm_id) osm_id, tags, way FROM planet_osm_point
ORDER BY osm_id
ON CONFLICT (osm_id) DO UPDATE SET tags = osm_elements.tags || EXCLUDED.tags;
```

### Step 5: Optimization & Audit Finish

조회 성능 극대화를 위해 통계 정보를 갱신(`ANALYZE`)하고 공간 인덱스 기준으로 물리적 정렬(`CLUSTER`)을 수행함.

```sql
-- 통계 갱신 및 물리적 정렬
ANALYZE osm_elements;
CLUSTER osm_elements USING idx_osm_elements_way;

-- 임시 자원 정리
DROP TABLE IF EXISTS planet_osm_point, planet_osm_line, planet_osm_polygon, planet_osm_roads, planet_osm_nodes, planet_osm_rels, planet_osm_ways, osm2pgsql_properties;

-- 최종 완료 처리
UPDATE osm_data_catalog SET status = 'COMPLETED', total_count = (SELECT count(*) FROM osm_elements)
WHERE status = 'PROCESSING' AND country_code = 'KR';
```

## 5. Maintenance Notes

- **SRID Consistency**: 모든 공간 데이터는 EPSG:3857(Web Mercator) 규격을 유지해야 함. 렌더링 엔진의 3D 복셀 연산 시 오차를 방지하기 위함임.
- **Data Volume**: `.pbf` 원본 파일은 대용량 바이너리이므로 형상 관리(Git) 대상에서 제외하고 `.gitignore`에 등록함.

- **Idempotency**: 모든 SQL 스크립트는 재실행 시 데이터 중복이 발생하지 않도록 `TRUNCATE` 또는 `DROP...IF EXISTS` 구문을 적용함.

- ** Network Context (Port & Host)**: 접속 환경에 따라 Host와 Port를 구분하여 사용해야 함.
  - **Local Access (Host)**: 외부 도구(DBeaver 등)나 API 로컬 개발 시에는 `localhost:5433`을 사용함.
  - **Container Access (Internal)**: 도커 네트워크 내부(`setup-geo-db.sh` 등)에서 통신할 때는 컨테이너 이름인 `pixelet-db-geo`와 기본 포트인 `5432`를 사용함.

## 6. Security & Sensitive Information

- **Credentials**: DB 접속 정보(비밀번호 등)는 절대 스크립트나 문서에 직접 기재하지 않음.
- **Environment Variables**: 주입 전 아래 환경 변수가 설정되어 있어야 함.
  ```bash
  export PGPASSWORD='your_secure_password'
  ```
- **Access Control**: `db-geo`는 외부 네트워크 접속을 차단하고 프로젝트 내부 네트워크(`pixelet-network`)에서만 접근 가능하도록 설정함.
