(() => {
  'use strict';

  const ipc = (payload) => window.ipc && window.ipc.postMessage(JSON.stringify(payload));

  function mount() {
    if (!document.body || document.querySelector('[data-dsh-window-chrome]')) return;

    const root = document.createElement('div');
    root.dataset.dshWindowChrome = 'true';
    root.attachShadow({ mode: 'open' });
    root.shadowRoot.innerHTML = `
      <style>
        :host { all: initial; }
        .bar { position: fixed; inset: 0 0 auto; height: 42px; z-index: 2147483647; display: flex; align-items: center; justify-content: space-between; padding: 0 14px 0 18px; color: #c7d3e7; font: 600 12px -apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif; user-select: none; pointer-events: none; }
        .drag { position: absolute; inset: 0 170px 0 0; pointer-events: auto; cursor: default; }
        .brand { position: relative; display: flex; align-items: center; gap: 8px; letter-spacing: .01em; opacity: .82; pointer-events: none; }
        .brand-mark { width: 8px; height: 8px; border-radius: 50%; background: #83f4df; box-shadow: 0 0 14px #83f4df; }
        .actions { position: relative; display: flex; align-items: center; gap: 8px; pointer-events: auto; }
        .settings { border: 1px solid rgba(138,246,224,.18); border-radius: 999px; padding: 7px 11px; color: #dffefa; background: rgba(13,29,39,.72); box-shadow: 0 8px 24px rgba(0,0,0,.14); backdrop-filter: blur(18px); font: 600 11px -apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif; cursor: pointer; }
        .settings:hover { background: rgba(36,67,77,.9); }
        .lights { display: flex; gap: 7px; padding: 7px 9px; border: 1px solid rgba(255,255,255,.12); border-radius: 999px; background: rgba(9,15,25,.72); box-shadow: 0 8px 24px rgba(0,0,0,.2); backdrop-filter: blur(18px); }
        .light { width: 12px; height: 12px; padding: 0; border: 0; border-radius: 50%; cursor: pointer; box-shadow: inset 0 1px rgba(255,255,255,.35), 0 0 0 1px rgba(0,0,0,.18); }
        .light:hover { filter: brightness(1.18); transform: scale(1.08); }
        .close { background: #ff625d; }
        .minimize { background: #ffbd44; }
        .maximize { background: #28c840; }
        .panel { position: fixed; top: 52px; right: 16px; width: 286px; padding: 16px; border: 1px solid rgba(138,246,224,.2); border-radius: 18px; color: #dce8f6; background: rgba(13,23,34,.94); box-shadow: 0 22px 60px rgba(0,0,0,.35); backdrop-filter: blur(24px); pointer-events: auto; }
        .panel[hidden] { display: none; }
        .panel h2 { margin: 0 0 13px; color: #f7fbff; font-size: 15px; }
        .panel p { margin: 0 0 14px; color: #8fa2bb; font-size: 12px; line-height: 1.55; }
        .row { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 11px 0; border-top: 1px solid rgba(255,255,255,.08); font-size: 12px; }
        .row strong { display: block; margin-bottom: 3px; color: #eef7ff; font-size: 12px; }
        .row small { color: #8397b0; }
        .switch { position: relative; width: 38px; height: 22px; flex: 0 0 auto; }
        .switch input { width: 0; height: 0; opacity: 0; }
        .track { position: absolute; inset: 0; border-radius: 999px; background: #334255; cursor: pointer; transition: .18s; }
        .track::after { content: ""; position: absolute; top: 3px; left: 3px; width: 16px; height: 16px; border-radius: 50%; background: #e9f5ff; transition: .18s; }
        .switch input:checked + .track { background: #32c9ad; }
        .switch input:checked + .track::after { transform: translateX(16px); }
        .reset { width: 100%; margin-top: 12px; padding: 9px 10px; border: 1px solid rgba(255,255,255,.12); border-radius: 10px; color: #e9f4ff; background: rgba(255,255,255,.07); cursor: pointer; font: 600 12px -apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif; }
        .reset:hover { background: rgba(255,255,255,.12); }
      </style>
      <div class="bar">
        <div class="drag" data-dsh-drag></div>
        <div class="brand"><span class="brand-mark"></span><span>DSH Desktop</span></div>
        <div class="actions">
          <button class="settings" type="button" data-settings>设置</button>
          <div class="lights" aria-label="窗口控制">
            <button class="light close" type="button" data-window="close" aria-label="关闭"></button>
            <button class="light minimize" type="button" data-window="minimize" aria-label="最小化"></button>
            <button class="light maximize" type="button" data-window="maximize" aria-label="最大化"></button>
          </div>
        </div>
        <section class="panel" data-panel hidden>
          <h2>本机设置</h2>
          <p>必须完成 API Key 配置后才能进入工作台。Key 只保存在系统钥匙串，不写入项目。</p>
          <div class="row"><div><strong>桌面宠物</strong><small>默认显示，可随时隐藏</small></div><label class="switch"><input type="checkbox" data-pet-visible checked><span class="track"></span></label></div>
          <div class="row"><div><strong>DeepSeek API Key</strong><small>由系统钥匙串管理</small></div><span>● 已保护</span></div>
          <button class="reset" type="button" data-reset-key>重新设置 API Key</button>
        </section>
      </div>
    `;
    document.body.append(root);

    const shadow = root.shadowRoot;
    const panel = shadow.querySelector('[data-panel]');
    const petVisible = shadow.querySelector('[data-pet-visible]');
    const syncPet = () => {
      if (typeof window.__dshGetPetVisible === 'function') petVisible.checked = window.__dshGetPetVisible();
    };
    shadow.querySelector('[data-settings]').addEventListener('click', () => {
      panel.hidden = !panel.hidden;
      syncPet();
    });
    shadow.querySelectorAll('[data-window]').forEach((button) => {
      button.addEventListener('click', () => ipc({ type: `window_${button.dataset.window}` }));
    });
    shadow.querySelector('[data-reset-key]').addEventListener('click', () => ipc({ type: 'reset_key' }));
    petVisible.addEventListener('change', () => {
      if (typeof window.__dshSetPetHidden === 'function') window.__dshSetPetHidden(!petVisible.checked);
      else ipc({ type: 'pet_visibility', hidden: !petVisible.checked });
    });
    shadow.querySelector('[data-dsh-drag]').addEventListener('pointerdown', (event) => {
      if (event.button === 0) ipc({ type: 'window_drag' });
    });
    shadow.querySelector('[data-dsh-drag]').addEventListener('dblclick', () => ipc({ type: 'window_maximize' }));
    document.addEventListener('keydown', (event) => {
      if (event.key === 'Escape') panel.hidden = true;
    });
  }

  const observer = new MutationObserver(mount);
  if (document.documentElement) observer.observe(document.documentElement, { childList: true, subtree: true });
  else document.addEventListener('DOMContentLoaded', () => observer.observe(document.documentElement, { childList: true, subtree: true }), { once: true });
  setTimeout(mount, 50);
})();
