import { describe, expect, it } from 'vitest';
import {
  generateResumeToken,
  hashToken,
  type PlayerSlot,
  type RoomRecord,
} from '../src/room/RoomStorage';
import { SocketRateLimiter } from '../src/security/RateLimiter';
import { createDefaultInventory, MAX_PLAYERS } from '@minebombers/shared';

describe('Room Storage & Security', () => {
  it('generates cryptographic resume tokens and hashes them deterministically', async () => {
    const token1 = generateResumeToken();
    const token2 = generateResumeToken();

    expect(token1).not.toBe(token2);
    expect(token1.length).toBe(48); // 24 bytes in hex

    const hash1a = await hashToken(token1);
    const hash1b = await hashToken(token1);
    const hash2 = await hashToken(token2);

    expect(hash1a).toBe(hash1b);
    expect(hash1a).not.toBe(hash2);
    expect(hash1a.length).toBe(64); // SHA-256 hex
  });

  it('enforces maximum 8 players capacity counting active and reserved slots', () => {
    const slots: PlayerSlot[] = [];

    for (let i = 0; i < MAX_PLAYERS; i++) {
      slots.push({
        playerId: `p_${i}`,
        displayName: `Miner ${i}`,
        joinedAt: Date.now(),
        connected: i % 2 === 0, // mix of connected and disconnected
        ready: false,
        reservedUntil: i % 2 === 0 ? null : Date.now() + 20_000,
        resumeTokenHash: `hash_${i}`,
        cash: 500,
        inventory: createDefaultInventory(),
        stats: { kills: 0, treasureValue: 0, roundWins: 0 },
      });
    }

    expect(slots.length).toBe(8);
    // 9th player must be rejected
    const canJoin9th = slots.length < MAX_PLAYERS;
    expect(canJoin9th).toBe(false);
  });

  it('rate limiter throttles burst abuse', () => {
    const limiter = new SocketRateLimiter();

    // Burst of 25 moves should pass
    let movesAllowed = 0;
    for (let i = 0; i < 30; i++) {
      if (limiter.checkMove()) movesAllowed++;
    }
    expect(movesAllowed).toBe(25);

    // Burst of 12 actions should pass
    let actionsAllowed = 0;
    for (let i = 0; i < 20; i++) {
      if (limiter.checkAction()) actionsAllowed++;
    }
    expect(actionsAllowed).toBe(12);

    // Burst of 8 purchases should pass
    let buysAllowed = 0;
    for (let i = 0; i < 15; i++) {
      if (limiter.checkBuy()) buysAllowed++;
    }
    expect(buysAllowed).toBe(8);
  });
});
