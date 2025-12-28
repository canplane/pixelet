import { Module } from '@nestjs/common';
import { TypeOrmModule } from '@nestjs/typeorm';
import { OsmElement } from './entities/osm-element.entity';
import { GeoService } from './geo.service';
import { GeoController } from './geo.controller';

@Module({
  imports: [
    /**
     * [TypeOrmModule]
     * 'geoConnection' 데이터베이스 연결에 사용할 엔티티들을 등록합니다.
     * 4-Pass 레이어 전략에 맞춰 Base, Transport, Structure 엔티티가 모두 포함되어야 합니다.
     */
    TypeOrmModule.forFeature([OsmElement], 'geoConnection'),
  ],
  providers: [GeoService],
  controllers: [GeoController],
  exports: [GeoService],
})
export class GeoModule {}
