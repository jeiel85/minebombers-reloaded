// The page around the game: the top bar menu and the controls help.
//
// While either is open the keyboard belongs to it, not to the game, so an Enter on a menu item never
// plants a bomb and Escape closes the menu instead of leaving the game menu.

export function setupChrome() {
  const menuButton = document.getElementById('btn-menu');
  const panel = document.getElementById('menu-panel');
  const help = document.getElementById('controls-dialog');
  const isOpen = () => !panel.hidden;
  const setOpen = (open) => {
    panel.hidden = !open;
    menuButton.setAttribute('aria-expanded', String(open));
  };

  menuButton.addEventListener('click', (event) => {
    event.stopPropagation();
    setOpen(!isOpen());
  });

  // Speed and the two toggles keep the menu open so the effect can be seen; everything else closes it
  const staysOpen = (item) => item.classList.contains('btn-speed') || item.id === 'btn-crt' || item.id === 'btn-touch';
  panel.addEventListener('click', (event) => {
    const item = event.target.closest('button');
    if (!item) return;
    if (item.id === 'btn-controls') {
      setOpen(false);
      help.showModal();
    } else if (!staysOpen(item)) {
      setOpen(false);
    }
  });
  document.addEventListener('click', (event) => {
    if (isOpen() && !panel.contains(event.target) && event.target !== menuButton) setOpen(false);
  });

  document.getElementById('controls-close').addEventListener('click', () => help.close());
  help.addEventListener('click', (event) => {
    if (event.target === help) help.close(); // a click on the backdrop
  });

  // Capture phase, so this runs before the game's own key handler and can keep the key from it
  window.addEventListener(
    'keydown',
    (event) => {
      if (!isOpen() && !help.open) return;
      if (event.key === 'Escape' && isOpen()) {
        setOpen(false);
        menuButton.focus();
        event.preventDefault();
      }
      event.stopImmediatePropagation();
    },
    true
  );
}
