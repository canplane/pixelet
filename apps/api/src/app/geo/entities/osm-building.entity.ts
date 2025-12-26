// src/geo/entities/osm-building.entity.ts
import { Entity, PrimaryColumn, Column, Index } from 'typeorm';
import { Geometry } from 'geojson';

@Entity({ name: 'osm_buildings' }) // 쉘 스크립트에서 생성한 테이블명과 일치
export class OsmBuilding {
  @PrimaryColumn({ type: 'bigint' })
  id!: string;

  @Column({ type: 'bigint' })
  osm_id!: string;

  @Column({ nullable: true })
  name?: string;

  @Column({ name: 'building_type', nullable: true })
  buildingType?: string;

  @Column({ type: 'int', default: 1 })
  levels!: number;

  @Index({ spatial: true })
  @Column({
    type: 'geometry',
    spatialFeatureType: 'Geometry',
    srid: 3857, // 우리 컨벤션인 EPSG:3857 유지
  })
  geom!: Geometry;
}
