/**
 * Mine Bombers P2P Netcode Manager
 * - WebRTC DataChannel for low-latency peer-to-peer input transmission
 * - 6-character Room Codes via PeerJS cloud broker (zero server requirement)
 * - BroadcastChannel for zero-config LAN and cross-tab room discovery
 * - 60 FPS deterministic input lockstep synchronization
 */

export class NetplayManager {
  constructor() {
    this.peer = null;
    this.connections = new Map(); // peerId -> DataConnection
    this.playerSlots = [null, null, null, null]; // peerId or 'HOST' or null
    this.mySlot = 0; // 0 for Host, 1..3 for Guests
    this.isHost = false;
    this.roomCode = null;
    this.state = 'DISCONNECTED'; // 'DISCONNECTED' | 'HOSTING' | 'JOINING' | 'CONNECTED' | 'IN_GAME'
    this.pingMs = 0;

    // Frame synchronization
    this.currentFrame = 0;
    this.remoteInputs = new Map(); // frame -> [p1, p2, p3, p4]

    // LAN BroadcastChannel
    this.lanChannel = null;
    this.lanRooms = new Map(); // roomCode -> { hostName, mode, players, timestamp }
    this.lanBroadcastTimer = null;

    // Callbacks
    this.onStatusChange = null;
    this.onPlayerUpdate = null;
    this.onGameStart = null;
    this.onFrameSync = null;
    this.onLanRoomsUpdate = null;

    this.initLanChannel();
  }

  // ---------------------------------------------------------------------------
  // LAN / Local Broadcast Discovery
  // ---------------------------------------------------------------------------

  initLanChannel() {
    try {
      if (typeof BroadcastChannel !== 'undefined') {
        this.lanChannel = new BroadcastChannel('minebombers_lan_lobby');
        this.lanChannel.onmessage = (e) => this.handleLanMessage(e.data);
      }
    } catch (err) {
      console.warn('BroadcastChannel not supported:', err);
    }
  }

  handleLanMessage(data) {
    if (!data || !data.type) return;

    if (data.type === 'ANNOUNCE_ROOM') {
      this.lanRooms.set(data.roomCode, {
        roomCode: data.roomCode,
        hostName: data.hostName || 'Player 1',
        mode: data.mode || 'Survival Horde',
        players: data.players || 1,
        maxPlayers: 4,
        timestamp: Date.now(),
      });
      this.cleanExpiredLanRooms();
      if (this.onLanRoomsUpdate) this.onLanRoomsUpdate(Array.from(this.lanRooms.values()));
    } else if (data.type === 'CLOSE_ROOM') {
      this.lanRooms.delete(data.roomCode);
      if (this.onLanRoomsUpdate) this.onLanRoomsUpdate(Array.from(this.lanRooms.values()));
    }
  }

  cleanExpiredLanRooms() {
    const now = Date.now();
    for (const [code, info] of this.lanRooms.entries()) {
      if (now - info.timestamp > 6000) {
        this.lanRooms.delete(code);
      }
    }
  }

  startLanAnnouncements(modeName) {
    if (!this.lanChannel) return;
    this.stopLanAnnouncements();
    this.lanBroadcastTimer = setInterval(() => {
      if (!this.isHost || !this.roomCode) return;
      const count = this.getConnectedPlayerCount();
      this.lanChannel.postMessage({
        type: 'ANNOUNCE_ROOM',
        roomCode: this.roomCode,
        hostName: 'Host (P1)',
        mode: modeName || 'Survival Horde',
        players: count,
      });
    }, 1500);
  }

  stopLanAnnouncements() {
    if (this.lanBroadcastTimer) {
      clearInterval(this.lanBroadcastTimer);
      this.lanBroadcastTimer = null;
    }
    if (this.lanChannel && this.roomCode) {
      try {
        this.lanChannel.postMessage({ type: 'CLOSE_ROOM', roomCode: this.roomCode });
      } catch (_) {}
    }
  }

  // ---------------------------------------------------------------------------
  // Room Code Generation & PeerJS Setup
  // ---------------------------------------------------------------------------

  generateRoomCode() {
    const chars = 'ABCDEFGHJKLMNPQRSTUVWXYZ23456789';
    let code = '';
    for (let i = 0; i < 6; i++) {
      code += chars.charAt(Math.floor(Math.random() * chars.length));
    }
    return code;
  }

  getPeerIdForRoom(roomCode) {
    return 'mb-' + roomCode.toUpperCase();
  }

