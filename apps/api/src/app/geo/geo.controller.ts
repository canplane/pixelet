import { Controller, Post, Body } from '@nestjs/common';
import { GeoService } from './geo.service';
import { GeometryTileDto } from '@pixelet/shared/entities/osm/dto/tiles.dto';

/**
 * [TileRequestDto]
 * 요청 바디의 타입을 정의합니다.
 */
interface TileRequestDto {
  tx: number;
  ty: number;
}

@Controller('geo')
export class GeoController {
  constructor(private readonly geoService: GeoService) {}

  /**
   * [POST] /geo/tile
   * 특정 타일 인덱스를 바디로 받아 가공 완료된 GeometryTile을 반환합니다.
   * 페이지 이동 없이 Web Worker의 fetch에 의해 호출됩니다.
   */
  @Post('tile')
  async getTile(@Body() body: TileRequestDto): Promise<GeometryTileDto> {
    const { tx, ty } = body;
    // GeoService에서 베이킹까지 완료된 GeometryTile을 반환합니다.
    return await this.geoService.getProcessedTile({ tx, ty });
  }
}
