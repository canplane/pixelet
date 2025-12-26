import {
  Controller,
  Get,
  Post,
  Body,
  Query,
  ParseFloatPipe,
} from '@nestjs/common';
import { GeoService } from './geo.service';
import type { OverpassResponse } from '@pixelet/shared/entities/osm/api/overpass-types';

@Controller('geo')
export class GeoController {
  constructor(private readonly geoService: GeoService) {}

  /**
   * [GET] 단순 위경도/반경 테스트용 API
   */
  @Get('nearby')
  async getNearby(
    @Query('lat', ParseFloatPipe) lat: number,
    @Query('lng', ParseFloatPipe) lng: number,
    @Query('radius', ParseFloatPipe) radius: number,
  ) {
    // Shared의 parseOsmData 결과를 그대로 반환 { infras, pois }
    return await this.geoService.getNearbyMapData(lat, lng, radius);
  }

  /**
   * [POST] 기존 클라이언트(requestOverpass) 호환용 API
   * Overpass QL 쿼리 문자열에서 BBox를 추출하여 공간 검색을 수행합니다.
   */
  @Post('query')
  async handleOverpassQuery(
    @Body('data') query: string, // 'data=' 형식으로 들어오는 쿼리 본문
  ): Promise<OverpassResponse> {
    // 1. 쿼리 문자열에서 BBox 파싱 (예: [37.5, 127.0, 37.6, 127.1])
    const { lat, lng, radius } = this.parseCoordsFromQuery(query);

    // 2. 파싱된 좌표를 바탕으로 기존 서비스 로직 실행
    return await this.geoService.getNearbyMapData(lat, lng, radius);
  }

  /**
   * Overpass QL의 BBox (s, w, n, e) 구문을 해석하여
   * 서비스가 이해할 수 있는 중심점과 반경으로 변환합니다.
   */
  private parseCoordsFromQuery(query: string) {
    // 정규표현식으로 (south, west, north, east) 패턴 추출
    const bboxMatch = query.match(
      /\(([-.\d]+),([-.\d]+),([-.\d]+),([-.\d]+)\)/,
    );

    if (bboxMatch) {
      const [_, s, w, n, e] = bboxMatch.map(Number);

      // 중심점 계산
      const lat = (s + n) / 2;
      const lng = (w + e) / 2;

      // 반경 계산 (간단하게 위도 차이 기반 미터 계산 혹은 고정값 적용)
      // 여기서는 500m를 기본으로 하되, BBox 크기에 따라 유동적으로 설정 가능합니다.
      const radius = 500;

      return { lat, lng, radius };
    }

    // 파싱 실패 시 기본 좌표 (예: 서울 시청 혹은 설정값)
    return { lat: 37.5665, lng: 126.978, radius: 500 };
  }
}
