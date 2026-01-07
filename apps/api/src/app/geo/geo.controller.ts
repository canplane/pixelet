import { Controller, Post, Body, BadRequestException, HttpCode, HttpStatus } from '@nestjs/common';
import { GeoService } from './geo.service';
import { TileDto } from '@pixelet/core/entities/geo/model/tiles';

/**
 * [TileRequestDto]
 * 요청 바디의 타입을 정의합니다. (Z는 전역 상수를 사용하므로 X, Y만 수신)
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
   * 타일 데이터를 생성하여 반환합니다.
   * 페이지 이동 없이 Web Worker의 fetch에 의해 호출됩니다.
   * (Get보다 Post가 캐싱 제어 및 복잡한 바디 전달에 유리할 수 있습니다.)
   */
  @Post('tile')
  @HttpCode(HttpStatus.OK) // POST 기본값인 201 대신 200 반환 (데이터 조회 성격)
  @Post('tile')
  async getTile(@Body() body: TileRequestDto): Promise<TileDto> {
    const { tx, ty } = body;

    // 🛡️ [Guard] 타일 인덱스 유효성 검사
    if (isNaN(tx) || isNaN(ty)) {
      throw new BadRequestException('Invalid tile coordinates (tx, ty must be numbers)');
    }

    try {
      // GeoService에서 베이킹까지 완료된 Tile을 반환합니다.
      return await this.geoService.createTile({ tx, ty });
    } catch (error) {
      // 서버 에러 로깅 (엔지니어링 필수 요소)
      console.error(`[GeoController] Failed to bake tile ${tx}:${ty} ->`, error);
      throw error; // NestJS 기본 인터셉터가 500 에러로 변환
    }
  }
}
