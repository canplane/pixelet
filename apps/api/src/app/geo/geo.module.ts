import { Module } from '@nestjs/common';
import { TypeOrmModule } from '@nestjs/typeorm';
import { GeoController } from './geo.controller';
import { GeoService } from './geo.service';
import { GeoRepository } from './geo.repository';
import { OsmAdapter } from './osm-adapter';
import { OsmElement } from './entities/osm-element.entity';

@Module({
  imports: [
    /**
     * [TypeOrmModule]
     * 'geoConnection' 데이터베이스 연결에 사용할 엔티티들을 등록합니다.
     */
    TypeOrmModule.forFeature([OsmElement], 'geoConnection'),
  ],
  providers: [GeoService, GeoRepository, OsmAdapter],
  controllers: [GeoController],
  exports: [GeoService],
})
export class GeoModule {}
