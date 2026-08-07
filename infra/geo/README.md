# Geo-Infrastructure Operational Guide

본 문서는 pixelet 지리 정보 시스템(GIS)의 인프라 구성, 데이터 파이프라인 및 운영 절차를 기술합니다.
인프라는 **Vector(지형지물, 해안선)**와 **Raster(고도)** 등 독립적인 레이어로 구성되며, 단일 PostGIS 인스턴스(`db-geo`) 내에서 통합 관리됩니다.

## 1. Database Topology

| Service     | Port (Ext:Int) | Role                                        | Stack          |
| :---------- | :------------- | :------------------------------------------ | :------------- |
| **db-main** | 5432:5432      | Business logic & Metadata                   | PostgreSQL 16  |
| **db-geo**  | 5433:5432      | **Spatial Data Hub** (Features + Elevation) | PostGIS 16-3.4 |

## 2. Data Management Rules

데이터는 성격에 따라 세 가지 파이프라인으로 분리되어 관리됩니다. 모든 파이프라인은 동일한 설계 철학(Symmetry)을 공유합니다.
(경로는 본 문서가 위치한 `infra/geo/` 기준 상대 경로입니다.)

| Feature         | **OSM Features (Vector)**                                        | **ESA Elevation (Raster)**                | **OSM Coastline (Vector)**                |
| :-------------- | :--------------------------------------------------------------- | :---------------------------------------- | :---------------------------------------- |
| **Source**      | OpenStreetMap (PBF)                                              | Copernicus GLO-30 (GeoTIFF)               | OSM Coastline (Water Polygons)            |
| **Directory**   | `./osm-features/`                                                | `./esa-elevation/`                        | `./osm-coastline/`                        |
| **Raw Data**    | `./osm-features/region/[CODE]/raw/*.osm.pbf`                     | `./esa-elevation/region/[CODE]/raw/*.tif` | `./osm-coastline/region/global/raw/*.shp` |
| **Main Table**  | 1. `osm_elements` **(Primary)**<br>2. `osm_roads` **(Reserved)** | `esa_elevation_raster`                    | `osm_coastline_polygons` **(Primary)**    |
| **Catalog**     | `osm_features_catalog`                                           | `esa_elevation_catalog`                   | `osm_coastline_catalog`                   |
| **Schema File** | `01_osm_features_schema.sql`                                     | `01_esa_elevation_schema.sql`             | `01_osm_coastline_schema.sql`             |
| **SRID**        | EPSG:3857 (Web Mercator)                                         | EPSG:3857 (Web Mercator)                  | EPSG:3857 (Web Mercator)                  |

## 3. How to Run (Quick Start)

이 섹션은 표준 자동화 스크립트를 사용하여 전체 파이프라인을 실행하는 방법을 다룹니다.

### Prerequisites (macOS)

데이터 인프라 구축을 위해 맥 로컬 환경에 아래 도구들이 설치되어 있어야 합니다.

```bash
# 1. 통합 도구 설치 (envsubst 포함을 위해 gettext 설치 필요)
brew install osm2pgsql gdal postgis jq gettext

# 2. gettext 도구(envsubst 등) 시스템 경로 연결
brew link --force gettext
```

- **osm2pgsql**: Vector 데이터 적재 엔진 (PBF 처리)
- **gdal**: Raster 및 Vector 데이터 변환 도구 (Warping, `ogr2ogr`, `raster2pgsql` 등 포함)
- **postgis**: PostgreSQL 공간 데이터 확장팩 및 DB 주입 유틸리티
- **jq**: JSON 메타데이터 파싱 및 조작
- **gettext (envsubst)**: 쉘 스크립트 내 환경 변수 치환(Substitution) 처리

### Execution Steps

모든 명령은 **프로젝트 루트(`pixelet/`)**를 기준으로 실행합니다.

#### 1. Geo Database 컨테이너 기동 및 헬스 체크

루트의 `.env`를 참조하여 컨테이너를 가동합니다.

```bash
# 1. 인프라 기동 (기존 데이터 볼륨 삭제 후 클린 시작)
docker compose -f infra/docker-compose.yml --env-file .env down -v
docker compose -f infra/docker-compose.yml --env-file .env up -d db-geo

# 2. .env 환경 변수를 현재 쉘 세션에 로드
export $(grep -v '^#' .env | xargs)

# 3. DB 접속 가능 여부 확인 (Health Check)
echo "Waiting for PostGIS [$DB_GEO_CONTAINER] to be ready..."
until docker exec "$DB_GEO_CONTAINER" pg_isready -U postgres; do
  echo 'DB is unavailable - sleeping...'
  sleep 2
done
echo '✅ DB is ready!'
```

#### 2. 레이어별 데이터 주입 (순서 무관)

각 스크립트는 내부적으로 루트(`.env`)의 환경 변수를 로드하여 인증 및 접속 정보를 처리합니다.

