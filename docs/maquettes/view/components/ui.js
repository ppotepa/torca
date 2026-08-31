(function (Torca) {
  'use strict';
  const { escape, initials } = Torca.util;
  const C = Torca.components;

  C.avatar = function avatar(contact, size) {
    const name = contact && contact.name ? contact.name : '?';
    const online = contact && contact.online;
    const classes = ['avatar', size || '', contact && contact.identityChanged ? 'identity-warning' : ''].filter(Boolean).join(' ');
    return `<div class="${classes}" title="${escape(name)}">${escape((contact && contact.initials) || initials(name))}${online ? '<span class="status-dot online"></span>' : ''}</div>`;
  };

  C.badge = (value, tone) => `<span class="badge ${tone || ''}">${escape(value)}</span>`;
  C.iconButton = (icon, title, attrs, className) => `<button class="icon-button ${className || ''}" type="button" title="${escape(title || '')}" aria-label="${escape(title || '')}" ${attrs || ''}>${C.icon(icon)}</button>`;
  C.button = (label, icon, attrs, style) => `<button class="button ${style || ''}" type="button" ${attrs || ''}>${icon ? C.icon(icon, 'sm') : ''}<span>${escape(label)}</span></button>`;
  C.switchControl = (value, attrs, label) => `<button type="button" class="switch ${value ? 'on' : ''}" role="switch" aria-checked="${value ? 'true' : 'false'}" aria-label="${escape(label || '')}" ${attrs || ''}></button>`;
  C.progress = (value) => `<div class="progress"><span style="width:${Math.max(0, Math.min(100, Number(value) || 0))}%"></span></div>`;
  C.emptyState = (icon, title, message, action) => `<div class="empty-state"><div class="empty-state__inner"><div class="empty-state__icon">${C.icon(icon, 'lg')}</div><h3>${escape(title)}</h3><p>${escape(message)}</p>${action || ''}</div></div>`;
  C.detailRow = (label, value) => `<div class="detail-row"><span>${escape(label)}</span><strong>${escape(value)}</strong></div>`;

  C.toast = function toast(message) {
    const root = document.getElementById('toast-root'); if (!root) return;
    const node = document.createElement('div'); node.className='toast'; node.textContent=message; root.appendChild(node);
    window.setTimeout(() => node.remove(), 2800);
  };

  C.qr = function qr() {
    const pattern = [0,1,2,3,5,6,7,8,9,15,17,18,19,21,24,25,26,27,29,30,32,34,36,37,38,39,40,41,43,45,47,48,49,51,53,55,57,59,61,63,64,65,66,69,71,72,73,74,75,77,78,79,80];
    return `<div class="qr-placeholder" aria-label="QR mock">${Array.from({length:81},(_,i)=>`<span style="background:${pattern.includes(i)?'#111':'#fff'}"></span>`).join('')}</div>`;
  };

  C.modal = function modal(options) {
    const root = document.getElementById('modal-root'); if (!root) return;
    root.innerHTML = `<div class="modal-backdrop" data-modal-backdrop><section class="modal ${options.className || ''}" role="dialog" aria-modal="true"><header class="modal__header"><strong>${escape(options.title || '')}</strong>${C.iconButton('close', Torca.i18n.t('close'), 'data-close-modal')}</header><div class="modal__body">${options.body || ''}</div>${options.actions ? `<footer class="modal__actions">${options.actions}</footer>` : ''}</section></div>`;
    const close = () => { root.innerHTML=''; if (options.onClose) options.onClose(); };
    root.querySelector('[data-close-modal]').addEventListener('click', close);
    root.querySelector('[data-modal-backdrop]').addEventListener('click', (event) => { if (event.target === event.currentTarget) close(); });
    if (options.bind) options.bind(root, close);
  };

  C.contactStatus = function contactStatus(contact) {
    if (contact.blocked) return Torca.i18n.t('blocked');
    return contact.online ? Torca.i18n.t('online') : `${Torca.i18n.t('lastSeen')} ${Torca.util.formatRelative(contact.lastSeen)}`;
  };

  C.showTransferCenter = function showTransferCenter(app) {
    const t = Torca.i18n.t.bind(Torca.i18n);
    const transfers = app.store.state.transfers;
    const rows = transfers.map((item) => `<div class="transfer-row"><div class="transfer-row__icon">${C.icon('transfer')}</div><div class="transfer-row__body"><strong>${escape(item.name)}</strong><small>${escape(item.direction === 'in' ? 'RX' : 'TX')} · ${escape(item.size || '')} · ${escape(t(item.state) || item.state)}</small>${item.progress < 100 ? C.progress(item.progress) : ''}</div><span class="transfer-row__value">${item.progress >= 100 ? C.icon('check', 'sm') : `${Number(item.progress) || 0}%`}</span></div>`).join('');
    C.modal({
      title: t('transferCenter'),
      className:'modal--wide',
      body: transfers.length ? `<div class="transfer-list">${rows}</div>` : C.emptyState('transfer', t('noTransfers'), ''),
      actions: C.button(t('close'), 'close', 'data-close-transfer', 'outline'),
      bind(root, close) { root.querySelector('[data-close-transfer]').addEventListener('click', close); }
    });
  };

  C.showBuildInfo = function showBuildInfo(app) {
    const t = Torca.i18n.t.bind(Torca.i18n);
    const p = app.store.state.profile;
    const r = app.store.state.runtime;
    C.modal({
      title:t('buildInfo'),
      body:`<div class="detail-list">${C.detailRow(t('productVersion'), `${p.version}+${p.build}`)}${C.detailRow(t('build'), r.build)}${C.detailRow(t('sourceCommit'), p.sourceCommit)}${C.detailRow(t('contract'), `${p.contract} / wire ${p.wire}`)}${C.detailRow(t('storageEpoch'), String(p.storageEpoch))}${C.detailRow(t('provider'), r.provider.toUpperCase())}${C.detailRow(t('providerProfile'), r.providerProfile)}${C.detailRow(t('endpoint'), r.endpoint)}</div>`
    });
  };

  C.showAppMenu = function showAppMenu(app) {
    const t = Torca.i18n.t.bind(Torca.i18n);
    const entries = [
      ['/invitations','plus',t('newPairing')],
      ['/identity','identity',t('identity')],
      ['/diagnostics','diagnostics',t('diagnostics')],
      ['/settings','settings',t('settings')],
      ['/about','about',t('about')]
    ];
    C.modal({
      title:'Torca',
      className:'modal--menu',
      body:`<div class="menu-list">${entries.map(([route,icon,label])=>`<button type="button" class="menu-row" data-menu-route="${route}">${C.icon(icon)}<span>${escape(label)}</span><span class="icon-turn">${C.icon('back','sm')}</span></button>`).join('')}</div>`,
      bind(root, close) {
        root.querySelectorAll('[data-menu-route]').forEach((node)=>node.addEventListener('click',()=>{const route=node.dataset.menuRoute;close();app.router.navigate(route);}));
      }
    });
  };
}(window.Torca));
