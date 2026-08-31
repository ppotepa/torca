(function (Torca) {
  'use strict';
  const { escape, initials } = Torca.util;
  const C = Torca.components;
  C.avatar = function avatar(contact, size) {
    const name = contact && contact.name ? contact.name : '?';
    const online = contact && contact.online;
    return `<div class="avatar ${size || ''}" title="${escape(name)}">${escape((contact && contact.initials) || initials(name))}${online ? '<span class="status-dot online"></span>' : ''}</div>`;
  };
  C.badge = (value) => `<span class="badge">${escape(value)}</span>`;
  C.iconButton = (icon, title, attrs, className) => `<button class="icon-button ${className || ''}" type="button" title="${escape(title || '')}" aria-label="${escape(title || '')}" ${attrs || ''}>${C.icon(icon)}</button>`;
  C.button = (label, icon, attrs, style) => `<button class="button ${style || ''}" type="button" ${attrs || ''}>${icon ? C.icon(icon, 'sm') : ''}<span>${escape(label)}</span></button>`;
  C.emptyState = (icon, title, message, action) => `<div class="empty-state"><div class="empty-state__inner"><div class="empty-state__icon">${C.icon(icon, 'lg')}</div><h3>${escape(title)}</h3><p>${escape(message)}</p>${action || ''}</div></div>`;
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
    root.innerHTML = `<div class="modal-backdrop" data-modal-backdrop><section class="modal" role="dialog" aria-modal="true"><header class="modal__header"><strong>${escape(options.title || '')}</strong>${C.iconButton('close', Torca.i18n.t('close'), 'data-close-modal')}</header><div class="modal__body">${options.body || ''}</div>${options.actions ? `<footer class="modal__actions">${options.actions}</footer>` : ''}</section></div>`;
    const close = () => { root.innerHTML=''; if (options.onClose) options.onClose(); };
    root.querySelector('[data-close-modal]').addEventListener('click', close);
    root.querySelector('[data-modal-backdrop]').addEventListener('click', (event) => { if (event.target === event.currentTarget) close(); });
    if (options.bind) options.bind(root, close);
  };
  C.contactStatus = function contactStatus(contact) {
    if (contact.blocked) return Torca.i18n.t('blocked');
    return contact.online ? Torca.i18n.t('online') : `${Torca.i18n.t('lastSeen')} ${Torca.util.formatRelative(contact.lastSeen)}`;
  };
}(window.Torca));
