# Geo-Infrastructure Operational Guide

본 문서는 pixelet 지리 정보 시스템(GIS)의 인프라 구성, 데이터 파이프라인 및 운영 절차를 기술합니다.
인프라는 **Vector(지형지물)**와 **Raster(고도)** 두 개의 독립적인 레이어로 구성되며, 단일 PostGIS 인스턴스(`db-geo`) 내에서 통합 관리됩니다.

## 1. Database Topology

| Service     | Port (Ext:Int) | Role                                        | Stack          |
| :---------- | :------------- | :------------------------------------------ | :------------- |
| **db-main** | 5432:5432      | Business logic & Metadata                   | PostgreSQL 16  |
| **db-geo**  | 5433:5432      | **Spatial Data Hub** (Features + Elevation) | PostGIS 16-3.4 |

## 2. Data Management Rules

데이터는 성격에 따라 두 가지 파이프라인으로 분리되어 관리됩니다. 두 파이프라인은 동일한 설계 철학(Symmetry)을 공유합니다.
(경로는 본 문서가 위치한 `infra/geo/` 기준 상대 경로입니다.)

| Feature         | **OSM Features (Vector)**                                     | **ESA Elevation (Raster)**    |
| :-------------- | :------------------------------------------------------------ | :---------------------------- |
| **Source**      | OpenStreetMap (PBF)                                           | Copernicus GLO-30 (GeoTIFF)   |
| **Directory**   | `./osm-features/`                                             | `./esa-elevation/`            |
| **Raw Data**    | `../../region/[CODE]/*.osm.pbf`                               | `../../region/[CODE]/*.tif`   |
| **Main Table**  | 1. `osm_elements` **(SSOT)**<br>2. `osm_roads` **(Reserved)** | `esa_elevation_raster`        |
| **Catalog**     | `osm_features_catalog`                                        | `esa_elevation_catalog`       |
| **Schema File** | `01_osm_features_schema.sql`                                  | `01_esa_elevation_schema.sql` |
| **SRID**        | EPSG:3857 (Web Mercator)                                      | EPSG:3857 (Web Mercator)      |

## 3. How to Run (Quick Start)

이 섹션은 표준 자동화 스크립트를 사용하여 전체 파이프라인을 실행하는 방법을 다룹니다.

### Prerequisites (macOS)

데이터 인프라 구축을 위해 맥 로컬 환경에 아래 도구들이 설치되어 있어야 합니다.

```bash
# 통합 도구 설치
brew install osm2pgsql gdal postgis jq envsubst
```

- **osm2pgsql**: Vector 데이터 적재 엔진 (PBF 처리)
- **gdal**: Raster 데이터 변환(Warping) 및 VRT 생성
- **postgis**: DB 주입용 클라이언트 (`raster2pgsql` 포함)
- **jq / envsubst**: 메타데이터 파싱 및 환경 변수 처리

### Execution Steps

모든 명령은 **프로젝트 루트(`pixelet/`)**를 기준으로 실행합니다.

#### 1. Geo Database 컨테이너 기동 및 헬스 체크

루트의 `.env`를 참조하여 컨테이너를 가동합니다.

```bash
# 1. 기존 볼륨 정리 및 컨테이너 기동
docker compose -f infra/docker-compose.yml --env-file .env down -v
docker compose -f infra/docker-compose.yml --env-file .env up -d db-geo

# 2. DB 접속 가능 여부 확인 (Health Check)
echo "Waiting for PostGIS to be ready..."
until docker exec $(docker compose -f infra/docker-compose.yml ps -q db-geo) pg_isready -U postgres; do
  echo "DB is unavailable - sleeping..."
  sleep 2
done
echo "DB is ready!"
```

#### 2. 레이어별 데이터 주입 (순서 무관)

각 스크립트는 내부적으로 루트(`.env`)의 환경 변수를 로드하여 인증 및 접속 정보를 처리합니다.

```bash
# [Layer 1] OSM Features (도로, 건물 등) 구축
chmod +x infra/geo/osm-features/scripts/setup-osm-features.sh
./infra/geo/osm-features/scripts/setup-osm-features.sh

# [Layer 2] ESA Elevation (고도 정보) 구축
chmod +x infra/geo/esa-elevation/scripts/setup-esa-elevation.sh
./infra/geo/esa-elevation/scripts/setup-esa-elevation.sh
```

## 4. Ingestion Workflow (Deep Dive)

각 파이프라인의 내부 동작 로직입니다. 디버깅 및 유지보수 시 참조하십시오.

### Pipeline A: OSM Features (Vector)

#### Step 1: Schema Initialization