```bash
# [Channel 1] OSM Features (도로, 건물 등) 구축
chmod +x infra/geo/osm-features/scripts/setup-osm-features.sh
./infra/geo/osm-features/scripts/setup-osm-features.sh

# [Channel 2] ESA Elevation (고도 정보) 구축
chmod +x infra/geo/esa-elevation/scripts/setup-esa-elevation.sh
./infra/geo/esa-elevation/scripts/setup-esa-elevation.sh

# [Channel 3] OSM Coastline (해안선/해수면) 구축
chmod +x infra/geo/osm-coastline/scripts/setup-osm-coastline.sh
./infra/geo/osm-coastline/scripts/setup-osm-coastline.sh
```

## 4. Ingestion Workflow (Deep Dive)

각 파이프라인의 내부 동작 로직입니다. 디버깅 및 유지보수 시 참조하십시오. 모든 레이어는 데이터 무결성을 위해 **Clean Start(Drop & Create)**와 **Catalog 기반 상태 관리**를 공유합니다.

### Pipeline A: OSM Features (Vector)

#### Step 1: Schema Initialization

- 기존 테이블(`osm_elements`, `osm_roads`) 및 카탈로그(`osm_features_catalog`)를 삭제(DROP)하여 잔류 데이터로 인한 오염을 방지합니다.
- `init/01_osm_features_schema.sql`을 실행하여 PostGIS 확장팩 확인, 테이블 정의 및 공간/태그 인덱스를 재생성합니다.

#### Step 2: Raw Processing (osm2pgsql)

- `metadata.json`을 순회하며 대상 PBF 데이터를 처리합니다.
- **`--hstore-all`** 옵션을 핵심으로 사용하여, 별도 컬럼 매핑 없이 **모든 속성 데이터를 `tags` 컬럼에 보존(Lossless)**합니다. 이는 스키마 변경 없이 어떤 태그든 수용할 수 있는 유연성을 제공합니다.
- `--multi-geometry` 옵션을 사용하여 원본 데이터의 형상을 유실 없이 보관합니다.

#### Step 3: Sequential Merge & Optimization

- **Migration**: 쉘 스크립트 에러 방지를 위해 임시 SQL 파일을 생성합니다. 임시 테이블(`planet_osm_point` 등)의 데이터를 메인 테이블(`osm_elements`)로 이관합니다. 이때 `hstore`로 통합된 태그와 형상 정보가 단일 테이블로 모입니다.
- **Physical Clustering**: 데이터 이관이 완료된 직후, 공간 인덱스(`GiST`)를 기준으로 **`CLUSTER` 명령을 수행**하여 디스크 상의 데이터 배치를 물리적으로 최적화합니다. (범위 검색 성능 극대화)
- **Generalized Roads (`osm_roads`)**: 줌 아웃 시 렌더링 성능을 위해 단순화된 형상 데이터를 별도 보관합니다.

#### Step 4: Finalization & Catalog Update

- 임시 테이블(`planet_osm_*`)을 정리(DROP)하고, `osm_features_catalog`의 상태를 `COMPLETED`로 업데이트하며 처리된 총 레코드 수를 기록합니다.

---

### Pipeline B: ESA Elevation (Raster)

#### Step 1: Schema Initialization

- 기존 `esa_elevation_raster` 및 `esa_elevation_catalog`를 삭제합니다.
- `init/01_esa_elevation_schema.sql`을 주입하여 카탈로그 구조를 생성합니다. (Raster 테이블은 주입 도구인 `raster2pgsql`에 의해 자동 관리됩니다.)

#### Step 2: Reprojection (GDAL Warp)

- Docker 컨테이너(`/data`) 환경 내에서 `gdalwarp`를 실행합니다.
- Raw GeoTIFF(EPSG:4326)를 시스템 표준인 **EPSG:3857**로 투영 변환하며, 효율적인 스토리지를 위해 `DEFLATE` 압축을 적용합니다.

#### Step 3: Indexing & Ingestion (Hybrid Workflow)

- **Path Mapping**: Docker 컨테이너용 경로(`/data`) 치환 리스트를 통해 `gdalbuildvrt`를 실행, 가상 래스터 인덱스를 생성합니다.
- **Path Symmetry**: 생성된 VRT 내부의 가상 경로를 다시 **로컬 호스트 경로**로 환원(sed)하여 로컬 도구가 실제 파일을 짚을 수 있게 합니다.
- **Stream Ingestion**: 로컬의 `raster2pgsql`과 Docker의 `psql`을 파이프로 연결하여 주입합니다.
  - `-t 256x256`: 3D Voxel Grid 연산 최적화를 위한 페이지 규격 준수

#### Step 4: Finalization & Catalog Update

- 주입된 데이터에 대해 `ANALYZE`를 수행하고, `esa_elevation_catalog`의 상태를 `COMPLETED`로 업데이트하여 가용 상태임을 표시합니다.

---

### Pipeline C: OSM Coastline (Vector)

