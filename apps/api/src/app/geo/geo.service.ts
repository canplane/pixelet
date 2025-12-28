import { Injectable } from '@nestjs/common';
import { InjectRepository } from '@nestjs/typeorm';
import { Repository } from 'typeorm';
import { OsmElement } from '@/app/geo/entities/osm-element.entity';

import type { NormalizedElement } from '@pixelet/shared/entities/osm/api/normalized-types';
import { OSM_CONFIG } from '@pixelet/shared/entities/osm/osm-config';
import { getTileBbox } from '@pixelet/shared/entities/osm/lib/tile-math';
import {
  createEmptyGrids,
  TileGrids,
  WorldTile,
} from '@pixelet/shared/entities/osm/model/tiles';
import { processOsmToGrids } from '@pixelet/shared/entities/osm/lib/tile-data-handler';
import { MeterPos } from '@pixelet/shared/shared/model/coords';

// 🎯 [Refactoring] 공유 정책 모듈 Import
import {
  determineOsmCategory,
  shouldForceClosed,
  computeOsmMetrics,
} from '@pixelet/shared/entities/osm/lib/osm-policy';

const { TILE_SIZE, CELL_SIZE } = OSM_CONFIG;

interface OsmRawResult {
  osm_id: string;
  tags: string | Record<string, string>;
  wkt: string;
  cwkt: string;
}

type GeoJsonCoord = [number, number];

interface OsmGeometry {
  type: 'Point' | 'LineString' | 'Polygon' | 'MultiPolygon';
  coordinates: any;
}

@Injectable()
export class GeoService {
  constructor(
    @InjectRepository(OsmElement, 'geoConnection')
    private readonly geoRepo: Repository<OsmElement>,
  ) {}

  async getProcessedTile(tx: number, ty: number): Promise<WorldTile> {
    const resolution = Math.floor(TILE_SIZE / CELL_SIZE);
    const key = `${tx}:${ty}`;

    const elements = await this.getNearbyMapDataByTile(tx, ty);

    if (elements.length === 0) {
      console.warn(`[Tile-${key}] No elements found in this area.`);
    } else {
      console.log(`[Tile-${key}] Fetched total elements: ${elements.length}`);
    }

    const origin = { x: tx * TILE_SIZE, y: ty * TILE_SIZE, z: 0 };

    const { infras, pois, partialGrids } = processOsmToGrids(elements, origin, {
      tileSize: TILE_SIZE,
      cellSize: CELL_SIZE,
    });

    const finalGrids = createEmptyGrids(resolution);
    (Object.keys(partialGrids) as (keyof TileGrids)[]).forEach((channel) => {
      if (partialGrids[channel]) {
        finalGrids[channel].set(partialGrids[channel]);
      }
    });

    const gridsForJson: Record<string, number[]> = {};
    for (const [gKey, uint8Arr] of Object.entries(finalGrids)) {
      if (uint8Arr instanceof Uint8Array) {
        gridsForJson[gKey] = Array.from(uint8Arr);
      }
    }

    return {
      key,
      origin,
      grids: gridsForJson as unknown as TileGrids,
      infras,
      pois,
      resolution,
    };
  }

  private async getNearbyMapDataByTile(
    tx: number,
    ty: number,
  ): Promise<NormalizedElement[]> {
    const bbox = getTileBbox(tx, ty);
    const area3857 = `ST_MakeEnvelope(${bbox.minX}, ${bbox.minY}, ${bbox.maxX}, ${bbox.maxY}, 3857)`;

    const raws: any[] = await this.geoRepo.query(`
      SELECT 
        osm_id, 
        tags, 
        ST_AsGeoJSON(ST_SimplifyPreserveTopology(way, 0.1)) as geometry, 
        ST_AsText(ST_Centroid(way)) as cwkt
      FROM osm_elements
      WHERE way && ${area3857} 
        OR ST_Intersects(way, ${area3857})
    `);

    return raws.map((raw) => {
      const tags =
        typeof raw.tags === 'string'
          ? this.parseHstore(raw.tags)
          : raw.tags || {};
      const geo = JSON.parse(raw.geometry);

      // 🎯 [Refactoring] 분류 로직 위임 (Policy 모듈 사용)
      const category = determineOsmCategory(tags);

      return this.mapToNormalizedElementFromGeoJSON(raw, geo, tags, category);
    });
  }

