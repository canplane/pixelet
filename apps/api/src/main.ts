// import { bootstrapEngines } from './app/bootstrap';

import { NestFactory } from '@nestjs/core';
import {
  FastifyAdapter,
  NestFastifyApplication,
} from '@nestjs/platform-fastify';
import { ConfigService } from '@nestjs/config';
import { AppModule } from './app/app.module';

async function bootstrap() {
  // await bootstrapEngines();

  // Fastify 어댑터 생성 (기존 Fastify 설정과 호환)
  const app = await NestFactory.create<NestFastifyApplication>(
    AppModule,
    new FastifyAdapter({ logger: false }), // 개발 시 로그 확인 용이
    { logger: ['error', 'warn'] }, // 'error', 'warn'만 남기면 부팅 시 출력되는 파란색 LOG 메시지들이 사라집니다.
  );

  const configService = app.get(ConfigService); // ConfigService를 통해 .env 설정 로드
  const port = configService.get<number>('API_PORT') || 3000;
  const host = configService.get<string>('API_HOST') || '0.0.0.0';

  app.setGlobalPrefix('api'); // 전역 프리픽스 설정 (예: /api/v1)
  await app.listen(port, host);

  console.log(`🚀 pixelet API is running on: http://${host}:${port}/api`);
}

bootstrap();
