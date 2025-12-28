// src/app.module.ts
import { Module } from '@nestjs/common';
import { ConfigModule, ConfigService } from '@nestjs/config';
import { TypeOrmModule } from '@nestjs/typeorm';
import { GeoModule } from './geo/geo.module';
import * as path from 'path';

@Module({
  imports: [
    // 1. 루트 .env 로드 (isGlobal 설정으로 모든 모듈에서 ConfigService 사용 가능)
    ConfigModule.forRoot({
      // 모노레포 구조에 따라 ../를 조절하여 프로젝트 루트의 .env를 지칭합니다.
      // 보통 apps/api/src/app 에서 루트는 4단계 위입니다.
      envFilePath: path.resolve(__dirname, '../../../.env'),
      isGlobal: true,
    }),

    // 2. Main DB 연결 (Business Logic)
    TypeOrmModule.forRootAsync({
      name: 'default', // 기본 연결은 'default' 또는 이름을 생략합니다.
      useFactory: (configService: ConfigService) => ({
        type: 'postgres',
        url: configService.get<string>('DATABASE_URL'),
        autoLoadEntities: true,
        synchronize: configService.get<string>('NODE_ENV') === 'development', // 개발 환경에서만 true
      }),
      inject: [ConfigService],
    }),

    // 3. Geo DB 연결 (OSM/GIS)
    TypeOrmModule.forRootAsync({
      name: 'geoConnection', // GIS 전용 연결 식별자
      useFactory: (configService: ConfigService) => ({
        type: 'postgres',
        host: configService.get<string>('GEO_DB_HOST'),
        port: configService.get<number>('GEO_DB_PORT'),
        username: configService.get<string>('GEO_DB_USER'),
        password: configService.get<string>('GEO_DB_PASSWORD'),
        database: configService.get<string>('GEO_DB_NAME'),
        autoLoadEntities: true, // GeoModule에서 등록된 엔티티 자동 스캔
        synchronize: false, // OSM 데이터는 쉘 스크립트로 관리하므로 false 권장
        logging: false,
      }),
      inject: [ConfigService],
    }),

    GeoModule,
  ],
})
export class AppModule {}
