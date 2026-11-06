import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import './style.css';

type Version = { id: string; type: string; url: string; sha1?: string };
type Player = { name: string; id: string };
type InstallProgress = { version: string; done: number; total: number; message: string };
type DevicePrompt = { userCode: string; verificationUri: string; expiresIn: number; message: string };

type Page = 'instances' | 'accounts' | 'settings' | 'console';
const root = document.getElementById('app')!;
const saved = <T>(key: string, fallback: T): T => {
  const value = localStorage.getItem(key);
  try { return value === null ? fallback : JSON.parse(value) as T; }
  catch { return fallback; }
};
let page: Page = 'instances';
let versions: Version[] = [];
let installed: string[] = [];
let selected = saved('selected', '');
let showSnapshots = saved('snapshots', false);
let javaPath = saved('javaPath', 'java');
let memoryMb = saved('memoryMb', 4096);
let clientId = saved('clientId', '');
let account: Player | null = null;
let loginPrompt: DevicePrompt | null = null;
let loginWaiting = false;
let progress: InstallProgress | null = null;
let loading = true;
let busy = false;
let status = 'Ready';
let error = '';
let gameLogs: string[] = [];

const esc = (value: unknown): string => String(value).replace(/[&<>"']/g, c =>
  ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c] ?? c));
const setSaved = (key: string, value: unknown) => localStorage.setItem(key, JSON.stringify(value));
const pct = (): number => progress && progress.total ? Math.floor(100 * progress.done / progress.total) : 0;