#### Step 1: Schema Initialization

- 기존 테이블(`osm_coastline_polygons`) 및 카탈로그(`osm_coastline_catalog`)를 삭제합니다.
- `init/01_osm_coastline_schema.sql`을 실행하여 해안선 전용 테이블과 공간 인덱스를 재생성합니다.

#### Step 2: Raw Processing (ogr2ogr)

- GDAL의 `ogr2ogr` 도구를 사용하여 Shapefile(.shp) 데이터를 PostGIS로 직접 적재합니다.
- `metadata.json`의 정의에 따라 전지구 해안선 데이터를 순회하며, `-update -append` 옵션을 통해 단일 테이블로 물리적 통합을 수행합니다.

#### Step 3: Topology Validation & Optimization

- 쉘 해석 에러 방지 및 작업 원자성 확보를 위해 **`temp_optimization.sql` 파일**을 생성하여 실행합니다.
- **ST_MakeValid**: 데이터 주입 중 발생하는 기하학적 오류(Self-intersection 등)를 자동 보정합니다.
- **Spatial Clustering**: GiST 인덱스 기반으로 `CLUSTER` 명령을 수행하여 공간 검색 속도를 최적화합니다.

#### Step 4: Finalization & Catalog Update

- 임시 SQL 파일을 제거하고, `osm_coastline_catalog`의 상태를 `COMPLETED`로 업데이트하며 최종 주입된 객체 수를 동기화합니다.

## 5. Maintenance Notes

- **Verification Query**: 작업 완료 후 데이터 적재 여부를 검증하기 위해 아래 쿼리를 활용하십시오.

  ```sql
  -- Vector Count Check
  SELECT type, count(*) FROM osm_elements GROUP BY type;

  -- Raster Metadata Check
  SELECT r_table_name, r_raster_column, srid, scale_x, scale_y
  FROM raster_columns
  WHERE r_table_name = 'esa_elevation_raster';

  -- Coastline Validity Check
  SELECT count(*) as total, count(*) FILTER (WHERE ST_IsValid(geom)) as valid FROM osm_coastline_polygons;
  ```

- **SRID Consistency**: 모든 공간 데이터는 **EPSG:3857(Web Mercator)** 규격을 엄수해야 합니다. 이는 렌더링 엔진의 3D 복셀 연산 시 좌표 오차를 방지하기 위함입니다.
- **Idempotency**: 모든 스크립트는 재실행 시 데이터 중복이나 충돌이 발생하지 않도록 **Clean Start(DROP & CREATE)** 또는 **Upsert** 로직을 포함합니다.
- **Directory Structure**:

  ```
  infra/geo/
  ├── osm-features/   # Vector Layer (Roads, Buildings)
  ├── esa-elevation/  # Raster Layer (Height)
  └── osm-coastline/  # Vector Layer (Water Polygons)
  ```

- **Network Context**:
  - **Local Access**: `localhost:5433` (외부 도구 사용 시)
  - **Container Access**: `db-geo:5432` (Docker 내부 통신 시)

- **Backup & Recovery Strategy**: 레이어별 독립적 복구를 위해 전체 DB 백업 외에 레이어별 백업 파일 생성을 권장합니다.

  ```bash
  # 1. 특정 레이어 백업 (예: Coastline)
  docker exec -t $DB_GEO_CONTAINER pg_dump -U $PGUSER -d $PGDATABASE \
    -t osm_coastline_polygons -t osm_coastline_catalog > osm_coastline_backup.sql

  # 2. 고도 데이터(Raster) 백업 (압축 권장)
  docker exec -t $DB_GEO_CONTAINER pg_dump -U $PGUSER -d $PGDATABASE \
    -t public.esa_elevation_raster -t public.esa_elevation_catalog | gzip > esa_elevation_backup.sql.gz

  # 3. 데이터 복구 (Restore)
  cat osm_coastline_backup.sql | docker exec -i $DB_GEO_CONTAINER psql -U $PGUSER -d $PGDATABASE
  ```

- **Storage Optimization**: 대규모 데이터 주입(Clean Start) 직후에는 데드 튜플(Dead Tuples) 정리가 필수적입니다. 데이터 적재가 끝난 후 아래 명령을 통해 통계 정보를 갱신하고 스토리지 회수를 수행하십시오.
  ```sql
  VACUUM ANALYZE;      -- 통계 갱신 및 일반 정리 (운영 중 권장)
  -- or
  VACUUM FULL ANALYZE; -- 물리적 디스크 공간 회수 (다운타임 허용 시 권장)
  ```

## 6. Security & Sensitive Information

- **Credentials**: DB 비밀번호 등 민감 정보는 `.env` 파일을 통해서만 주입하며, 문서나 코드에 하드코딩하지 않습니다.
- **Environment Variables**: 실행 전 `DB_GEO_USER`, `DB_GEO_PASSWORD` 등이 환경 변수로 로드되어 있어야 합니다.
