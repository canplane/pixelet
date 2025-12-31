import { Entity, PrimaryColumn, Column, Index } from 'typeorm';
import { Geometry } from 'geojson';

// DB 엔티티 (영속성 계층)
@Entity('osm_elements')
export class OsmElement {
  @PrimaryColumn({ type: 'bigint' })
  osm_id!: string; // 🎯 '!' 추가: DB에서 반드시 가져올 값임을 명시

  @Column({ type: 'hstore', nullable: true })
  tags!: any; // 🎯 '!' 추가

  @Index({ spatial: true })
  @Column({
    type: 'geometry',
    spatialFeatureType: 'Geometry',
    srid: 3857,
    nullable: true,
  })
  way!: Geometry; // 🎯 '!' 추가
}
