/**
 * @file geo.repository.ts
 * PostGIS 쿼리 실행 및 Raw 데이터 Fetch를 담당하는 저장소 계층입니다.
 */

import { Injectable } from '@nestjs/common';
import { InjectRepository } from '@nestjs/typeorm';
import { Repository } from 'typeorm';
import { OsmElement } from '@/app/geo/entities/osm-element.entity';
import { GEO_CONSTANTS } from '@pixelet/core/entities/geo/model/geo-constants';
import type { Coord } from '@pixelet/core/shared/model/coords';
import type { Bbox } from '@pixelet/core/entities/geo/lib/utils/tile-utils';

const { ELEVATION_SAMPLE_STEP } = GEO_CONSTANTS;

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
   * @strategy Reference Raster + Union + Padding + South-to-North Order
   * 1. Padding: 타일 경계(Seam)의 보간 오차를 방지하기 위해 상하좌우 1칸(step)씩 더 넓게 조회합니다.
   * 2. Reference Raster: 목표 규격(크기, 원점, 스케일)을 가진 빈 래스터를 먼저 정의하여 반 픽셀 오차를 원천 차단합니다.
   * 3. Union: 여러 SRTM/ESA 파일에 걸쳐 있는 타일의 경우, 조각난 래스터를 병합(ST_Union)합니다.
   * 4. 🎯 South-to-North Order (Critical):
   * PostGIS의 Top-Down 배열을 역순으로 순회하여 월드 좌표계(South-to-North)와 일치시킵니다.
   * 이로써 별도의 Y-Flip 없이 Three.js의 UV 좌표계(0,0 = South-West)와 직접 정합됩니다.
   */
  async fetchElevationPoints(
    bbox: Bbox,
    step: number = ELEVATION_SAMPLE_STEP,
  ): Promise<Coord[]> {
    const widthMeters = bbox.maxX - bbox.minX;
    const heightMeters = bbox.maxY - bbox.minY;

    // 1. Grid Size Calculation
    const innerWidth = Math.round(widthMeters / step) + 1;
    const innerHeight = Math.round(heightMeters / step) + 1;
    const paddedWidth = innerWidth + PAD * 2;
    const paddedHeight = innerHeight + PAD * 2;

    // 2. Logical Coordinates
    const paddedMinX = bbox.minX - step * PAD;
    const paddedMaxY = bbox.maxY + step * PAD;

    // 3. Half-Pixel Correction Coordinates
    // 픽셀의 중심(Center)이 정확히 Vertex(교차점)에 오도록 원점을 반 픽셀(Half-Step) 이동합니다.
    // 이는 타일 간 경계선 높이를 비트 단위로 일치시키는 핵심 로직입니다.
    const halfStep = step / 2;
    const resampleMinX = paddedMinX - halfStep;
    const resampleMaxY = paddedMaxY + halfStep;

    // 4. Data Fetching Buffer (for interpolation)
    // 원본 데이터는 타일보다 조금 더 넓게 가져와야 경계면 Bilinear 보간이 깨지지 않습니다.
    const bufferDistance = step * 2;

    /**
     * 🚀 [Reference Raster Strategy]
     * 복잡한 ST_Resample 인자 대신, '목표 규격'을 가진 빈 래스터(ref_rast)를 정의하고
     * 거기에 데이터를 끼워 맞추는 방식입니다. 가장 안전하고 정확합니다.
     */
    const query = `
      WITH 
      -- 1. 목표 타일 규격 정의 (Reference Raster)
      -- 우리가 원하는 완벽한 격자(크기, 원점, 스케일)를 미리 정의합니다.
      ref_rast AS (
        SELECT ST_MakeEmptyRaster(
          ${paddedWidth}::integer, ${paddedHeight}::integer,
          ${resampleMinX}::float8, ${resampleMaxY}::float8,
          ${step}::float8, ${-step}::float8,
          0::float8, 0::float8,
          3857
        ) as rast
      ),
      -- 2. 데이터 조회 영역 설정 (목표 영역 + 버퍼)
      -- 보간(Interpolation) 시 경계면이 깨지지 않도록 버퍼를 포함해 넓게 잡습니다.
      fetch_area AS (
        SELECT ST_Expand(ST_Envelope(rast), ${bufferDistance}) as geom
        FROM ref_rast
      ),
      -- 3. 소스 데이터 잘라내기 (인덱스 활용 & 메모리 최적화)
      -- 거대한 원본 래스터에서 필요한 조각만 잘라내어 메모리 사용량을 최소화합니다.
      -- ST_Transform(geom, ST_SRID(r.rast))를 통해 인덱스를 강제로 태웁니다.
      source_clips AS (
        SELECT 
          ST_Transform(
            ST_Clip(
              r.rast, 
              ST_Transform(f.geom, ST_SRID(r.rast))
            ), 
            3857
          ) as rast
        FROM esa_elevation_raster r, fetch_area f
        WHERE ST_Intersects(r.rast, ST_Transform(f.geom, ST_SRID(r.rast)))
      ),
      -- 4. 조각 병합
      -- 여러 파일에 걸쳐 있는 경우 하나로 합칩니다.
      unioned_rast AS (
        SELECT ST_Union(rast) as rast
        FROM source_clips
      ),
      -- 5. 리샘플링 (Reference Raster에 정렬)
      -- Reference Raster를 기준(Scale, Grid)으로 데이터를 끼워 맞춥니다(Snap).
      resampled_rast AS (
        SELECT ST_Resample(
          u.rast,
          r.rast,   -- 🎯 Reference Raster 기준
          'Bilinear'
        ) as rast
        FROM unioned_rast u, ref_rast r
      ),
      -- 6. 최종 트리밍 (정확한 픽셀 크기로 확정)
      -- 혹시 모를 오차를 제거하고 정확한 19x19(예시) 크기로 자릅니다.
      final_rast AS (
        SELECT ST_Clip(s.rast, ST_Envelope(r.rast)) as rast
        FROM resampled_rast s, ref_rast r
      )
      SELECT (ST_DumpValues(rast)).valarray as z_values
      FROM final_rast;
    `;

    // 🛡️ 타임아웃 5초 설정 (안전장치)
    await this.repo.query('SET statement_timeout = 5000');

    const result = await this.repo.query(query);

    if (!result || result.length === 0 || !result[0]?.z_values) {
      const points: Coord[] = [];
      for (let row = 0; row < paddedHeight; row++) {
        const worldY = paddedMaxY - row * step;
        for (let col = 0; col < paddedWidth; col++) {
          const worldX = paddedMinX + col * step;
          points.push({ x: worldX, y: worldY, z: 0 });
        }
      }
      return points;
    }

    const zValues: number[][] = result[0].z_values;
    const points: Coord[] = [];

    // South-to-North Ordering
    // PostGIS는 Top-Down(North->South)으로 데이터를 주므로,
    // 아래쪽(남쪽) Row부터 읽어서 배열에 넣으면 0번 인덱스가 남쪽(South-West)이 됩니다.
    for (let row = paddedHeight - 1; row >= 0; row--) {
      const worldY = paddedMaxY - row * step;
      for (let col = 0; col < paddedWidth; col++) {
        const worldX = paddedMinX + col * step;
        // zValues가 null일 경우 안전하게 0 처리
        const z = zValues[row]?.[col] ?? 0;
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
      SELECT osm_id, tags, ST_AsGeoJSON(ST_SimplifyPreserveTopology(way, 0.1)) as geometry, ST_AsText(ST_Centroid(way)) as cwkt
      FROM osm_elements WHERE way && ${area3857} OR ST_Intersects(way, ${area3857})
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
      SELECT fid as id, ST_AsGeoJSON(geom) as geometry FROM osm_coastline_polygons WHERE geom && ${area3857} AND ST_IsValid(geom)
    `);
  }
}
