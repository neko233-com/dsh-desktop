(() => {
  'use strict';

  const INITIAL_HIDDEN = __DSH_PET_HIDDEN__;
  const SPRITE_DATA_URL = '__DSH_PET_SPRITE__';
  const isDshPage = () => location.hostname === '127.0.0.1' || location.hostname === 'localhost';
  const ipc = (payload) => window.ipc && window.ipc.postMessage(JSON.stringify(payload));

  function activateGoalMode() {
    if (typeof window.__dshActivateGoalMode === 'function') {
      window.__dshActivateGoalMode();
    }
  }

  function installStyles() {
    if (document.querySelector('[data-dsh-pet-style]')) return;
    const style = document.createElement('style');
    style.dataset.dshPetStyle = 'true';
    style.textContent = `
      [data-dsh-desktop-pet] { position: fixed; right: 16px; bottom: 8px; width: 214px; height: 342px; z-index: 2147483646; pointer-events: none; font-family: -apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif; }
      [data-dsh-desktop-pet] .dsh-pet-card { position: absolute; inset: 0; transition: opacity .22s ease, transform .22s ease; }
      [data-dsh-desktop-pet][data-hidden="true"] .dsh-pet-card { opacity: 0; transform: translateY(10px) scale(.96); pointer-events: none; }
      [data-dsh-desktop-pet] .dsh-pet-character { position: absolute; right: 0; bottom: 0; width: 144px; height: 288px; padding: 0; border: 0; background: transparent center / 400% 100% no-repeat; cursor: pointer; animation: dsh-pet-frames 2.6s steps(4, end) infinite, dsh-pet-float 2.8s ease-in-out infinite; pointer-events: auto; filter: drop-shadow(0 10px 16px rgba(0,0,0,.25)); transition: transform .18s ease; }
      [data-dsh-desktop-pet] .dsh-pet-character:hover { transform: translateY(-5px) scale(1.03); }
      [data-dsh-desktop-pet][data-mood="working"] .dsh-pet-character { animation-duration: .86s, 1.5s; }
      [data-dsh-desktop-pet][data-mood="complete"] .dsh-pet-character { animation-duration: 1.2s, .9s; }
      [data-dsh-desktop-pet] .dsh-pet-bubble { position: absolute; right: 4px; top: 8px; max-width: 142px; padding: 8px 11px; border: 1px solid rgba(138,246,224,.26); border-radius: 14px 14px 4px 14px; color: #e7fffb; background: rgba(10,25,34,.86); box-shadow: 0 10px 28px rgba(0,0,0,.24); backdrop-filter: blur(14px); font-size: 12px; line-height: 1.35; text-align: center; pointer-events: none; }
      [data-dsh-desktop-pet] .dsh-pet-actions { position: absolute; right: 4px; top: 50px; display: flex; gap: 5px; opacity: 0; transform: translateY(-3px); transition: opacity .18s ease, transform .18s ease; pointer-events: none; }
      [data-dsh-desktop-pet]:hover .dsh-pet-actions, [data-dsh-desktop-pet]:focus-within .dsh-pet-actions { opacity: 1; transform: none; pointer-events: auto; }
      [data-dsh-desktop-pet] .dsh-pet-actions button, [data-dsh-desktop-pet] .dsh-pet-reopen { border: 1px solid rgba(138,246,224,.3); border-radius: 999px; padding: 6px 8px; color: #dffefa; background: rgba(13,29,39,.92); box-shadow: 0 8px 20px rgba(0,0,0,.2); font: 600 11px -apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif; cursor: pointer; }
      [data-dsh-desktop-pet] .dsh-pet-actions button:hover, [data-dsh-desktop-pet] .dsh-pet-reopen:hover { background: rgba(33,62,72,.96); }
      [data-dsh-desktop-pet] .dsh-pet-reopen { position: absolute; right: 0; bottom: 14px; display: none; pointer-events: auto; }
      [data-dsh-desktop-pet][data-hidden="true"] .dsh-pet-reopen { display: block; }
      [data-dsh-desktop-pet] .dsh-pet-sr { position: absolute; width: 1px; height: 1px; padding: 0; margin: -1px; overflow: hidden; clip: rect(0,0,0,0); white-space: nowrap; border: 0; }
      @keyframes dsh-pet-frames { from { background-position: 0 0; } to { background-position: 100% 0; } }
      @keyframes dsh-pet-float { 0%,100% { margin-bottom: 0; } 50% { margin-bottom: 4px; } }
    `;
    document.head?.append(style);
  }

  function mountPet() {
    if (!isDshPage() || !document.body || document.querySelector('[data-dsh-desktop-pet]')) return;
    installStyles();
    const root = document.createElement('aside');
    root.dataset.dshDesktopPet = 'true';
    root.dataset.hidden = String(INITIAL_HIDDEN);
    root.dataset.mood = 'idle';
    root.setAttribute('aria-label', 'DeepSeek 桌面宠物');
    root.innerHTML = `
      <div class="dsh-pet-card">
        <div class="dsh-pet-bubble" data-pet-bubble>准备好了，告诉我目标吧</div>
        <div class="dsh-pet-actions">
          <button type="button" data-pet-goal>目标模式</button>
          <button type="button" data-pet-hide>隐藏</button>
        </div>
        <button type="button" class="dsh-pet-character" data-pet-character aria-label="打开目标模式">
          <span class="dsh-pet-sr">打开目标模式</span>
        </button>
      </div>
      <button type="button" class="dsh-pet-reopen" data-pet-reopen aria-label="显示桌面宠物">◉ 显示宠物</button>
    `;
    document.body.append(root);
    root.querySelector('[data-pet-character]').style.backgroundImage = `url("${SPRITE_DATA_URL}")`;

    const bubble = root.querySelector('[data-pet-bubble]');
    let lastMood = '';
    let lastMessage = '';
    const setHidden = (hidden) => {
      root.dataset.hidden = String(hidden);
      try { localStorage.setItem('dsh-desktop-pet-hidden', hidden ? '1' : '0'); } catch (_) {}
      ipc({ type: 'pet_visibility', hidden });
    };
    window.__dshGetPetVisible = () => root.dataset.hidden !== 'true';
    window.__dshSetPetHidden = setHidden;
    const refreshMood = () => {
      const goal = document.querySelector('[data-goal-bar]');
      const text = (goal?.textContent || '').toLowerCase();
      let mood = 'idle';
      let message = '准备好了，告诉我目标吧';
      if (/完成|complete|成功/.test(text)) {
        mood = 'complete';
        message = '目标完成，做得漂亮！';
      } else if (/进行|running|处理中|working|执行/.test(text)) {
        mood = 'working';
        message = '正在专注执行…';
      } else if (/暂停|pause/.test(text)) {
        message = '已暂停，随时继续';
      }
      if (mood === lastMood && message === lastMessage) return;
      lastMood = mood;
      lastMessage = message;
      root.dataset.mood = mood;
      bubble.textContent = message;
    };

    root.querySelector('[data-pet-character]').addEventListener('click', activateGoalMode);
    root.querySelector('[data-pet-goal]').addEventListener('click', activateGoalMode);
    root.querySelector('[data-pet-hide]').addEventListener('click', () => setHidden(true));
    root.querySelector('[data-pet-reopen]').addEventListener('click', () => setHidden(false));
    refreshMood();
    const observer = new MutationObserver(refreshMood);
    observer.observe(document.body, { childList: true, subtree: true, characterData: true });
  }

  const observer = new MutationObserver(mountPet);
  if (document.documentElement) observer.observe(document.documentElement, { childList: true, subtree: true });
  else document.addEventListener('DOMContentLoaded', () => observer.observe(document.documentElement, { childList: true, subtree: true }), { once: true });
  setTimeout(mountPet, 900);
})();
