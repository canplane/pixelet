// src/geo/entities/osm-road.entity.ts
import { Entity, PrimaryColumn, Column, Index } from 'typeorm';
import { Geometry } from 'geojson';
// import { IOsmRoad } from '@pixelet/shared'; // 예시: shared 타입 참조

@Entity({ name: 'osm_roads' })
export class OsmRoad {
  @PrimaryColumn({ type: 'bigint' })
  id!: string;

  @Column({ type: 'bigint' })
  osm_id!: string;

  @Column({ nullable: true })
  name?: string;

  @Column({ name: 'highway_type', nullable: true })
  highwayType?: string;

  @Column({ type: 'boolean', default: false })
  oneway!: boolean;

  @Index({ spatial: true })
  @Column({
    type: 'geometry',
    spatialFeatureType: 'LineString', // 도로는 주로 LineString
    srid: 3857,
  })
  geom!: Geometry;
}
