# 🏔️ Elevation Infrastructure Operational Guide

본 문서는 지형 고도 데이터(Digital Elevation Model)를 수집하고 PostGIS 래스터 테이블로 통합하는 절차를 기술합니다.

## 1. Data Pipeline Strategy

지형 데이터 처리는 연산 효율성과 인프라 독립성을 위해 **Hybrid 방식**을 채택합니다.

- **Processing (Docker)**: 좌표계 투영 변환(`gdalwarp`) 및 가상 데이터셋 생성(`gdalbuildvrt`)은 `ghcr.io/osgeo/gdal` 컨테이너를 사용합니다.
- **Ingestion (Native)**: 대용량 래스터 데이터를 DB로 스트리밍하는 작업은 맥 로컬의 `raster2pgsql`을 사용하여 파이프라인 효율을 극대화합니다.

## 2. Directory Structure

- `countries/[COUNTRY_CODE]/raw/`: 원본 GeoTIFF 파일 (`.tif`, EPSG:4326)
- `countries/[COUNTRY_CODE]/warped/`: 투영 변환된 파일 (`.tif`, EPSG:3857)
- `countries/[COUNTRY_CODE]/metadata.json`: 처리 대상 파일 리스트 정의

## 3. How to Run

### Prerequisites (macOS)

- **Homebrew Tools**: `gdal`, `postgis`, `jq`, `envsubst` 설치 필수.
- **Environment**: 프로젝트 루트의 `.env`에 `GEO_DB_CONTAINER` 및 접속 정보 설정 완료.

### Execution Steps

현재 위치(`infra/elevation/`)를 기준으로 실행합니다.

```bash
chmod +x scripts/ingest-elevation.sh
./scripts/ingest-elevation.sh
```

## 4. Ingestion Workflow

- **Step 1: Warping (EPSG:4326 -> 3857)**: `gdalwarp`로 웹 메르카토르 재투영. 3D 엔진 정합성을 위해 `-r bilinear` 보간법을 적용합니다.
- **Step 2: VRT Indexing**: 여러 TIFF 파일을 하나의 가상 레이어(`index.vrt`)로 통합합니다.
- **Step 3: DB Setup & Ingestion**:
  - **Extension**: `postgis_raster` 확장을 자동 활성화하여 `raster` 타입을 준비합니다.
  - **Streaming**: `raster2pgsql`을 통해 `index.vrt`를 SQL 스트림으로 변환 후 DB로 직접 주입합니다.
  - **Options**: `-t 256x256`(타일링), `-I -C -M`(인덱스 및 제약 조건) 적용.

## 5. Maintenance Notes

- **Self-Healing**: 스크립트 실행 시 `CREATE EXTENSION`을 포함하여 초기화되지 않은 DB 환경에서도 작동을 보장합니다.
- **Optimization**: 주입 완료 후 `ANALYZE`를 실행하여 공간 쿼리 플래너의 성능을 최적화합니다.
- **Z-Axis Alignment**: 적재된 고도 데이터는 osm_elements 벡터 데이터와 독립적으로 유지되며, 런타임 시 3D 객체를 지면에 안착시키기 위한 **지형 참조 기준(Ground Zero)**으로 활용됩니다.
