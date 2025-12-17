import Fastify from 'fastify';

const app = Fastify();

app.get('/health', async () => {
  return { ok: true };
});

app.listen({ port: 3000 }, () => {
  console.log('API running at http://localhost:3000');
});