function navButton(id: Page, label: string, icon: string): string {
  return `<button class="nav-item ${page === id ? 'active' : ''}" data-action="page" data-page="${id}">
    <span class="nav-icon">${icon}</span><span>${label}</span></button>`;
}
function versionLabel(v: Version): string { return `${v.id}${v.type === 'snapshot' ? ' · Snapshot' : ''}`; }
function versionsView(): string {
  const filtered = versions.filter(v => showSnapshots || v.type === 'release');
  const displayed = [...new Set([...installed, ...filtered.map(v => v.id)])];
  const options = displayed.map(id => {
    const v = versions.find(v => v.id === id);
    const label = v ? versionLabel(v) : `${id} · Installed`;
    return `<option value="${esc(id)}" ${selected === id ? 'selected' : ''}>${esc(label)}</option>`;
  }).join('');
  const title = selected || 'Select a Minecraft version';
  const isReady = installed.includes(selected);
  const v = versions.find(v => v.id === selected);
  return `<div class="heading"><div><h1>Instances</h1><p>Choose, install, and play Minecraft Java Edition.</p></div>
    <span class="chip">Vanilla · V0.1</span></div>
    <section class="panel">
      <div class="fieldline"><label for="version">Minecraft version</label><span class="quiet">${esc(versions.length ? `${versions.length} versions available` : 'Local / offline')}</span></div>
      <select id="version" ${busy ? 'disabled' : ''} aria-label="Minecraft version">${options || '<option value="">No versions loaded</option>'}</select>
      <label class="checkline"><input type="checkbox" id="snapshots" ${showSnapshots ? 'checked' : ''}/> Show snapshots</label>
      <div class="instance-card"><div class="block-icon">▣</div><div class="instance-copy">
        <strong>${esc(title)}</strong>
        <small>${isReady ? 'Installed and ready' : 'Not installed'}${v ? ` · ${esc(v.type)}` : ''}</small></div>
        <span class="${isReady ? 'dot-ready' : 'dot-empty'}"></span>
      </div>
      <div class="buttons">
        <button class="btn secondary" data-action="install" ${busy || !selected || !v ? 'disabled' : ''}>${isReady ? 'Verify / Repair' : 'Install'}</button>
        <button class="btn primary" data-action="play" ${busy || !selected || !account ? 'disabled' : ''}>▶ Play</button>
      </div>
      ${!account ? '<div class="hint">Sign in with a Microsoft account on the Accounts tab before playing.</div>' : ''}
      ${progress ? `<div class="progress-info"><span>${esc(progress.message)}</span><span>${pct()}% (${progress.done}/${progress.total})</span></div>
        <div class="progress-track"><div class="progress-fill" id="progressFill"></div></div>` : ''}
    </section>
    <div class="tiny-note">Minecraft files are downloaded from official Mojang services. A purchased Minecraft Java Edition account is required.</div>`;
}
function accountsView(): string {
  return `<div class="heading"><div><h1>Accounts</h1><p>Microsoft authentication through the official sign-in website.</p></div></div>
    <section class="panel">
      ${account ? `<div class="account-row"><span class="avatar">${esc(account.name.charAt(0).toUpperCase())}</span>
        <div><strong>${esc(account.name)}</strong><small>Signed in · Minecraft Java profile</small></div></div>
        <button data-action="logout" class="btn secondary" ${busy ? 'disabled' : ''}>Sign out</button>` : `
      <label for="clientIdAccount">Microsoft application (client) ID</label>
      <input id="clientIdAccount" spellcheck="false" placeholder="Azure app registration client ID" value="${esc(clientId)}" />
      <p class="description">Register a public client application and obtain Minecraft API permission. This launcher does not ask for your Microsoft password.</p>
      <button data-action="login" class="btn primary" ${busy || loginWaiting ? 'disabled' : ''}>Sign in with Microsoft</button>
      ${loginPrompt ? `<div class="login-code"><small>Open the address below in your browser and enter this code:</small>
        <strong class="code">${esc(loginPrompt.userCode)}</strong>
        <span class="linklike">${esc(loginPrompt.verificationUri)}</span>
        <button class="btn secondary small" data-action="copy-code">Copy code</button>
        <small>${loginWaiting ? 'Waiting for Microsoft authorization…' : 'Login not in progress'}</small></div>` : ''}
      `}
    </section>`;
}
function settingsView(): string {
  return `<div class="heading"><div><h1>Settings</h1><p>Configure your Java installation and launcher preferences.</p></div></div>
    <section class="panel form-grid">
      <label for="javaPath">Java executable</label>
      <input id="javaPath" value="${esc(javaPath)}" spellcheck="false" placeholder="java or full path" />
      <p class="description">Example: <code>java</code> for PATH, or <code>/usr/lib/jvm/.../bin/java</code> on Linux. Minecraft versions may require Java 8, 17, 21, or 25.</p>
      <label for="memoryMb">Maximum RAM allocation (MB)</label>
      <input id="memoryMb" type="number" min="1024" max="32768" step="256" value="${memoryMb}" />
      <label for="clientId">Microsoft application (client) ID</label>
      <input id="clientId" spellcheck="false" value="${esc(clientId)}" placeholder="Public app client ID" />
      <button class="btn primary" data-action="save-settings">Save settings</button>
    </section>
    <p class="tiny-note">Preferences are stored locally. Minecraft login tokens are kept in the operating system's credential store when available.</p>`;
}
function consoleView(): string {
  return `<div class="heading"><div><h1>Game console</h1><p>Recent output from your running Minecraft process.</p></div>
    <button class="btn secondary small" data-action="clear-console">Clear</button></div>
    <pre class="console" id="console">${esc(gameLogs.slice(-200).join('\n') || 'No game output yet. Launch Minecraft to see logs.')}</pre>`;
}
function render(): void {
  root.innerHTML = `<div class="shell">
  <aside class="sidebar"><div class="brand"><div class="brand-icon">▣</div><span>Pebble</span><small>Launcher</small></div>
    <div class="nav">${navButton('instances', 'Instances', '▦')}${navButton('accounts', 'Accounts', '◉')}
      ${navButton('settings', 'Settings', '⚙')}${navButton('console', 'Console', '⌘')}</div>
    <div class="sidebar-bottom">Java Edition <span>0.1.0</span></div>
  </aside><main class="content">
    ${loading ? '<div class="loading">Loading Minecraft versions…</div>' : page === 'instances' ? versionsView() : page === 'accounts' ? accountsView() : page === 'settings' ? settingsView() : consoleView()}
    ${error ? `<div class="notice error">${esc(error)} <button data-action="dismiss" aria-label="Dismiss">×</button></div>` : ''}
  </main><div class="bottom-bar"><span class="status-led"></span><span>${esc(status)}</span>
  <span class="bottom-account">${esc(account ? account.name : 'Not signed in')}</span></div></div>`;
  const fill = document.getElementById('progressFill');
  if (fill) fill.style.width = `${pct()}%`;
  const log = document.getElementById('console');
  if (log && page === 'console') log.scrollTop = log.scrollHeight;
}
async function handle(action: string): Promise<void> {
  error = '';
  try {
    switch (action) {
      case 'install':
        if (!selected || busy) return;
        busy = true; status = `Installing ${selected}`; render();
        await invoke('install_version', { versionId: selected });
        if (!installed.includes(selected)) installed.push(selected);
        status = `${selected} installed`;
        break;
      case 'play':
        if (!selected || busy) return;
        busy = true; status = `Preparing ${selected}`; render();
        if (!installed.includes(selected)) {
          await invoke('install_version', { versionId: selected });
          installed.push(selected);
        }
        await invoke('launch_game', { versionId: selected, javaPath, memoryMb });
        status = `Launched ${selected}`;
        break;
      case 'login': {
        if (busy || loginWaiting) return;
        readClientId();
        busy = true; status = 'Starting Microsoft sign-in'; render();
        loginPrompt = await invoke<DevicePrompt>('start_login', { clientId });
        loginWaiting = true; busy = false;
        page = 'accounts'; status = 'Waiting for sign-in'; render();
        void completeSignIn();
        return;
      }
      case 'logout':
        busy = true; render();
        await invoke('logout'); account = null; loginPrompt = null; status = 'Signed out';
        break;
      case 'save-settings': {
        javaPath = (document.getElementById('javaPath') as HTMLInputElement).value.trim() || 'java';
        memoryMb = Number((document.getElementById('memoryMb') as HTMLInputElement).value);
        clientId = (document.getElementById('clientId') as HTMLInputElement).value.trim();
        if (!Number.isInteger(memoryMb) || memoryMb < 1024 || memoryMb > 32768) throw new Error('RAM must be 1024–32768 MB');
        setSaved('javaPath', javaPath); setSaved('memoryMb', memoryMb); setSaved('clientId', clientId);
        status = 'Settings saved';
        break;
      }
      case 'copy-code':
        if (loginPrompt) { await navigator.clipboard.writeText(loginPrompt.userCode); status = 'Code copied'; }
        break;
      case 'clear-console': gameLogs = []; status = 'Console cleared'; break;
      case 'dismiss': error = ''; break;
    }
  } catch (cause) {
    error = String(cause);
    status = 'Operation failed';
  } finally {
    busy = false;
    render();
  }
}
function readClientId(): void {
  const input = document.getElementById('clientIdAccount') as HTMLInputElement | null;
  if (input) { clientId = input.value.trim(); setSaved('clientId', clientId); }
}
async function completeSignIn(): Promise<void> {
  try {
    account = await invoke<Player>('complete_login');
    status = `Signed in as ${account.name}`;
    error = '';
  } catch (cause) {
    error = String(cause); status = 'Sign-in failed';
  } finally {
    loginWaiting = false; loginPrompt = null; render();
  }
}
root.addEventListener('click', event => {
  const button = (event.target as HTMLElement).closest<HTMLElement>('[data-action]');
  if (!button) return;
  const action = button.dataset.action!;
  if (action === 'page') { page = button.dataset.page as Page; error = ''; render(); }
  else { void handle(action); }
});
root.addEventListener('change', event => {
  const target = event.target as HTMLInputElement;
  if (target.id === 'version') { selected = target.value; setSaved('selected', selected); progress = null; render(); }
  if (target.id === 'snapshots') { showSnapshots = target.checked; setSaved('snapshots', showSnapshots); render(); }
});
async function init(): Promise<void> {
  render();
  await listen<InstallProgress>('install-progress', ({ payload }) => { progress = payload; if (page === 'instances') render(); });
  await listen<{ version: string; message: string }>('game-event', ({ payload }) => {
    gameLogs.push(`[${payload.version}] ${payload.message}`);
    if (gameLogs.length > 300) gameLogs = gameLogs.slice(-200);
    if (page === 'console') render();
  });
  const results = await Promise.allSettled([
    invoke<Version[]>('list_versions'),
    invoke<string[]>('list_installed'),
    invoke<Player | null>('account_info'),
  ]);
  if (results[0].status === 'fulfilled') versions = results[0].value;
  else { error = `Version list unavailable: ${String(results[0].reason)}`; }
  if (results[1].status === 'fulfilled') installed = results[1].value;
  if (results[2].status === 'fulfilled') account = results[2].value;
  if (!selected || (!versions.some(v => v.id === selected) && !installed.includes(selected))) {
    selected = versions.find(v => v.type === 'release')?.id || installed[0] || '';
  }
  loading = false; status = versions.length ? 'Ready' : 'Offline · local versions only';
  render();
}
void init().catch(cause => { loading = false; error = String(cause); render(); });