  private mapToNormalizedElementFromGeoJSON(
    raw: OsmRawResult,
    geo: OsmGeometry,
    tags: Record<string, string>,
    category: string,
  ): NormalizedElement {
    let points: MeterPos[] = [];
    let holes: MeterPos[][] = [];

    // 1. Polygon 처리
    if (geo.type === 'Polygon') {
      const coords = geo.coordinates as GeoJsonCoord[][];
      points = coords[0].map(([x, y]: GeoJsonCoord) => ({ x, y, z: 0 }));
      if (coords.length > 1) {
        holes = coords
          .slice(1)
          .map((ring: GeoJsonCoord[]) =>
            ring.map(([x, y]: GeoJsonCoord) => ({ x, y, z: 0 })),
          );
      }
    }
    // 2. MultiPolygon 처리
    else if (geo.type === 'MultiPolygon') {
      const coords = geo.coordinates as GeoJsonCoord[][][];
      const sortedPolygons = coords.sort((a, b) => b[0].length - a[0].length);
      const mainPolygon = sortedPolygons[0];
      points = mainPolygon[0].map(([x, y]: GeoJsonCoord) => ({ x, y, z: 0 }));

      if (mainPolygon.length > 1) {
        holes = mainPolygon
          .slice(1)
          .map((ring: GeoJsonCoord[]) =>
            ring.map(([x, y]: GeoJsonCoord) => ({ x, y, z: 0 })),
          );
      }

      for (let i = 1; i < sortedPolygons.length; i++) {
        const poly = sortedPolygons[i];
        holes.push(poly[0].map(([x, y]: GeoJsonCoord) => ({ x, y, z: 0 })));
        if (poly.length > 1) {
          const subHoles = poly
            .slice(1)
            .map((ring: GeoJsonCoord[]) =>
              ring.map(([x, y]: GeoJsonCoord) => ({ x, y, z: 0 })),
            );
          holes.push(...subHoles);
        }
      }
    }
    // 3. LineString & Point 처리
    else if (geo.type === 'LineString') {
      const coords = geo.coordinates as GeoJsonCoord[];
      points = coords.map(([x, y]: GeoJsonCoord) => ({ x, y, z: 0 }));
    } else if (geo.type === 'Point') {
      const [x, y] = geo.coordinates as GeoJsonCoord;
      points = [{ x, y, z: 0 }];
    }

    const centerPoints = this.parseWktToMeterPos(raw.cwkt);
    const center = centerPoints[0] || points[0] || { x: 0, y: 0, z: 0 };

    // 🎯 [Refactoring] 강제 폐곡 판정 위임 + 기하학적 검사 결합
    const isClosed =
      shouldForceClosed(tags) ||
      (!['highway', 'waterway'].includes(tags.highway || tags.waterway || '') &&
        (geo.type === 'Polygon' ||
          geo.type === 'MultiPolygon' ||
          (points.length >= 3 &&
            Math.abs(points[0].x - points[points.length - 1].x) < 0.1)));

    // 🎯 [Refactoring] 물리 수치 계산 위임 (Policy 모듈 사용)
    const computed = computeOsmMetrics(tags, category);

    return {
      type: points.length === 1 ? 'node' : 'way',
      id: Number(raw.osm_id),
      center,
      shape: { points, isClosed, holes: holes.length > 0 ? holes : undefined },
      computed, // 계산된 객체 그대로 주입
      tags,
      category,
    };
  }

  private parseWktToMeterPos(wkt: string): MeterPos[] {
    if (!wkt) return [];
    const coordsMatch = wkt.match(/[\d\.-]+\s+[\d\.-]+/g);
    return coordsMatch
      ? coordsMatch.map((pair) => {
          const [x, y] = pair.split(/\s+/).map(Number);
          return { x, y, z: 0 };
        })
      : [];
  }

  private parseHstore(hstoreStr: string): Record<string, string> {
    const result: Record<string, string> = {};
    const pattern = /"(.+?)"=>"(.*?)"/g;
    let match;
    while ((match = pattern.exec(hstoreStr)) !== null) {
      result[match[1]] = match[2];
    }
    return result;
  }
}