  async hostRoom(modeName = 'Survival Horde') {
    this.disconnect();
    this.isHost = true;
    this.mySlot = 0;
    this.playerSlots = ['HOST', null, null, null];
    this.roomCode = this.generateRoomCode();
    this.state = 'HOSTING';
    this.updateStatus('Creating Room [' + this.roomCode + ']...');

    if (typeof Peer === 'undefined') {
      this.updateStatus('PeerJS not loaded. Local LAN mode active.');
      this.startLanAnnouncements(modeName);
      return this.roomCode;
    }

    return new Promise((resolve, reject) => {
      const peerId = this.getPeerIdForRoom(this.roomCode);
      this.peer = new Peer(peerId, {
        debug: 1,
        config: {
          iceServers: [
            { urls: 'stun:stun.l.google.com:19302' },
            { urls: 'stun:stun1.l.google.com:19302' },
          ],
        },
      });

      this.peer.on('open', (id) => {
        console.log('[Netplay] Peer opened: ' + id);
        this.updateStatus('Hosting Room: ' + this.roomCode);
        this.startLanAnnouncements(modeName);
        if (this.onPlayerUpdate) this.onPlayerUpdate(this.playerSlots, this.mySlot);
        resolve(this.roomCode);
      });

      this.peer.on('connection', (conn) => {
        this.handleIncomingConnection(conn);
      });

      this.peer.on('error', (err) => {
        console.error('[Netplay] PeerJS error:', err);
        if (err.type === 'unavailable-id') {
          this.hostRoom(modeName).then(resolve).catch(reject);
        } else {
          this.updateStatus('Connection Error: ' + err.type);
          reject(err);
        }
      });
    });
  }

  handleIncomingConnection(conn) {
    let freeSlot = -1;
    for (let i = 1; i < 4; i++) {
      if (this.playerSlots[i] === null) {
        freeSlot = i;
        break;
      }
    }

    if (freeSlot === -1) {
      conn.on('open', () => {
        conn.send({ type: 'ERROR', message: 'Room is full (max 4 players)' });
        conn.close();
      });
      return;
    }

    this.playerSlots[freeSlot] = conn.peer;
    this.connections.set(conn.peer, conn);

    conn.on('open', () => {
      console.log('[Netplay] Guest connected at slot ' + (freeSlot + 1));
      conn.send({
        type: 'WELCOME',
        slot: freeSlot,
        roomCode: this.roomCode,
        slots: this.playerSlots,
      });

      this.broadcastToGuests({
        type: 'PLAYER_JOINED',
        slot: freeSlot,
        slots: this.playerSlots,
      }, conn.peer);

      if (this.onPlayerUpdate) this.onPlayerUpdate(this.playerSlots, this.mySlot);
    });

    conn.on('data', (data) => {
      this.handlePacket(data, freeSlot, conn);
    });

    conn.on('close', () => {
      console.log('[Netplay] Guest disconnected from slot ' + (freeSlot + 1));
      this.playerSlots[freeSlot] = null;
      this.connections.delete(conn.peer);
      this.broadcastToGuests({
        type: 'PLAYER_LEFT',
        slot: freeSlot,
        slots: this.playerSlots,
      });
      if (this.onPlayerUpdate) this.onPlayerUpdate(this.playerSlots, this.mySlot);
    });
  }

  async joinRoom(roomCode) {
    this.disconnect();
    this.isHost = false;
    this.roomCode = roomCode.trim().toUpperCase();
    this.state = 'JOINING';
    this.updateStatus('Connecting to [' + this.roomCode + ']...');

    if (typeof Peer === 'undefined') {
      throw new Error('PeerJS library not available.');
    }

    return new Promise((resolve, reject) => {
      this.peer = new Peer({
        debug: 1,
        config: {
          iceServers: [
            { urls: 'stun:stun.l.google.com:19302' },
            { urls: 'stun:stun1.l.google.com:19302' },
          ],
        },
      });

      this.peer.on('open', () => {
        const hostPeerId = this.getPeerIdForRoom(this.roomCode);
        console.log('[Netplay] Connecting to host ' + hostPeerId + '...');

        const conn = this.peer.connect(hostPeerId, {
          reliable: true,
          serialization: 'json',
        });

        conn.on('open', () => {
          console.log('[Netplay] Connected to host DataChannel!');
          this.connections.set(hostPeerId, conn);
          this.state = 'CONNECTED';
          this.updateStatus('Connected to Room ' + this.roomCode);
          this.startPingLoop(conn);
          resolve(conn);
        });

        conn.on('data', (data) => {
          this.handlePacket(data, 0, conn);
        });

        conn.on('close', () => {
          console.log('[Netplay] Connection closed.');
          this.disconnect();
          this.updateStatus('Host closed the room.');
        });

        conn.on('error', (err) => {
          console.error('[Netplay] DataChannel error:', err);
          this.updateStatus('DataChannel error: ' + err);
          reject(err);
        });
      });

      this.peer.on('error', (err) => {
        console.error('[Netplay] Peer error:', err);
        this.updateStatus('Failed to connect: ' + err.type);
        reject(err);
      });
    });
  }