- 기존 테이블(`osm_elements, osm_roads`) 및 카탈로그(`osm_features_catalog`)를 삭제(DROP)합니다.
- `init/01_osm_features_schema.sql`을 주입하여 테이블과 인덱스를 재생성합니다.

#### Step 2: Raw Processing (osm2pgsql)

- `metadata.json`을 순회하며 PBF 데이터를 처리합니다.
- 여러 리전의 데이터는 Append 모드로 하나의 임시 테이블 세트에 통합됩니다.
- `--hstore-all`, `--multi-geometry` 옵션으로 원본 데이터를 유실 없이 보존합니다.

#### Step 3: Sequential Merge (Migration)

임시 테이블의 데이터를 목적에 맞게 두 개의 테이블로 분리하여 이관합니다.

1. **Vector SSOT (`osm_elements`)**:

- `point`, `line`, `polygon` 테이블 데이터를 통합합니다.
- **Type Discrimination**: `type` 컬럼을 추가하여 ID 충돌을 방지합니다.
- **High-fidelity**: 원본 형상을 그대로 보존합니다.

2. **Generalized Roads (`osm_roads`)**:

- `planet_osm_roads` 테이블 데이터를 별도로 저장합니다.
- **Optimization**: 줌 아웃(Zoom-out) 시 렌더링 성능 최적화를 위한 단순화된 형상 데이터입니다.

#### Step 4: Cleanup

- 임시 테이블을 정리하고 카탈로그 상태를 `COMPLETED`로 갱신합니다.

---

### Pipeline B: ESA Elevation (Raster)

#### Step 1: Schema Initialization

- 기존 `esa_elevation_raster` 및 `esa_elevation_catalog`를 삭제(DROP)합니다.
- `init/01_esa_elevation_schema.sql`을 주입하여 카탈로그 테이블을 생성합니다. (Raster 테이블은 Tool이 자동 생성)

#### Step 2: Reprojection (GDAL Warp)

- Raw GeoTIFF(EPSG:4326)를 시스템 표준인 **EPSG:3857**로 투영 변환(Warping)합니다.

#### Step 3: Indexing & Ingestion

- 변환된 파일들을 `gdalbuildvrt`로 묶습니다.
- `raster2pgsql`을 사용하여 적재합니다.
  - `-s 3857`: 좌표계 명시
  - `-t 256x256`: 타일 크기 지정 (Voxel Grid 정합성)
  - `-I`: 공간 인덱스(GiST) 생성
  - `-C`: **Raster Constraints** 적용 (픽셀 크기, SRID 등의 제약조건을 메타데이터 테이블에 등록하여 쿼리 최적화 지원)

#### Step 4: Cleanup

- **Vacuum Analyze**: 대량의 Raster 데이터 적재 후 데드 튜플(Dead Tuples) 정리와 통계 갱신을 위해 `VACUUM ANALYZE esa_elevation_raster;`를 필수적으로 수행합니다.
- 카탈로그 상태를 `COMPLETED`로 갱신합니다.

## 5. Maintenance Notes

- **Verification Query**: 작업 완료 후 데이터 적재 여부를 검증하기 위해 아래 쿼리를 활용하십시오.

  ```sql
  -- Vector Count Check
  SELECT type, count(*) FROM osm_elements GROUP BY type;

  -- Raster Metadata Check
  SELECT r_table_name, r_raster_column, srid, scale_x, scale_y
  FROM raster_columns
  WHERE r_table_name = 'esa_elevation_raster';
  ```

- **SRID Consistency**: 모든 공간 데이터는 **EPSG:3857(Web Mercator)** 규격을 엄수해야 합니다. 이는 렌더링 엔진의 3D 복셀 연산 시 좌표 오차를 방지하기 위함입니다.

- **Idempotency**: 모든 스크립트는 재실행 시 데이터 중복이나 충돌이 발생하지 않도록 **Clean Start(DROP & CREATE)** 또는 **Upsert** 로직을 포함합니다.

- **Directory Structure**:

  ```
  infra/geo/
  ├── osm-features/  # Vector Layer
  └── esa-elevation/ # Raster Layer
  ```

- **Network Context**:
  - **Local Access**: `localhost:5433` (외부 도구 사용 시)

  - **Container Access**: `db-geo:5432` (Docker 내부 통신 시)

## 6. Security & Sensitive Information

- **Credentials**: DB 비밀번호 등 민감 정보는 `.env` 파일을 통해서만 주입하며, 문서나 코드에 하드코딩하지 않습니다.

- **Environment Variables**: 실행 전 `GEO_DB_USER`, `GEO_DB_PASSWORD` 등이 환경 변수로 로드되어 있어야 합니다.
