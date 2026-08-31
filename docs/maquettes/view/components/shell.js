(function (Torca) {
  'use strict';
  const C = Torca.components;

  function navItem(app, route, icon, label, badge) {
    const active = app.router.currentPath().startsWith(route);
    return `<a href="#${route}" class="nav-item ${active ? 'active' : ''}" data-route="${route}">${C.icon(icon)}<span>${Torca.util.escape(label)}</span>${badge ? C.badge(badge) : ''}</a>`;
  }

  C.shell = function shell(app, options) {
    const t = Torca.i18n.t.bind(Torca.i18n);
    const unread = app.store.unreadTotal();
    const pending = app.store.state.pairings.filter((p) => p.state === 'awaiting').length;
    const runtime = app.store.state.runtime;
    const activeTransfers = app.store.state.transfers.filter((item) => !['complete','cancelled'].includes(item.state)).length;
    const nav = `${navItem(app,'/chats','chats',t('chats'),unread||'')}${navItem(app,'/contacts','contacts',t('contacts'))}${navItem(app,'/invitations','invitations',t('invitations'),pending||'')}`;
    const extraActions = options.actions || '';
    const globalActions = `${extraActions}${C.networkMonitor(runtime, app.isCompact())}<span class="header-action-wrap">${C.iconButton('transfer',t('transferCenter'),'data-open-transfers')}${activeTransfers ? C.badge(activeTransfers,'floating') : ''}</span>${C.iconButton('more','Menu','data-open-app-menu')}`;
    const globalHeader = options.hideHeader ? '' : `<header class="app-header">${options.leading || ''}<div class="app-header__title"><h1>${Torca.util.escape(options.title || 'Torca')}</h1>${options.subtitle ? `<p>${Torca.util.escape(options.subtitle)}</p>` : ''}</div><div class="app-header__actions">${globalActions}</div></header>`;
    const bottomClass = options.hideBottomNav ? 'bottom-nav is-hidden' : 'bottom-nav';
    return `<div class="torca-app"><aside class="nav-rail"><button class="brand" type="button" data-build-info><span class="brand-mark">T</span><span>Torca</span></button><nav class="nav-list">${nav}</nav><div class="nav-spacer"></div><button class="runtime-footer" type="button" data-build-info><span data-runtime-footer>${C.runtimeFooter(runtime)}</span><small class="runtime-footer__build">${Torca.util.escape(app.store.state.profile.version)}+${app.store.state.profile.build}</small></button></aside><section class="main-frame ${options.hideHeader?'no-global-header':''}">${globalHeader}<div class="screen-host">${options.body || ''}</div></section><nav class="${bottomClass}">${nav}</nav></div>`;
  };

  C.bindShell = function bindShell(root, app) {
    root.querySelectorAll('[data-route]').forEach((node) => node.addEventListener('click', (event) => { event.preventDefault(); app.router.navigate(node.getAttribute('data-route')); }));
    root.querySelectorAll('[data-open-transfers]').forEach((node)=>node.addEventListener('click',()=>C.showTransferCenter(app)));
    root.querySelectorAll('[data-open-app-menu]').forEach((node)=>node.addEventListener('click',()=>C.showAppMenu(app)));
    root.querySelectorAll('[data-build-info]').forEach((node)=>node.addEventListener('click',()=>C.showBuildInfo(app)));
  };
}(window.Torca));
