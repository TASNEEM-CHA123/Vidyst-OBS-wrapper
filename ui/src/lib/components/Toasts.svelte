<script lang="ts">
  import Icon from './Icon.svelte'
  import type { Toast } from '../types'

  let { toasts, onDismiss }: { toasts: Toast[]; onDismiss: (id: number) => void } = $props()
</script>

<div class="toasts" aria-live="polite">
  {#each toasts as toast (toast.id)}
    <div class="toast" class:error={toast.kind === 'error'} class:success={toast.kind === 'success'}>
      <span class="toast-icon">
        <Icon
          name={toast.kind === 'error' ? 'alert' : toast.kind === 'success' ? 'check' : 'sparkles'}
          size={14}
        />
      </span>
      <span class="toast-message">{toast.message}</span>
      <button type="button" class="dismiss" onclick={() => onDismiss(toast.id)} aria-label="Dismiss">
        <Icon name="x" size={12} />
      </button>
    </div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    right: 18px;
    bottom: 18px;
    z-index: 200;
    display: flex;
    flex-direction: column;
    gap: 7px;
    width: min(340px, calc(100vw - 36px));
    pointer-events: none;
  }

  .toast {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    padding: 9px 9px 9px 10px;
    border-radius: 9px;
    border: 1px solid var(--border);
    background: color-mix(in srgb, var(--surface-2) 94%, transparent);
    box-shadow: 0 16px 40px -18px rgba(0, 0, 0, 0.9);
    backdrop-filter: blur(12px);
    pointer-events: auto;
    animation: slide-in 180ms cubic-bezier(0.22, 1, 0.36, 1);
  }

  .toast.error {
    border-color: rgba(255, 100, 100, 0.22);
    background: color-mix(in srgb, #2a1519 92%, transparent);
  }

  .toast.success {
    border-color: rgba(65, 210, 158, 0.2);
  }

  .toast-icon {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: 6px;
    flex: none;
    color: #a99cff;
    background: rgba(137, 108, 255, 0.1);
  }

  .toast.error .toast-icon {
    color: #ff9191;
    background: rgba(255, 94, 94, 0.1);
  }

  .toast.success .toast-icon {
    color: #63d8ab;
    background: rgba(64, 205, 154, 0.1);
  }

  .toast-message {
    flex: 1;
    padding-top: 3px;
    color: var(--text-muted);
    font-size: 9.5px;
    line-height: 1.45;
    word-break: break-word;
  }

  .dismiss {
    width: 20px;
    height: 20px;
    display: grid;
    place-items: center;
    border: 0;
    border-radius: 5px;
    background: transparent;
    color: var(--text-faint);
    cursor: pointer;
    flex: none;
  }

  .dismiss:hover {
    color: var(--text);
    background: var(--surface-3);
  }

  @keyframes slide-in {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
  }
</style>
