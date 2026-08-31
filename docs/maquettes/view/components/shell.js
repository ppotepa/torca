(function (Torca) {
  'use strict';
  const C = Torca.components;
  function navItem(app, route, icon, label, badge) {
    const active = app.router.currentPath().startsWith(route);
    return `<a href="#${route}" class="nav-item ${active ? 'active' : ''}" data-route="${route}">${C.icon(icon)}<span>${Torca.util.escape(label)}</span>${badge ? C.badge(badge) : ''}</a>`;
  }
  C.shell = function shell(app, options) {
    const t=Torca.i18n.t.bind(Torca.i18n); const unread=app.store.unreadTotal(); const pending=app.store.state.pairings.filter((p)=>p.state==='awaiting').length;
    const nav = `${navItem(app,'/chats','chats',t('chats'),unread||'')}${navItem(app,'/contacts','contacts',t('contacts'))}${navItem(app,'/invitations','invitations',t('invitations'),pending||'')}`;
    const lower = `${navItem(app,'/settings','settings',t('settings'))}${navItem(app,'/diagnostics','diagnostics',t('diagnostics'))}${navItem(app,'/lab','palette',t('uiLab'))}`;
    const runtime=app.store.state.runtime;
    const status = runtime.state==='ready' ? t('online') : runtime.state==='offline' ? t('offline') : t('reconnecting');
    const extraActions = options.actions || '';
    const settingsAction = app.router.currentPath().startsWith('/settings') ? '' : C.iconButton('settings',t('settings'),'data-route="/settings"');
    const actions = `${extraActions}${settingsAction}`;
    const globalHeader = options.hideHeader ? '' : `<header class="app-header"><div class="app-header__title"><h1>${Torca.util.escape(options.title || 'Torca')}</h1>${options.subtitle ? `<p>${Torca.util.escape(options.subtitle)}</p>` : ''}</div><div style="display:flex;align-items:center;gap:4px">${actions}</div></header>`;
    return `<div class="torca-app"><aside class="nav-rail"><div class="brand"><div class="brand-mark">T</div><span>Torca</span></div><nav class="nav-list">${nav}</nav><div class="nav-spacer"></div><nav class="nav-list">${lower}</nav><div class="brand" style="padding-bottom:0"><span class="status-dot ${runtime.state==='ready'?'online':''}"></span><span style="font-size:11px;color:var(--muted)">${Torca.util.escape(runtime.provider)} · ${Torca.util.escape(status)}</span></div></aside><section class="main-frame ${options.hideHeader?'no-global-header':''}">${globalHeader}<div class="screen-host">${options.body || ''}</div></section><nav class="bottom-nav">${nav}</nav></div>`;
  };
  C.bindShell = function bindShell(root, app) {
    root.querySelectorAll('[data-route]').forEach((node) => node.addEventListener('click', (event) => { event.preventDefault(); app.router.navigate(node.getAttribute('data-route')); }));
  };
}(window.Torca));
