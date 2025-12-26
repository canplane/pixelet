// src/geo/entities/osm-poi.entity.ts
import { Entity, PrimaryColumn, Column, Index } from 'typeorm';
import { Geometry } from 'geojson';

@Entity({ name: 'osm_pois' })
export class OsmPoi {
  @PrimaryColumn({ type: 'bigint' })
  id!: string; // '!' 추가: 초기화 지연 허용

  @Column({ type: 'bigint' })
  osm_id!: string;

  @Column({ nullable: true })
  name?: string; // nullable이므로 '?' 또는 string | null 사용

  @Column({ name: 'poi_type', nullable: true })
  poiType?: string;

  @Index({ spatial: true })
  @Column({
    type: 'geometry',
    spatialFeatureType: 'Point',
    srid: 3857,
  })
  geom!: Geometry;
}
