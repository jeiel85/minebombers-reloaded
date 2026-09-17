const http = require('http');
const fs = require('fs');
const path = require('path');
const { spawn } = require('child_process');

const PORT = 5195;
const DIST_DIR = path.resolve(__dirname, '../apps/client/dist');

const MIME_TYPES = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'application/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.png': 'image/png',
  '.wav': 'audio/wav',
  '.mp3': 'audio/mpeg',
  '.ico': 'image/x-icon',
  '.wasm': 'application/wasm',
  '.woff': 'font/woff',
  '.jsdos': 'application/octet-stream',
  '.zip': 'application/zip',
};

const server = http.createServer((req, res) => {
  let reqPath = decodeURIComponent(req.url.split('?')[0]);
  if (reqPath === '/' || reqPath === '') reqPath = '/index.html';

  const filePath = path.join(DIST_DIR, reqPath);

  // Prevent directory traversal
  if (!filePath.startsWith(DIST_DIR)) {
    res.writeHead(403);
    res.end('Forbidden');
    return;
  }

  fs.stat(filePath, (err, stats) => {
    if (err || !stats.isFile()) {
      // SPA Fallback to index.html
      const indexPath = path.join(DIST_DIR, 'index.html');
      fs.readFile(indexPath, (readErr, content) => {
        if (readErr) {
          res.writeHead(404);
          res.end('Not Found');
        } else {
          res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' });
          res.end(content);
        }
      });
      return;
    }

    const ext = path.extname(filePath).toLowerCase();
    const contentType = MIME_TYPES[ext] || 'application/octet-stream';

    res.writeHead(200, {
      'Content-Type': contentType,
      'Content-Length': stats.size,
      'Cache-Control': 'no-cache',
    });

    fs.createReadStream(filePath).pipe(res);
  });
});

server.listen(PORT, '127.0.0.1', () => {
  const isDos = process.argv.includes('--dos');
  const pathSuffix = isDos ? '/dos.html' : '';
  const url = `http://127.0.0.1:${PORT}${pathSuffix}`;
  console.log(`====================================================`);
  console.log(`  💣 MINE BOMBERS 3.11 (1995) - 100% Original DOS PC Edition   `);
  console.log(`====================================================`);
  console.log(`Local Engine Server running at: ${url}`);
  console.log(`Launching standalone Desktop Window...`);

  // Attempt to launch in App Mode (Edge or Chrome on Windows)
  const edgePaths = [
    'C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe',
    'C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe',
  ];
  const chromePaths = [
    'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe',
    'C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe',
  ];

  let browserPath = edgePaths.find((p) => fs.existsSync(p)) || chromePaths.find((p) => fs.existsSync(p));

  if (browserPath) {
    const child = spawn(
      browserPath,
      [
        `--app=${url}`,
        '--window-size=1008,775',
        '--app-id=minebombers_retro',
        '--disable-extensions',
      ],
      { detached: true, stdio: 'ignore' }
    );
    child.unref();
    console.log(`Desktop window launched successfully!`);
    console.log(`Press Ctrl+C to close the background launcher.`);
  } else {
    console.log(`Open ${url} in your browser to play!`);
  }
});
