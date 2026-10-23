<script lang="ts">
  import { api } from '../api'
  import Icon from './Icon.svelte'
  import type { ConnectionStatus, DiscoveryReport, SettingsView } from '../types'

  let {
    open,
    settings,
    status,
    busy = '',
    onClose,
    onSaveObs,
    onSaveRecording,
    onSaveStream,
    onReconnect,
    onPrepareScenes,
  }: {
    open: boolean
    settings: SettingsView | null
    status: ConnectionStatus
    busy?: string
    onClose: () => void
    onSaveObs: (host: string, port: number, password?: string) => void
    onSaveRecording: (path: string | null, format: string) => void
    onSaveStream: (server: string | null, key?: string) => void
    onReconnect: () => void
    onPrepareScenes: () => void
  } = $props()

  type Tab = 'connection' | 'outputs' | 'plugins'
  let tab = $state<Tab>('connection')
  let host = $state('127.0.0.1')
  let port = $state(4455)
  let password = $state('')
  let recordPath = $state('')
  let recordFormat = $state('hybrid_mp4')
  let streamServer = $state('')
  let streamKey = $state('')
  let discovery = $state<DiscoveryReport | null>(null)
  let pluginBusy = $state(false)
  let pluginError = $state('')

  $effect(() => {
    if (!open || !settings) return
    host = settings.obs_host
    port = settings.obs_port
    password = ''
    recordPath = settings.record_path ?? ''
    recordFormat = settings.record_format
    streamServer = settings.stream_server ?? ''
    streamKey = ''
  })

  function submitConnection(event: SubmitEvent) {
    event.preventDefault()
    onSaveObs(host.trim(), Number(port), password)
    password = ''
  }

  function submitRecording(event: SubmitEvent) {
    event.preventDefault()
    onSaveRecording(recordPath.trim() || null, recordFormat)
  }

  function submitStream(event: SubmitEvent) {
    event.preventDefault()
    onSaveStream(streamServer.trim() || null, streamKey)
    streamKey = ''
  }

  async function checkPlugins() {
    pluginBusy = true
    pluginError = ''
    try {
      discovery = await api.pluginHealth()
    } catch (error) {
      pluginError = String(error)
    } finally {
      pluginBusy = false
    }
  }

  function closeOnEscape(event: KeyboardEvent) {
    if (open && event.key === 'Escape') onClose()
  }
</script>

<svelte:window onkeydown={closeOnEscape} />

