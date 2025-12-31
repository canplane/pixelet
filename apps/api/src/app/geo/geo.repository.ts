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
   * @strategy Union + Padding + Standard Order
   * 1. Padding: 타일 경계(Seam)의 보간 오차를 없애기 위해 상하좌우 1칸씩 더 넓게 조회합니다.
   * 2. Union: 여러 SRTM 파일에 걸쳐 있는 타일의 경우, 조각난 래스터를 병합합니다.
   * 3. Order: 이미지 표준인 Top-Down(북->남) 순서를 유지하여 Three.js 텍스처 좌표계와 정합성을 맞춥니다.
   */
  async fetchElevationPoints(
    bbox: Bbox,
    step: number = Z_SAMPLING_STEP,
  ): Promise<Coord[]> {
    // 1. Calculate Grid Dimensions (Inner Tile)
    const innerWidth = Math.floor((bbox.maxX - bbox.minX) / step) + 1;
    const innerHeight = Math.floor((bbox.maxY - bbox.minY) / step) + 1;

    // 2. Apply Padding (1 cell margin)
    const pad = 1;
    const paddedWidth = innerWidth + pad * 2;
    const paddedHeight = innerHeight + pad * 2;

    const paddedMinX = bbox.minX - step * pad;
    const paddedMaxX = bbox.maxX + step * pad;
    const paddedMinY = bbox.minY - step * pad;
    const paddedMaxY = bbox.maxY + step * pad;

    // 3. PostGIS Query Execution
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

    const zValues: number[][] = result[0].z_values;
    const points: Coord[] = [];

    // 4. Data Extraction Loop (Top-Down / North -> South)
    // PostGIS 래스터 데이터는 일반적으로 북쪽(Top)이 Row 0입니다.
    // 이 순서를 그대로 유지하며, 패딩을 제외한 유효 타일 영역만 추출합니다.
    for (let row = pad; row < paddedHeight - pad; row++) {
      // worldY 계산: 북쪽(MaxY)에서 시작하여 아래로 감소
      const worldY = paddedMaxY - row * step;

      for (let col = pad; col < paddedWidth - pad; col++) {
        // worldX 계산: 서쪽(MinX)에서 시작하여 우측으로 증가
        const worldX = paddedMinX + col * step;
        const z = zValues[row]?.[col] ?? 0;

        // [Validation] 부동소수점 오차를 고려한 BBox 포함 여부 검사
        if (
          worldX >= bbox.minX - 0.1 &&
          worldX <= bbox.maxX + 0.1 &&
          worldY >= bbox.minY - 0.1 &&
          worldY <= bbox.maxY + 0.1
        ) {
          points.push({ x: worldX, y: worldY, z });
        }
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
