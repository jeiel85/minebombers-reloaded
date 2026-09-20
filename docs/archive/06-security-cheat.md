# 06. Security and Cheat Resistance

## Threat model

Public browser JavaScript is fully inspectable and modifiable. Therefore no client-side check is a security boundary.

Assume a malicious client can:

- fabricate protocol messages;
- send messages faster than UI permits;
- edit local cooldowns;
- forge coordinates/state if protocol allows it;
- reconnect repeatedly;
- choose arbitrary display names/payload sizes.

## Required server validations

For every message:

- schema validation;
- phase validation;
- player/session binding;
- monotonically increasing sequence validation;
- rate limiting;
- legal action validation;
- cooldown/inventory validation;
- payload size cap.

## Never trust from client

- player ID as authority;
- world position;
- hp;
- money;
- score;
- damage;
- item quantity;
- server time;
- cooldown completion;
- map seed;
- hit detection result.

## Origin restrictions

The Worker/DO should validate the WebSocket request's `Origin` against an allowlist, for example:

```text
https://<username>.github.io
https://game.example.com
http://localhost:5173        # development only
```

Origin checking is useful abuse reduction, not a substitute for server validation.

## Room codes

Use random codes with enough entropy. A 6-character code from a 32-symbol alphabet has roughly 30 bits of space; for a casual room system this is convenient but still enumerable at scale. Mitigate with:

- no room directory;
- rate-limit join attempts;
- optional room password/PIN later;
- generic error responses if room enumeration becomes a problem.

## Reconnect tokens

- generate server-side with cryptographically secure randomness;
- do not put in URLs;
- treat as bearer secrets;
- rotate after successful resume if practical;
- do not log raw tokens.

## XSS / UI

Display names and chat (if added) must be rendered as text, never injected as HTML.

For GitHub Pages, configure a CSP using a custom domain/proxy only if your hosting path allows the necessary response headers; otherwise keep DOM use strict and minimize third-party scripts.

## Abuse controls

Per socket token buckets:

```text
input     10/s normal, burst 20
button/action 8/s normal, burst 12
join/hello small fixed limit
oversized packet -> immediate close
persistent schema/rate violation -> close with policy code
```

## Logging

Production logs should capture:

- room code hash or safe identifier;
- match ID;
- server event sequence;
- player ID (non-secret ID);
- violation reason;
- latency/reconnect counts.

Do not log resume tokens or personal data unnecessarily.
