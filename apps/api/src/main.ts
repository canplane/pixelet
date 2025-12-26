import { NestFactory } from '@nestjs/core';
import {
  FastifyAdapter,
  NestFastifyApplication,
} from '@nestjs/platform-fastify';
import { ConfigService } from '@nestjs/config';
import { AppModule } from './app/app.module';

async function bootstrap() {
  // Fastify 어댑터 생성 (기존 Fastify 설정과 호환)
  const app = await NestFactory.create<NestFastifyApplication>(
    AppModule,
    new FastifyAdapter({ logger: true }), // 개발 시 로그 확인 용이
  );

  // ConfigService를 통해 .env 설정 로드
  const configService = app.get(ConfigService);
  const port = configService.get<number>('API_PORT') || 3000;
  const host = configService.get<string>('API_HOST') || '0.0.0.0';

  // 전역 프리픽스 설정 (예: /api/v1)
  app.setGlobalPrefix('api');

  await app.listen(port, host);
  console.log(`🚀 Pixelet API is running on: http://localhost:${port}/api`);
}

bootstrap();
