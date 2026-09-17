import Phaser from 'phaser';
import { BootScene } from './scenes/BootScene';
import { MenuScene } from './scenes/MenuScene';
import { LobbyScene } from './scenes/LobbyScene';
import { ShopScene } from './scenes/ShopScene';
import { GameScene } from './scenes/GameScene';
import { ResultScene } from './scenes/ResultScene';

const appDiv = document.querySelector<HTMLDivElement>('#app')!;
appDiv.innerHTML = `
  <div style="display:flex;flex-direction:column;align-items:center;justify-content:center;min-height:100vh;background-color:#0d0f12;color:#ffffff;font-family:'Courier New',Courier,monospace;margin:0;padding:12px;box-sizing:border-box;">
    <header style="margin-bottom:8px;text-align:center;">
      <h1 style="margin:0;font-size:24px;color:#f39c12;letter-spacing:2px;">⛏️ MINE BOMBERS WEB MULTIPLAYER 💣</h1>
      <p style="margin:4px 0 0 0;font-size:13px;color:#7f8c8d;">Phaser 4 • TypeScript • Cloudflare Workers & Durable Objects</p>
    </header>
    <div id="game-container" style="border:3px solid #34495e;border-radius:6px;overflow:hidden;box-shadow:0 8px 24px rgba(0,0,0,0.8);max-width:100%;max-height:85vh;aspect-ratio:992/736;">
      <div id="game" style="width:100%;height:100%;"></div>
    </div>
    <footer style="margin-top:8px;font-size:12px;color:#95a5a6;text-align:center;">
      <span>[WASD / Arrows] Move & Dig • [SPACE] Action • [1..8] Hotbar • [Q/E] Cycle Weapon • [F / Right-Click] Detonate / Utility • [F3] Debug</span>
    </footer>
  </div>
`;

new Phaser.Game({
  type: Phaser.AUTO,
  width: 992,
  height: 736,
  parent: 'game',
  backgroundColor: '#111111',
  pixelArt: true,
  scale: {
    mode: Phaser.Scale.FIT,
    autoCenter: Phaser.Scale.CENTER_BOTH,
  },
  scene: [BootScene, MenuScene, LobbyScene, ShopScene, GameScene, ResultScene],
});
