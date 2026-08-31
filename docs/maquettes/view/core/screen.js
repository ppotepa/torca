(function (Torca) {
  'use strict';
  class Screen {
    constructor(app, params) { this.app = app; this.params = params || {}; this.root = null; }
    mount(root) { this.root = root; root.innerHTML = this.render(); this.bind(); }
    render() { return ''; }
    bind() {}
    unmount() { this.root = null; }
    q(selector) { return this.root ? this.root.querySelector(selector) : null; }
    qa(selector) { return this.root ? Array.from(this.root.querySelectorAll(selector)) : []; }
    on(selector, event, handler) { this.qa(selector).forEach((node) => node.addEventListener(event, handler)); }
  }
  Torca.core.Screen = Screen;
}(window.Torca));
