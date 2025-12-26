// src/geo/geo.module.ts
import { Module } from '@nestjs/common';
import { TypeOrmModule } from '@nestjs/typeorm';
import { OsmBuilding } from './entities/osm-building.entity';
import { OsmRoad } from './entities/osm-road.entity';
import { OsmPoi } from './entities/osm-poi.entity';
import { GeoService } from './geo.service';
import { GeoController } from './geo.controller';

@Module({
  imports: [
    // Geo DB 연결 이름('geoConnection')을 명시하여 엔티티 등록
    TypeOrmModule.forFeature([OsmBuilding, OsmRoad, OsmPoi], 'geoConnection'),
  ],
  providers: [GeoService],
  controllers: [GeoController],
  exports: [GeoService],
})
export class GeoModule {}
