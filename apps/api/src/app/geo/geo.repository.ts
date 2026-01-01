/**
 * @file geo.repository.ts
 * PostGIS 쿼리 실행 및 Raw 데이터 Fetch를 담당하는 저장소 계층입니다.
 */

import { Injectable } from '@nestjs/common';
import { InjectRepository } from '@nestjs/typeorm';
import { Repository } from 'typeorm';
import { OsmElement } from '@/app/geo/entities/osm-element.entity';
import { OSM_CONSTANTS } from '@pixelet/shared/entities/osm/model/osm-constants';
import type { Coord } from '@pixelet/shared/shared/model/coords';
import type { Bbox } from '@pixelet/shared/entities/osm/lib/utils/tile-utils';

const { Z_SAMPLING_STEP } = OSM_CONSTANTS;

const PAD = 1;

@Injectable()
export class GeoRepository {
  constructor(
    @InjectRepository(OsmElement, 'geoConnection')
    private readonly repo: Repository<OsmElement>,
  ) {}

  /**
   * [fetchElevationPoints]
   * PostGIS 래스터 쿼리를 통해 고도 데이터를 샘플링하여 Coord 배열로 반환합니다.
   *
   * @strategy Union + Padding + South-to-North Order
   * 1. Padding: 타일 경계(Seam)의 보간 오차를 방지하기 위해 상하좌우 1칸(step)씩 더 넓게 조회합니다.
   * 2. Union: 여러 SRTM/ESA 파일에 걸쳐 있는 타일의 경우, 조각난 래스터를 병합(ST_Union)합니다.
   * 3. 🎯 South-to-North Order (Critical):
   * PostGIS의 Top-Down 배열을 역순으로 순회하여 월드 좌표계(South-to-North)와 일치시킵니다.
   * 이로써 별도의 Y-Flip 없이 Three.js의 UV 좌표계(0,0 = South-West)와 직접 정합됩니다.
   */
  async fetchElevationPoints(
    bbox: Bbox,
    step: number = Z_SAMPLING_STEP,
  ): Promise<Coord[]> {
    // 1. Calculate Grid Dimensions
    // 예: 폭 100m, step 10m -> 11개 포인트 필요
    const innerWidth = Math.floor((bbox.maxX - bbox.minX) / step) + 1;
    const innerHeight = Math.floor((bbox.maxY - bbox.minY) / step) + 1;

    // 2. Apply Padding (상하좌우 1칸)
    const paddedWidth = innerWidth + PAD * 2; // 35 = 33 + 2
    const paddedHeight = innerHeight + PAD * 2; // 35 = 33 + 2

    // 3. 🎯 [Critical Fix] Extent Calculation
    // SQL의 ST_Resample이 이미지를 찌그러뜨리지 않도록,
    // 픽셀 개수(paddedWidth)에 맞춰 물리적 거리(step)를 곱해 Extent를 재정의합니다.
    const paddedMinX = bbox.minX - step * PAD;
    const paddedMaxY = bbox.maxY + step * PAD;

    // ❌ 기존: const paddedMaxX = bbox.maxX + step * pad; (오차 발생 원인)
    // ✅ 수정: 시작점 + (개수 * 간격)으로 정확한 끝점을 계산
    const paddedMaxX = paddedMinX + paddedWidth * step;
    // Y축은 PostGIS 래스터 특성상(Top-Down) MinY도 맞춰줘야 안전합니다.
    const paddedMinY = paddedMaxY - paddedHeight * step;

    const query = `
      WITH interest_area AS (
        SELECT ST_MakeEnvelope(
          ${paddedMinX}, ${paddedMinY}, ${paddedMaxX}, ${paddedMaxY}, 3857
        ) as geom
      ),
      clipped_chunks AS (
        SELECT ST_Clip(r.rast, i.geom) as rast
        FROM esa_elevation_raster r, interest_area i
        WHERE ST_Intersects(r.rast, i.geom)
      ),
      unioned_rast AS (
        -- [Merge] 경계선에 걸친 여러 래스터 조각을 하나로 합병
        SELECT ST_Union(rast) as rast
        FROM clipped_chunks
      ),
      resampled_rast AS (
        -- [Resample] 요청한 해상도(paddedWidth/Height)에 맞춰 픽셀 재배치
        SELECT ST_Resample(
          rast,
          ${paddedWidth}, ${paddedHeight},
          ${paddedMinX}, ${paddedMaxY},
          0, 0
        ) as rast
        FROM unioned_rast
      )
      -- [Extract] 2D 배열 형태의 값 추출
      SELECT (ST_DumpValues(rast)).valarray as z_values
      FROM resampled_rast;
    `;

    const result = await this.repo.query(query);
    if (!result || result.length === 0 || !result[0].z_values) return [];

    const zValues: number[][] = result[0].z_values; // PostGIS Standard: [Row 0 = North]
    const points: Coord[] = [];

    /**
     * 3. 🎯 [Natural Fix] 인덱스 매핑만 교정
     * PostGIS 래스터의 바닥(마지막 row)이 월드 좌표계의 남쪽(MinY)입니다.
     * 따라서 바닥 row부터 위로 올라가며 수집하면 자연스럽게 남->북 순서가 됩니다.
     */
    for (let row = paddedHeight - 1; row >= 0; row--) {
      // row가 줄어들수록(위로 올라갈수록) worldY는 증가함
      const worldY = paddedMaxY - row * step;

      for (let col = 0; col < paddedWidth; col++) {
        // col이 증가할수록(오른쪽으로 갈수록) worldX는 증가함
        const worldX = paddedMinX + col * step;
        const z = zValues[row]?.[col] ?? 0; // PostGIS 배열 접근 안전장치

        points.push({ x: worldX, y: worldY, z });
      }
    }

    return points;
  }

  /**
   * [fetchRawMapData]
   * 해당 타일 영역(BBox)에 포함되거나 걸쳐있는 모든 OSM 요소를 조회합니다.
   * 변환 전의 Raw Row 상태를 반환합니다.
   */
  async fetchRawMapData(bbox: Bbox): Promise<any[]> {
    const area3857 = `ST_MakeEnvelope(${bbox.minX}, ${bbox.minY}, ${bbox.maxX}, ${bbox.maxY}, 3857)`;

    return await this.repo.query(`
      SELECT 
        osm_id, 
        tags, 
        ST_AsGeoJSON(ST_SimplifyPreserveTopology(way, 0.1)) as geometry, 
        ST_AsText(ST_Centroid(way)) as cwkt
      FROM osm_elements
      WHERE way && ${area3857} 
        OR ST_Intersects(way, ${area3857})
    `);
  }

  /**
   * [fetchCoastlines]
   * 전용 해안선 테이블(osm_coastline_polygons)에서 데이터를 조회합니다.
   * fetchRawMapData와 동일하게 GeoJSON 규격을 사용하여 데이터 파이프라인의 통일성을 유지합니다.
   */
  async fetchCoastlines(bbox: Bbox): Promise<any[]> {
    const area3857 = `ST_MakeEnvelope(${bbox.minX}, ${bbox.minY}, ${bbox.maxX}, ${bbox.maxY}, 3857)`;

    return await this.repo.query(`
      SELECT 
        fid as id,
        ST_AsGeoJSON(geom) as geometry 
      FROM osm_coastline_polygons
      WHERE geom && ${area3857}
        AND ST_IsValid(geom)
    `);
  }
}