{#if open}
  <div
    class="backdrop"
    role="presentation"
    onclick={(event) => event.target === event.currentTarget && onClose()}
  >
    <div class="modal" role="dialog" aria-modal="true" aria-label="Studio settings">
      <header class="modal-head">
        <div>
          <span class="eyebrow">Configuration</span>
          <h2>Studio settings</h2>
        </div>
        <button type="button" class="close" onclick={onClose} aria-label="Close settings">
          <Icon name="x" size={17} />
        </button>
      </header>

      <div class="modal-body">
        <nav class="tabs" aria-label="Settings sections">
          <button type="button" class:active={tab === 'connection'} onclick={() => (tab = 'connection')}>
            <Icon name="projector" size={15} /> Connection
          </button>
          <button type="button" class:active={tab === 'outputs'} onclick={() => (tab = 'outputs')}>
            <Icon name="record" size={15} /> Outputs
          </button>
          <button type="button" class:active={tab === 'plugins'} onclick={() => (tab = 'plugins')}>
            <Icon name="sparkles" size={15} /> Plugins
          </button>
          <div class="tab-foot">
            <Icon name="lock" size={12} />
            <span>Secrets stay local. Stream keys live in the Windows credential store.</span>
          </div>
        </nav>

        <div class="tab-content">
          {#if tab === 'connection'}
            <section class="pane">
              <div class="status-card" class:online={status.connected}>
                <span class="status-dot" class:online={status.connected}></span>
                <div>
                  <strong>{status.connected ? 'OBS WebSocket connected' : 'OBS not connected'}</strong>
                  <p>{status.message}</p>
                  {#if status.obs_version}
                    <span class="version-line">
                      OBS {status.obs_version} · obs-websocket {status.websocket_version} · RPC {status.rpc_version}
                    </span>
                  {/if}
                </div>
              </div>

              <form onsubmit={submitConnection}>
                <div class="form-row two">
                  <label>
                    <span>Host</span>
                    <input type="text" bind:value={host} placeholder="127.0.0.1" autocomplete="off" />
                  </label>
                  <label>
                    <span>WebSocket port</span>
                    <input type="number" min="1" max="65535" bind:value={port} />
                  </label>
                </div>
                <label>
                  <span>Password</span>
                  <input
                    type="password"
                    bind:value={password}
                    placeholder={settings?.has_obs_password ? 'Saved / auto-detected — leave blank to keep' : 'OBS WebSocket password'}
                    autocomplete="new-password"
                  />
                  <small>
                    {settings?.has_obs_password
                      ? 'A password is available. Leave this blank to keep using it.'
                      : 'No password has been read yet. Enter the OBS WebSocket password if required.'}
                  </small>
                </label>
                <div class="form-actions">
                  <button type="submit" class="primary" disabled={busy === 'settings'}>
                    {busy === 'settings' ? 'Saving…' : 'Save & reconnect'}
                  </button>
                  <button type="button" class="secondary" onclick={onReconnect} disabled={busy === 'reconnect'}>
                    <Icon name="refresh" size={13} /> Reconnect now
                  </button>
                </div>
              </form>

              <div class="divider"></div>

              <div class="inline-card">
                <div>
                  <strong>Scene preparation</strong>
                  <p>Create or refresh the eight layouts, camera group, filters, and Move transition in OBS.</p>
                </div>
                <button type="button" class="secondary" onclick={onPrepareScenes} disabled={!status.connected || busy === 'prepare'}>
                  {busy === 'prepare' ? 'Preparing…' : 'Prepare scenes'}
                </button>
              </div>
            </section>
          {:else if tab === 'outputs'}
            <section class="pane">
              <h3>Recording</h3>
              <form onsubmit={submitRecording}>
                <label>
                  <span>Recording folder</span>
                  <input type="text" bind:value={recordPath} placeholder="Leave blank to use the OBS profile default" />
                  <small>OBS must be able to write to this folder. Existing OBS settings are used when blank.</small>
                </label>
                <label>
                  <span>Format</span>
                  <select bind:value={recordFormat}>
                    <option value="hybrid_mp4">Hybrid MP4 (safe + compatible)</option>
                    <option value="mkv">MKV</option>
                    <option value="mp4">MP4</option>
                  </select>
                </label>
                <div class="form-actions">
                  <button type="submit" class="primary" disabled={busy === 'settings'}>Save recording</button>
                  <button type="button" class="secondary" onclick={() => { recordPath = settings?.record_path ?? ''; recordFormat = settings?.record_format ?? 'hybrid_mp4' }}>
                    Reset
                  </button>
                </div>
              </form>

              <div class="divider"></div>

              <h3>Streaming</h3>
              <form onsubmit={submitStream}>
                <label>
                  <span>Stream server</span>
                  <input type="text" bind:value={streamServer} placeholder="rtmp://live.example.com/app" autocomplete="off" />
                </label>
                <label>
                  <span>Stream key</span>
                  <div class="secret-input">
                    <input
                      type="password"
                      bind:value={streamKey}
                      placeholder={settings?.has_stream_key ? 'Saved in Windows credential store — leave blank to keep' : 'Paste stream key'}
                      autocomplete="new-password"
                    />
                    {#if settings?.has_stream_key}<span class="saved-badge"><Icon name="check" size={10} /> Stored</span>{/if}
                  </div>
                  <small>The key is written to Windows Credential Manager through the app's keyring entry. It is never written to settings JSON or logs.</small>
                </label>
                <div class="form-actions">
                  <button type="submit" class="primary" disabled={busy === 'settings'}>Save destination</button>
                  <button
                    type="button"
                    class="secondary danger"
                    onclick={() => onSaveStream(streamServer.trim() || null, '')}
                    disabled={!settings?.has_stream_key || busy === 'settings'}
                  >
                    Remove key
                  </button>
                </div>
                <p class="footnote">
                  V1 publishes to the OBS output plus the Multi RTMP plugin destinations configured in OBS.
                </p>
              </form>
            </section>
          {:else}
            <section class="pane">
              <div class="pane-title-row">
                <div>
                  <h3>Required OBS plugins</h3>
                  <p class="pane-copy">All five plugins are required. The status check also reports the OBS runtime and canvas list.</p>
                </div>
                <button type="button" class="secondary" onclick={checkPlugins} disabled={pluginBusy || !status.connected}>
                  <Icon name="refresh" size={13} /> {pluginBusy ? 'Checking…' : 'Check now'}
                </button>
              </div>

              {#if pluginError}
                <div class="error-card">{pluginError}</div>
              {/if}

              {#if discovery}
                <div class="runtime-card">
                  <span class:online={discovery.ready} class="runtime-dot"></span>
                  <div>
                    <strong>{discovery.ready ? 'Plugin set ready' : 'Plugin set incomplete'}</strong>
                    <span>
                      OBS {discovery.runtime.obs_studio_version} · WebSocket {discovery.runtime.obs_websocket_version}
                      · {discovery.runtime.canvas_list.length} canvases
                    </span>
                  </div>
                </div>
                <div class="plugin-list">
                  {#each discovery.plugins as plugin (plugin.key)}
                    <div class="plugin" class:missing={!plugin.installed}>
                      <span class="plugin-icon" class:ok={plugin.installed}>
                        <Icon name={plugin.installed ? 'check' : 'x'} size={12} />
                      </span>
                      <div class="plugin-copy">
                        <strong>{plugin.name}</strong>
                        <span>{plugin.evidence[0] ?? (plugin.installed ? 'Detected' : 'Not found')}</span>
                      </div>
                      <span class="plugin-state" class:ok={plugin.installed}>{plugin.installed ? 'Installed' : 'Missing'}</span>
                    </div>
                  {/each}
                </div>
                {#if discovery.warnings.length}
                  <div class="warning-card">
                    {#each discovery.warnings as warning (warning)}
                      <p>{warning}</p>
                    {/each}
                  </div>
                {/if}
              {:else if !pluginError}
                <div class="empty-state">
                  <Icon name="sparkles" size={24} />
                  <strong>Run a health check</strong>
                  <p>Confirms Move Transition, Advanced Masks, Aitum Vertical, Background Removal, and Multi RTMP.</p>
                </div>
              {/if}
            </section>
          {/if}
        </div>
      </div>

      <footer class="modal-foot">
        <span>{settings?.settings_path ?? 'Settings are stored in the app config directory.'}</span>
        <button type="button" class="secondary" onclick={onClose}>Done</button>
      </footer>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 100;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 28px;
    background: rgba(2, 4, 8, 0.66);
    backdrop-filter: blur(7px);
    animation: fade-in 140ms ease;
  }

  .modal {
    width: min(820px, 100%);
    max-height: min(720px, calc(100vh - 56px));
    display: flex;
    flex-direction: column;
    border-radius: 14px;
    border: 1px solid var(--border);
    background: var(--surface-1);
    box-shadow:
      0 40px 100px -26px rgba(0, 0, 0, 0.9),
      0 0 0 1px rgba(255, 255, 255, 0.02);
    overflow: hidden;
    animation: rise-in 180ms cubic-bezier(0.22, 1, 0.36, 1);
  }

  .modal-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 14px;
    padding: 18px 20px 15px;
    border-bottom: 1px solid var(--border-soft);
  }

  .eyebrow {
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--text-faint);
  }

  .modal-head h2 {
    margin: 4px 0 0;
    font-size: 19px;
    font-weight: 650;
    letter-spacing: -0.025em;
  }

  .close {
    width: 29px;
    height: 29px;
    display: grid;
    place-items: center;
    border-radius: 7px;
    border: 1px solid var(--border-soft);
    background: transparent;
    color: var(--text-faint);
    cursor: pointer;
  }

  .close:hover {
    color: var(--text);
    border-color: var(--border);
    background: var(--surface-2);
  }

  .modal-body {
    flex: 1;
    min-height: 0;
    display: flex;
  }

  .tabs {
    width: 186px;
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: 14px 11px;
    border-right: 1px solid var(--border-soft);
    background: color-mix(in srgb, var(--surface-0) 45%, transparent);
  }

  .tabs > button {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 34px;
    padding: 0 9px;
    border-radius: 7px;
    border: 1px solid transparent;
    background: transparent;
    color: var(--text-faint);
    font-size: 10.5px;
    font-weight: 550;
    text-align: left;
    cursor: pointer;
    transition: 120ms ease;
  }

  .tabs > button:hover {
    color: var(--text-muted);
    background: var(--surface-2);
  }

  .tabs > button.active {
    color: var(--text);
    background: var(--surface-3);
    border-color: var(--border-soft);
  }

  .tab-foot {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    margin-top: auto;
    padding: 12px 4px 2px;
    color: var(--text-faint);
    font-size: 8.5px;
    line-height: 1.45;
  }

  .tab-foot :global(svg) {
    flex: none;
    margin-top: 1px;
  }

  .tab-content {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    scrollbar-width: thin;
    scrollbar-color: #323a48 transparent;
  }

  .pane {
    padding: 20px 22px 24px;
  }

  h3 {
    margin: 0 0 12px;
    font-size: 12.5px;
    font-weight: 650;
    letter-spacing: -0.01em;
  }

  .status-card {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 12px;
    margin-bottom: 18px;
    border-radius: 9px;
    border: 1px solid var(--border-soft);
    background: var(--surface-2);
  }

  .status-card.online {
    border-color: rgba(66, 207, 154, 0.17);
    background: rgba(46, 167, 120, 0.06);
  }

  .status-dot {
    width: 7px;
    height: 7px;
    margin-top: 4px;
    border-radius: 50%;
    background: var(--danger);
    flex: none;
  }

  .status-dot.online {
    background: var(--success);
    box-shadow: 0 0 0 3px rgba(65, 210, 158, 0.11);
  }

  .status-card strong {
    display: block;
    color: var(--text-muted);
    font-size: 10.5px;
    font-weight: 600;
  }

  .status-card p {
    margin: 3px 0 0;
    color: var(--text-faint);
    font-size: 9.5px;
    line-height: 1.45;
  }

  .version-line {
    display: block;
    margin-top: 5px;
    color: var(--text-faint);
    font-size: 8.5px;
    font-variant-numeric: tabular-nums;
  }

  form {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 5px;
    color: var(--text-muted);
    font-size: 9.5px;
    font-weight: 550;
  }

  label small {
    color: var(--text-faint);
    font-size: 8.5px;
    font-weight: 400;
    line-height: 1.45;
  }

  input,
  select {
    width: 100%;
    height: 32px;
    padding: 0 9px;
    border-radius: 7px;
    border: 1px solid var(--border-soft);
    background: var(--surface-2);
    color: var(--text);
    font-size: 10.5px;
    outline: none;
    transition: border-color 120ms ease;
  }

  input:focus,
  select:focus {
    border-color: color-mix(in srgb, var(--accent) 50%, var(--border));
  }

  input::placeholder {
    color: var(--text-faint);
  }

  select {
    appearance: none;
    cursor: pointer;
    background-image:
      linear-gradient(45deg, transparent 50%, var(--text-faint) 50%),
      linear-gradient(135deg, var(--text-faint) 50%, transparent 50%);
    background-position:
      calc(100% - 13px) 14px,
      calc(100% - 9px) 14px;
    background-size: 4px 4px, 4px 4px;
    background-repeat: no-repeat;
  }

  .form-row.two {
    display: grid;
    grid-template-columns: 1fr 130px;
    gap: 10px;
  }

  .form-actions {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 2px;
    flex-wrap: wrap;
  }

  .primary {
    height: 31px;
    padding: 0 13px;
    border-radius: 7px;
    border: 1px solid transparent;
    background: linear-gradient(135deg, #8f73ff, #6544df);
    color: #fff;
    font-size: 10px;
    font-weight: 600;
    cursor: pointer;
    box-shadow: 0 4px 16px -9px rgba(123, 84, 255, 0.85);
  }

  .primary:hover:not(:disabled) {
    filter: brightness(1.08);
  }

  .secondary {
    height: 31px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 5px;
    padding: 0 11px;
    border-radius: 7px;
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text-muted);
    font-size: 9.5px;
    font-weight: 550;
    cursor: pointer;
    transition: 120ms ease;
  }

  .secondary:hover:not(:disabled) {
    border-color: var(--border-strong);
    background: var(--surface-3);
    color: var(--text);
  }

  .secondary.danger {
    color: #f19a9a;
    border-color: rgba(255, 107, 107, 0.16);
  }

  button:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .divider {
    height: 1px;
    margin: 20px 0;
    background: var(--border-soft);
  }

  .inline-card {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    padding: 12px;
    border-radius: 9px;
    border: 1px solid var(--border-soft);
    background: var(--surface-2);
  }

  .inline-card strong {
    display: block;
    color: var(--text-muted);
    font-size: 10px;
    font-weight: 600;
  }

  .inline-card p {
    margin: 3px 0 0;
    color: var(--text-faint);
    font-size: 9px;
    line-height: 1.45;
    max-width: 330px;
  }

  .secret-input {
    position: relative;
    display: flex;
    align-items: center;
  }

  .secret-input input {
    padding-right: 82px;
  }

  .saved-badge {
    position: absolute;
    right: 7px;
    display: inline-flex;
    align-items: center;
    gap: 3px;
    padding: 2px 5px;
    border-radius: 4px;
    background: rgba(64, 205, 154, 0.08);
    border: 1px solid rgba(64, 205, 154, 0.16);
    color: #67dbae;
    font-size: 8px;
    font-weight: 600;
  }

  .footnote {
    margin: 2px 0 0;
    color: var(--text-faint);
    font-size: 8.5px;
    line-height: 1.5;
  }

  .pane-title-row {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 14px;
  }

  .pane-title-row h3 {
    margin-bottom: 3px;
  }

  .pane-copy {
    margin: 0;
    color: var(--text-faint);
    font-size: 9px;
    line-height: 1.45;
    max-width: 350px;
  }

  .runtime-card {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 11px 12px;
    border-radius: 9px;
    border: 1px solid var(--border-soft);
    background: var(--surface-2);
    margin-bottom: 8px;
  }

  .runtime-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--danger);
    flex: none;
  }

  .runtime-dot.online {
    background: var(--success);
    box-shadow: 0 0 0 3px rgba(65, 210, 158, 0.1);
  }

  .runtime-card strong {
    display: block;
    color: var(--text-muted);
    font-size: 10px;
    font-weight: 600;
  }

  .runtime-card span {
    display: block;
    margin-top: 2px;
    color: var(--text-faint);
    font-size: 8.5px;
  }

  .plugin-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-top: 9px;
  }

  .plugin {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 8px 9px;
    border-radius: 8px;
    border: 1px solid var(--border-soft);
    background: var(--surface-2);
  }

  .plugin.missing {
    border-color: rgba(255, 100, 100, 0.16);
  }

  .plugin-icon {
    width: 22px;
    height: 22px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    background: rgba(255, 94, 94, 0.1);
    color: #ff8d8d;
    flex: none;
  }

  .plugin-icon.ok {
    background: rgba(57, 208, 150, 0.1);
    color: #56d9a4;
  }

  .plugin-copy {
    flex: 1;
    min-width: 0;
  }

  .plugin-copy strong {
    display: block;
    font-size: 9.5px;
    font-weight: 600;
    color: var(--text-muted);
  }

  .plugin-copy span {
    display: block;
    margin-top: 1px;
    font-size: 8px;
    color: var(--text-faint);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .plugin-state {
    font-size: 8px;
    font-weight: 600;
    color: #e58c8c;
  }

  .plugin-state.ok {
    color: #5fd9a9;
  }

  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 5px;
    min-height: 180px;
    text-align: center;
    color: var(--text-faint);
    border: 1px dashed var(--border);
    border-radius: 10px;
    margin-top: 12px;
  }

  .empty-state :global(svg) {
    color: #8f7bfa;
    margin-bottom: 4px;
  }

  .empty-state strong {
    color: var(--text-muted);
    font-size: 10px;
    font-weight: 600;
  }

  .empty-state p {
    margin: 0;
    max-width: 280px;
    font-size: 9px;
    line-height: 1.5;
  }

  .error-card,
  .warning-card {
    padding: 10px 11px;
    border-radius: 8px;
    font-size: 9px;
    line-height: 1.5;
    color: #f0a7a7;
    border: 1px solid rgba(255, 100, 100, 0.18);
    background: rgba(255, 80, 80, 0.06);
    margin-bottom: 8px;
  }

  .warning-card {
    margin-top: 8px;
    color: #e3bd7d;
    border-color: rgba(240, 183, 92, 0.16);
    background: rgba(240, 183, 92, 0.05);
  }

  .warning-card p {
    margin: 0;
  }

  .warning-card p + p {
    margin-top: 4px;
  }

  .modal-foot {
    flex: none;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    padding: 11px 20px;
    border-top: 1px solid var(--border-soft);
    color: var(--text-faint);
    font-size: 8.5px;
  }

  .modal-foot span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  @keyframes fade-in {
    from {
      opacity: 0;
    }
  }

  @keyframes rise-in {
    from {
      opacity: 0;
      transform: translateY(8px) scale(0.99);
    }
  }

  @media (max-width: 720px) {
    .tabs {
      width: 52px;
    }

    .tabs > button {
      justify-content: center;
      padding: 0;
    }

    .tabs > button :global(span),
    .tabs > button {
      font-size: 0;
    }

    .tabs > button :global(svg) {
      width: 17px;
      height: 17px;
    }

    .tab-foot {
      display: none;
    }
  }
</style>
