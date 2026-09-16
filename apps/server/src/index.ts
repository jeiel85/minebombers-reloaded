import { GameRoom } from './room/GameRoom';

export { GameRoom };

export interface Env {
  GAME_ROOM: DurableObjectNamespace<GameRoom>;
  ALLOWED_ORIGIN: string;
}

function allowedOrigins(env: Env): string[] {
  return env.ALLOWED_ORIGIN.split(',').map((v) => v.trim()).filter(Boolean);
}

function isAllowedOrigin(request: Request, env: Env): boolean {
  const origin = request.headers.get('Origin');
  return origin === null || allowedOrigins(env).includes(origin);
}

function corsHeaders(request: Request, env: Env): HeadersInit {
  const origin = request.headers.get('Origin');
  const configured = allowedOrigins(env);
  const allowed = origin && configured.includes(origin) ? origin : (configured[0] ?? 'null');
  return {
    'access-control-allow-origin': allowed,
    'access-control-allow-methods': 'GET,POST,OPTIONS',
    'access-control-allow-headers': 'content-type',
    'vary': 'Origin',
  };
}

function json(request: Request, env: Env, body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: {
      'content-type': 'application/json; charset=utf-8',
      ...corsHeaders(request, env),
    },
  });
}

function randomRoomCode(length = 6): string {
  const alphabet = '23456789ABCDEFGHJKLMNPQRSTUVWXYZ';
  const bytes = crypto.getRandomValues(new Uint8Array(length));
  return Array.from(bytes, (b) => alphabet[b % alphabet.length]).join('');
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);

    if (request.method === 'OPTIONS') {
      if (!isAllowedOrigin(request, env)) return new Response('Origin not allowed', { status: 403 });
      return new Response(null, { status: 204, headers: corsHeaders(request, env) });
    }

    if (url.pathname === '/health') {
      return json(request, env, { ok: true, protocolVersion: 1 });
    }

    if (request.method === 'POST' && url.pathname === '/api/rooms') {
      if (!isAllowedOrigin(request, env)) return json(request, env, { error: 'ORIGIN_NOT_ALLOWED' }, 403);
      for (let attempt = 0; attempt < 5; attempt++) {
        const roomCode = randomRoomCode();
        const id = env.GAME_ROOM.idFromName(roomCode);
        const stub = env.GAME_ROOM.get(id);
        const initRequest = new Request(`https://room.internal/init?code=${roomCode}`, {
          method: 'POST',
          headers: { 'x-room-code': roomCode },
        });
        const result = await stub.fetch(initRequest);
        if (result.status === 409) continue;
        if (!result.ok) return result;
        return json(request, env, { roomCode, protocolVersion: 1 });
      }
      return json(request, env, { error: 'ROOM_CODE_EXHAUSTED' }, 503);
    }

    const match = url.pathname.match(/^\/api\/rooms\/([A-Z0-9]+)\/(ws|status)$/);
    if (!match) return json(request, env, { error: 'NOT_FOUND' }, 404);

    const roomCode = match[1];
    const action = match[2];
    if (!roomCode || !action) return json(request, env, { error: 'NOT_FOUND' }, 404);
    const id = env.GAME_ROOM.idFromName(roomCode);
    const stub = env.GAME_ROOM.get(id);

    const forwarded = new URL(request.url);
    forwarded.pathname = `/${action}`;
    forwarded.searchParams.set('roomCode', roomCode);

    const response = await stub.fetch(new Request(forwarded.toString(), request));

    // WebSocket upgrade responses are returned untouched.
    if (response.status === 101) return response;

    const headers = new Headers(response.headers);
    for (const [k, v] of Object.entries(corsHeaders(request, env))) headers.set(k, String(v));
    return new Response(response.body, { status: response.status, headers });
  },
} satisfies ExportedHandler<Env>;
