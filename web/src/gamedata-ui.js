// The "give me your Mine Bombers 3.11 files" dialog (markup in index.html).
import { collectFromFiles, describeMissing } from './gamedata.js';

function readEntries(directory) {
  return new Promise((resolve, reject) => {
    const reader = directory.createReader();
    const all = [];
    const next = () =>
      reader.readEntries((batch) => {
        if (batch.length === 0) resolve(all);
        else {
          all.push(...batch);
          next();
        }
      }, reject);
    next();
  });
}

async function filesFromEntry(entry) {
  if (entry.isFile) {
    return [await new Promise((resolve, reject) => entry.file(resolve, reject))];
  }
  const files = [];
  for (const child of await readEntries(entry)) {
    files.push(...(await filesFromEntry(child)));
  }
  return files;
}

// Files, folders and ZIPs can all be dropped. The entries must be taken from the event synchronously.
async function filesFromDrop(dataTransfer) {
  const entries = [...dataTransfer.items].map((item) => item.webkitGetAsEntry && item.webkitGetAsEntry()).filter(Boolean);
  if (entries.length === 0) return [...dataTransfer.files];
  const files = [];
  for (const entry of entries) {
    files.push(...(await filesFromEntry(entry)));
  }
  return files;
}

/**
 * Shows the dialog until the user provides a complete copy.
 * @returns {Promise<Map<string, Uint8Array>>}
 */
export function promptForGameFiles() {
  const overlay = document.getElementById('gamedata-prompt');
  const drop = document.getElementById('gd-drop');
  const folder = document.getElementById('gd-folder');
  const files = document.getElementById('gd-files');
  const message = document.getElementById('gd-message');
  document.getElementById('loading').style.display = 'none';
  overlay.hidden = false;

  return new Promise((resolve) => {
    let busy = false;
    const handle = async (getFiles) => {
      if (busy) return;
      busy = true;
      message.className = '';
      message.textContent = 'Reading files...';
      try {
        const { files: picked, missing } = await collectFromFiles(await getFiles());
        if (missing.length > 0) {
          message.className = 'error';
          message.textContent = describeMissing(missing);
          return;
        }
        overlay.hidden = true;
        document.getElementById('loading').style.display = '';
        resolve(picked);
      } catch (e) {
        message.className = 'error';
        message.textContent = e.message;
      } finally {
        busy = false;
      }
    };

    for (const input of [folder, files]) {
      input.addEventListener('change', async () => {
        await handle(async () => [...input.files]);
        input.value = ''; // picking the same thing again must fire another change event
      });
    }
    drop.addEventListener('dragover', (e) => {
      e.preventDefault();
      drop.classList.add('over');
    });
    drop.addEventListener('dragleave', () => drop.classList.remove('over'));
    drop.addEventListener('drop', (e) => {
      e.preventDefault();
      drop.classList.remove('over');
      handle(() => filesFromDrop(e.dataTransfer));
    });
  });
}
