(function () {
  'use strict';
  const Torca = window.Torca = window.Torca || {};
  Torca.core = Torca.core || {};
  Torca.components = Torca.components || {};
  Torca.screens = Torca.screens || {};
  Torca.fixtures = Torca.fixtures || {};
  Torca.util = {
    clone(value) { return JSON.parse(JSON.stringify(value)); },
    escape(value) {
      return String(value == null ? '' : value)
        .replaceAll('&', '&amp;')
        .replaceAll('<', '&lt;')
        .replaceAll('>', '&gt;')
        .replaceAll('"', '&quot;')
        .replaceAll("'", '&#039;');
    },
    initials(name) {
      return String(name || '?').trim().split(/\s+/).slice(0, 2).map((part) => part[0] || '').join('').toUpperCase() || '?';
    },
    formatTime(ms) {
      const d = new Date(ms);
      return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`;
    },
    formatRelative(ms) {
      const delta = Date.now() - ms;
      if (delta < 60_000) return 'now';
      if (delta < 3_600_000) return `${Math.max(1, Math.round(delta / 60_000))}m`;
      if (delta < 86_400_000) return this.formatTime(ms);
      return new Intl.DateTimeFormat(undefined, { month: 'short', day: 'numeric' }).format(new Date(ms));
    },
    id(prefix) { return `${prefix}-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 7)}`; }
  };
}());