  handlePacket(data, senderSlot, conn) {
    if (!data || !data.type) return;

    switch (data.type) {
      case 'WELCOME': {
        this.mySlot = data.slot;
        this.playerSlots = data.slots;
        this.updateStatus('Joined as Player ' + (this.mySlot + 1));
        if (this.onPlayerUpdate) this.onPlayerUpdate(this.playerSlots, this.mySlot);
        break;
      }
      case 'PLAYER_JOINED':
      case 'PLAYER_LEFT': {
        this.playerSlots = data.slots;
        if (this.onPlayerUpdate) this.onPlayerUpdate(this.playerSlots, this.mySlot);
        break;
      }
      case 'GAME_START': {
        this.state = 'IN_GAME';
        this.currentFrame = 0;
        this.remoteInputs.clear();
        console.log('[Netplay] Match start: seed=' + data.seed + ', mode=' + data.mode);
        if (this.onGameStart) {
          this.onGameStart(data);
        }
        break;
      }
      case 'INPUT': {
        if (this.isHost) {
          const frame = data.frame;
          if (!this.remoteInputs.has(frame)) {
            this.remoteInputs.set(frame, [0, 0, 0, 0]);
          }
          const inputs = this.remoteInputs.get(frame);
          inputs[data.slot] = data.mask;
        }
        break;
      }
      case 'FRAME_SYNC': {
        if (!this.isHost) {
          if (this.onFrameSync) {
            this.onFrameSync(data.frame, data.inputs);
          }
        }
        break;
      }
      case 'PING': {
        conn.send({ type: 'PONG', t: data.t });
        break;
      }
      case 'PONG': {
        const rtt = Math.round(performance.now() - data.t);
        this.pingMs = rtt;
        break;
      }
      case 'ERROR': {
        alert('Netplay Notice: ' + data.message);
        this.disconnect();
        break;
      }
    }
  }

  startPingLoop(conn) {
    setInterval(() => {
      if (this.state === 'CONNECTED' || this.state === 'IN_GAME') {
        try {
          conn.send({ type: 'PING', t: performance.now() });
        } catch (_) {}
      }
    }, 2000);
  }

  startMatch(seed, mode, biome) {
    if (!this.isHost) return;
    this.state = 'IN_GAME';
    this.currentFrame = 0;
    this.remoteInputs.clear();

    const startPacket = {
      type: 'GAME_START',
      seed: seed || Math.floor(Math.random() * 1000000),
      mode: mode || 'Survival Horde',
      biome: biome || 0,
      slots: this.playerSlots,
    };

    this.broadcastToGuests(startPacket);
    if (this.onGameStart) {
      this.onGameStart(startPacket);
    }
  }

  sendLocalInput(frame, mask) {
    if (this.state !== 'IN_GAME') return;

    if (this.isHost) {
      if (!this.remoteInputs.has(frame)) {
        this.remoteInputs.set(frame, [0, 0, 0, 0]);
      }
      const inputs = this.remoteInputs.get(frame);
      inputs[0] = mask;
    } else {
      for (const conn of this.connections.values()) {
        try {
          conn.send({
            type: 'INPUT',
            slot: this.mySlot,
            frame,
            mask,
          });
        } catch (_) {}
      }
    }
  }

  broadcastFrameInputs(frame, inputs) {
    if (!this.isHost || this.state !== 'IN_GAME') return;
    const packet = {
      type: 'FRAME_SYNC',
      frame,
      inputs,
    };
    this.broadcastToGuests(packet);
  }

  broadcastToGuests(packet, excludePeer = null) {
    for (const [peerId, conn] of this.connections.entries()) {
      if (peerId !== excludePeer && conn.open) {
        try {
          conn.send(packet);
        } catch (err) {
          console.warn('[Netplay] Send error:', err);
        }
      }
    }
  }

  getConnectedPlayerCount() {
    return this.playerSlots.filter(s => s !== null).length;
  }

  updateStatus(msg) {
    console.log('[Netplay Status] ' + msg);
    if (this.onStatusChange) {
      this.onStatusChange(msg, this.state);
    }
  }

  disconnect() {
    this.stopLanAnnouncements();
    for (const conn of this.connections.values()) {
      try { conn.close(); } catch (_) {}
    }
    this.connections.clear();
    if (this.peer) {
      try { this.peer.destroy(); } catch (_) {}
      this.peer = null;
    }
    this.playerSlots = [null, null, null, null];
    this.roomCode = null;
    this.isHost = false;
    this.mySlot = 0;
    this.state = 'DISCONNECTED';
    this.updateStatus('Disconnected');
    if (this.onPlayerUpdate) this.onPlayerUpdate(this.playerSlots, this.mySlot);
  }
}
