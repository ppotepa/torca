(function (Torca) {
  'use strict';
  const icons = {
    chats:'<path d="M4 5.5h16v10H9l-5 4v-14Z"/><path d="M8 9h8M8 12h5"/>',
    contacts:'<path d="M16 20v-1.5a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4V20"/><circle cx="9" cy="7" r="4"/><path d="M17 11a3.5 3.5 0 0 0 0-7M22 20v-1.5a4 4 0 0 0-3-3.65"/>',
    invitations:'<rect x="3" y="3" width="7" height="7"/><rect x="14" y="3" width="7" height="7"/><rect x="3" y="14" width="7" height="7"/><path d="M14 14h3v3h-3zM18 18h3v3h-3zM18 14h3"/>',
    settings:'<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .34 1.88l.06.06-2.83 2.83-.06-.06A1.7 1.7 0 0 0 15 19.4a1.7 1.7 0 0 0-1 .6 1.7 1.7 0 0 0-.4 1.1V21h-4v-.1A1.7 1.7 0 0 0 8.6 19.4a1.7 1.7 0 0 0-1.88.34l-.06.06-2.83-2.83.06-.06A1.7 1.7 0 0 0 4.6 15a1.7 1.7 0 0 0-.6-1 1.7 1.7 0 0 0-1.1-.4H3v-4h.1A1.7 1.7 0 0 0 4.6 8.6a1.7 1.7 0 0 0-.34-1.88l-.06-.06 2.83-2.83.06.06A1.7 1.7 0 0 0 9 4.6a1.7 1.7 0 0 0 1-.6 1.7 1.7 0 0 0 .4-1.1V3h4v.1A1.7 1.7 0 0 0 15.4 4.6a1.7 1.7 0 0 0 1.88-.34l.06-.06 2.83 2.83-.06.06A1.7 1.7 0 0 0 19.4 9c.12.36.34.7.6 1 .28.3.67.48 1.1.5h.1v4h-.1a1.7 1.7 0 0 0-1.7.5Z"/>',
    diagnostics:'<path d="M4 19V9M10 19V5M16 19v-7M22 19V3"/>',
    search:'<circle cx="11" cy="11" r="7"/><path d="m20 20-4-4"/>',
    plus:'<path d="M12 5v14M5 12h14"/>',
    paperclip:'<path d="m21 11-8.5 8.5a6 6 0 0 1-8.5-8.5l9-9a4 4 0 0 1 5.66 5.66l-9 9a2 2 0 1 1-2.83-2.83L15 5.66"/>',
    send:'<path d="m22 2-7 20-4-9-9-4 20-7Z"/><path d="M22 2 11 13"/>',
    mic:'<rect x="9" y="3" width="6" height="11" rx="3"/><path d="M5 11a7 7 0 0 0 14 0M12 18v3M8 21h8"/>',
    info:'<circle cx="12" cy="12" r="9"/><path d="M12 11v6M12 7h.01"/>',
    back:'<path d="m15 18-6-6 6-6"/>',
    more:'<circle cx="5" cy="12" r="1"/><circle cx="12" cy="12" r="1"/><circle cx="19" cy="12" r="1"/>',
    pin:'<path d="m9 4 6 0 1 5 3 3H5l3-3 1-5Z"/><path d="M12 12v8"/>',
    muted:'<path d="M11 5 6 9H3v6h3l5 4V5Z"/><path d="m18 9 4 4M22 9l-4 4"/>',
    file:'<path d="M6 2h8l4 4v16H6z"/><path d="M14 2v5h5"/>',
    image:'<rect x="3" y="4" width="18" height="16" rx="2"/><circle cx="9" cy="10" r="2"/><path d="m21 15-5-5L5 20"/>',
    video:'<rect x="3" y="5" width="13" height="14" rx="2"/><path d="m16 10 5-3v10l-5-3"/>',
    check:'<path d="m5 12 4 4L19 6"/>',
    doubleCheck:'<path d="m2 12 4 4 8-9M10 16l3 3 9-10"/>',
    clock:'<circle cx="12" cy="12" r="9"/><path d="M12 7v6l4 2"/>',
    warning:'<path d="M12 3 2 21h20L12 3Z"/><path d="M12 9v5M12 18h.01"/>',
    wifi:'<path d="M3 9a14 14 0 0 1 18 0M6 13a9 9 0 0 1 12 0M9.5 16.5a4 4 0 0 1 5 0"/><circle cx="12" cy="20" r="1"/>',
    shield:'<path d="M12 3 4 6v5c0 5 3.4 8.6 8 10 4.6-1.4 8-5 8-10V6l-8-3Z"/><path d="m9 12 2 2 4-5"/>',
    copy:'<rect x="8" y="8" width="12" height="12" rx="2"/><path d="M16 8V5a1 1 0 0 0-1-1H5a1 1 0 0 0-1 1v10a1 1 0 0 0 1 1h3"/>',
    close:'<path d="M6 6l12 12M18 6 6 18"/>',
    chevronRight:'<path d="m9 18 6-6-6-6"/>',
    retry:'<path d="M20 7v5h-5M4 17v-5h5"/><path d="M6.1 8A7 7 0 0 1 18.7 6L20 12M4 12l1.3 6A7 7 0 0 0 17.9 16"/>',
    palette:'<path d="M12 3a9 9 0 1 0 0 18h1.5a1.5 1.5 0 0 0 0-3H12a2 2 0 0 1 0-4h4a5 5 0 0 0 5-5c0-3.3-4-6-9-6Z"/><circle cx="7" cy="10" r="1"/><circle cx="9" cy="6.5" r="1"/><circle cx="14" cy="6" r="1"/><circle cx="17" cy="9" r="1"/>'
  };
  Torca.components.icon = function icon(name, className) {
    return `<svg class="icon ${className || ''}" viewBox="0 0 24 24" aria-hidden="true">${icons[name] || icons.info}</svg>`;
  };
}(window.Torca));
