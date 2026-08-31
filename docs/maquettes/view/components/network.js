(function (Torca) {
  'use strict';
  const C = Torca.components;
  const e = Torca.util.escape;

  function linkTone(state) {
    if (state === 'ready') return 'good';
    if (state === 'starting' || state === 'checking' || state === 'connecting' || state === 'reconnecting') return 'warn';
    if (state === 'failed') return 'bad';
    return 'idle';
  }

  C.transportLight = function transportLight(label, icon, indicator, compact) {
    const latency = indicator.latency == null ? '' : ` · ${indicator.latency} ms`;
    const pressure = indicator.queued || indicator.inFlight ? ` · ${indicator.inFlight || 0} active / ${indicator.queued || 0} queued` : '';
    const title = `${label}: ${indicator.state}${latency}${pressure} · ${indicator.code || ''}`;
    return `<div class="transport-light ${compact ? 'compact' : ''}" title="${e(title)}">${C.icon(icon, 'sm')}<span class="transport-light__label">${e(label)}</span><span class="ether-led link ${linkTone(indicator.state)} ${indicator.state === 'starting' || indicator.state === 'reconnecting' ? 'pulse' : ''}" aria-label="LINK ${e(indicator.state)}"></span><span class="ether-led tx ${indicator.txActive ? 'active' : ''}" aria-label="TX"></span><span class="ether-led rx ${indicator.rxActive ? 'active' : ''}" aria-label="RX"></span></div>`;
  };

  C.networkMonitor = function networkMonitor(runtime, compact) {
    const provider = (runtime.provider || 'provider').toUpperCase();
    return `<div class="network-monitor ${compact ? 'compact' : ''}" data-network-monitor data-compact="${compact ? '1' : '0'}">${C.transportLight(provider, 'link', runtime.communication, compact)}${C.transportLight('P2P', 'online', runtime.peer, compact)}</div>`;
  };

  C.runtimeFooter = function runtimeFooter(runtime) {
    const t = Torca.i18n.t.bind(Torca.i18n);
    const status = runtime.state === 'ready' ? t('online') : runtime.state === 'offline' ? t('offline') : t('reconnecting');
    return `<span class="footer-link ${linkTone(runtime.state)}"></span><span><strong>${e(runtime.provider.toUpperCase())}</strong><small>${e(status)} · ${e(runtime.path || '—')}</small></span>`;
  };
}(window.Torca));
