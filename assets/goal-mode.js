(() => {
  'use strict';

  // DSH owns goal state, rounds, pause/resume, completion, and persistence.
  // Desktop only provides a stable shortcut into the native /goal command.
  const isDshPage = () => location.hostname === '127.0.0.1' || location.hostname === 'localhost';
  const input = () => document.querySelector('textarea:not([disabled])');

  function activateGoalMode() {
    const editor = input();
    if (!editor) return;
    const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value')?.set;
    if (!setter) return;
    const value = editor.value.startsWith('/goal') ? editor.value : `/goal ${editor.value}`;
    setter.call(editor, value);
    editor.dispatchEvent(new Event('input', { bubbles: true }));
    editor.focus();
    editor.setSelectionRange(value.length, value.length);
  }
  window.__dshActivateGoalMode = activateGoalMode;

  function mountGoalButton() {
    if (!isDshPage() || !document.body || document.querySelector('[data-dsh-desktop-goal]')) return;
    const button = document.createElement('button');
    button.dataset.dshDesktopGoal = 'true';
    button.type = 'button';
    button.textContent = '◉ 目标模式';
    button.title = '原生 /goal · Ctrl/Cmd+Shift+G';
    button.addEventListener('click', activateGoalMode);
    Object.assign(button.style, {
      position: 'fixed', right: '200px', bottom: '18px', zIndex: '2147483647',
      border: '1px solid rgba(138,246,224,.32)', borderRadius: '999px',
      padding: '9px 13px', color: '#dffefa', background: 'rgba(13,29,39,.9)',
      boxShadow: '0 8px 24px rgba(0,0,0,.24)', backdropFilter: 'blur(16px)',
      font: '600 12px -apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif', cursor: 'pointer',
    });
    document.body.append(button);
  }

  document.addEventListener('keydown', (event) => {
    if ((event.ctrlKey || event.metaKey) && event.shiftKey && event.key.toLowerCase() === 'g') {
      event.preventDefault();
      activateGoalMode();
    }
  });
  const observer = new MutationObserver(mountGoalButton);
  if (document.documentElement) observer.observe(document.documentElement, { childList: true, subtree: true });
  else document.addEventListener('DOMContentLoaded', () => observer.observe(document.documentElement, { childList: true, subtree: true }), { once: true });
  setTimeout(mountGoalButton, 800);
})();
