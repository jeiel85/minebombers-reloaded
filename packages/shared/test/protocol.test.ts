import { describe, expect, it } from 'vitest';
import { parseClientMessage } from '../src/protocol';
import { PROTOCOL_VERSION } from '../src/config';

describe('Protocol runtime message parsing & validation', () => {
  it('parses valid c.hello message', () => {
    const json = JSON.stringify({
      t: 'c.hello',
      v: PROTOCOL_VERSION,
      name: 'Player1',
      clientBuild: '1.0.0',
      resumeToken: null,
    });
    const msg = parseClientMessage(json);
    expect(msg.t).toBe('c.hello');
    if (msg.t === 'c.hello') {
      expect(msg.name).toBe('Player1');
    }
  });

  it('rejects version mismatch in c.hello', () => {
    const json = JSON.stringify({
      t: 'c.hello',
      v: 999,
      name: 'Player1',
      clientBuild: '1.0.0',
    });
    expect(() => parseClientMessage(json)).toThrow('INVALID_HELLO');
  });

  it('rejects invalid inputs (out of range dx/dy, negative seq)', () => {
    const invalidDx = JSON.stringify({
      t: 'c.input',
      seq: 1,
      clientTime: 100,
      dx: 5, // invalid
      dy: 0,
      primary: false,
      secondary: false,
      slot: 0,
    });
    expect(() => parseClientMessage(invalidDx)).toThrow('INVALID_INPUT');

    const negSeq = JSON.stringify({
      t: 'c.input',
      seq: -1, // invalid
      clientTime: 100,
      dx: 1,
      dy: 0,
      primary: false,
      secondary: false,
      slot: 0,
    });
    expect(() => parseClientMessage(negSeq)).toThrow('INVALID_INPUT');
  });

  it('parses and validates c.buy requests', () => {
    const valid = JSON.stringify({
      t: 'c.buy',
      seq: 5,
      equipmentId: 'small_charge',
      quantity: 2,
    });
    const msg = parseClientMessage(valid);
    expect(msg.t).toBe('c.buy');

    const invalidQty = JSON.stringify({
      t: 'c.buy',
      seq: 5,
      equipmentId: 'small_charge',
      quantity: 0, // must be >= 1
    });
    expect(() => parseClientMessage(invalidQty)).toThrow('INVALID_BUY');
  });
});
