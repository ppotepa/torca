(function (Torca) {
  'use strict';
  class Router {
    constructor(app) { this.app = app; this.routes = []; this.onHashChange = () => this.resolve(); }
    register(pattern, factory) { this.routes.push({ pattern, factory }); return this; }
    start() { window.addEventListener('hashchange', this.onHashChange); if (!location.hash) location.hash = '#/chats'; else this.resolve(); }
    stop() { window.removeEventListener('hashchange', this.onHashChange); }
    navigate(path) { location.hash = `#${path.startsWith('/') ? path : `/${path}`}`; }
    currentPath() { return (location.hash || '#/chats').slice(1).split('?')[0] || '/chats'; }
    refresh() { if (this.app.currentScreen) this.app.currentScreen.mount(this.app.root); else this.resolve(); }
    resolve() {
      const path = this.currentPath();
      const view = path==='/chats'?'chats':path.startsWith('/chat/')?'chat':(path==='/contacts'||path.startsWith('/contact/')||path.startsWith('/connection/'))?'contacts':path==='/invitations'?'invitations':path==='/settings'?'settings':path==='/diagnostics'?'diagnostics':path==='/lab'?'lab':path==='/bootstrap'?'bootstrap':path==='/profile'?'profile':path==='/identity'?'identity':path==='/about'?'about':'chats';
      if (this.app.store.state.ui.view !== view) this.app.store.state.ui.view = view;
      if (this.app.applyUi) this.app.applyUi();
      if (this.app.renderDevbar) { this.app.renderDevbar(); this.app.bindDevbar(); }
      for (const route of this.routes) {
        const params = this.match(route.pattern, path);
        if (params) { this.app.mountScreen(route.factory(params)); return; }
      }
      this.navigate('/chats');
    }
    match(pattern, path) {
      const p = pattern.split('/').filter(Boolean); const a = path.split('/').filter(Boolean);
      if (p.length !== a.length) return null;
      const params = {};
      for (let i = 0; i < p.length; i += 1) {
        if (p[i].startsWith(':')) params[p[i].slice(1)] = decodeURIComponent(a[i]);
        else if (p[i] !== a[i]) return null;
      }
      return params;
    }
  }
  Torca.core.Router = Router;
}(window.Torca));
