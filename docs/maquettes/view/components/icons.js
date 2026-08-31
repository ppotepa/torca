(function (Torca) {
  'use strict';
  const P = {
    check:'<path d="m5 12 4 4L19 6"/>',
    doubleCheck:'<path d="m2 12 4 4 8-9M10 16l3 3 9-10"/>',
    close:'<path d="M6 6l12 12M18 6 6 18"/>',
    back:'<path d="m15 18-6-6 6-6"/>',
    clock:'<circle cx="12" cy="12" r="9"/><path d="M12 7v6l4 2"/>',
    warning:'<path d="M12 3 2 21h20L12 3Z"/><path d="M12 9v5M12 18h.01"/>',
    file:'<path d="M6 2h8l4 4v16H6z"/><path d="M14 2v5h5"/>',
    link:'<path d="M10 13a5 5 0 0 0 7.1.1l2-2a5 5 0 0 0-7.1-7.1l-1.1 1.1"/><path d="M14 11a5 5 0 0 0-7.1-.1l-2 2A5 5 0 0 0 12 20l1.1-1.1"/>',
    shield:'<path d="M12 3 4 6v5c0 5 3.4 8.6 8 10 4.6-1.4 8-5 8-10V6l-8-3Z"/>'
  };
  const modern = {
    chats:'<path d="M4 5.5h16v10H9l-5 4v-14Z"/><path d="M8 9h8M8 12h5"/>',
    contacts:'<path d="M16 20v-1.5a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4V20"/><circle cx="9" cy="7" r="4"/><path d="M17 11a3.5 3.5 0 0 0 0-7M22 20v-1.5a4 4 0 0 0-3-3.65"/>',
    invitations:'<rect x="3" y="3" width="7" height="7"/><rect x="14" y="3" width="7" height="7"/><rect x="3" y="14" width="7" height="7"/><path d="M14 14h3v3h-3zM18 18h3v3h-3zM18 14h3"/>',
    addContact:'<circle cx="9" cy="8" r="4"/><path d="M3 21v-2a6 6 0 0 1 12 0v2M19 8v6M16 11h6"/>',
    contactInfo:'<circle cx="12" cy="12" r="9"/><path d="M12 11v6M12 7h.01"/>',
    send:'<path d="m22 2-7 20-4-9-9-4 20-7Z"/><path d="M22 2 11 13"/>',
    attachment:'<path d="m21 11-8.5 8.5a6 6 0 0 1-8.5-8.5l9-9a4 4 0 0 1 5.66 5.66l-9 9a2 2 0 1 1-2.83-2.83L15 5.66"/>',
    settings:'<circle cx="12" cy="12" r="3"/><path d="M19 12a7 7 0 0 0-.12-1.3l2-1.55-2-3.46-2.48 1a7 7 0 0 0-2.25-1.3L13.8 3h-4l-.35 2.39a7 7 0 0 0-2.25 1.3l-2.48-1-2 3.46 2 1.55A7 7 0 0 0 4.6 12c0 .44.04.87.12 1.3l-2 1.55 2 3.46 2.48-1a7 7 0 0 0 2.25 1.3L9.8 21h4l.35-2.39a7 7 0 0 0 2.25-1.3l2.48 1 2-3.46-2-1.55c.08-.43.12-.86.12-1.3Z"/>',
    search:'<circle cx="11" cy="11" r="7"/><path d="m20 20-4-4"/>',
    close:P.close, confirm:P.check, back:P.back,
    expand:'<path d="m6 9 6 6 6-6"/>', collapse:'<path d="m6 15 6-6 6 6"/>',
    more:'<circle cx="5" cy="12" r="1"/><circle cx="12" cy="12" r="1"/><circle cx="19" cy="12" r="1"/>',
    reply:'<path d="m9 17-5-5 5-5M4 12h9a7 7 0 0 1 7 7"/>', forward:'<path d="m15 17 5-5-5-5M20 12h-9a7 7 0 0 0-7 7"/>',
    copy:'<rect x="8" y="8" width="12" height="12" rx="2"/><path d="M16 8V5a1 1 0 0 0-1-1H5a1 1 0 0 0-1 1v10a1 1 0 0 0 1 1h3"/>',
    retry:'<path d="M20 7v5h-5M4 17v-5h5"/><path d="M6.1 8A7 7 0 0 1 18.7 6L20 12M4 12l1.3 6A7 7 0 0 0 17.9 16"/>', reconnect:'<path d="M20 7v5h-5M4 17v-5h5"/><path d="M6.1 8A7 7 0 0 1 18.7 6L20 12M4 12l1.3 6A7 7 0 0 0 17.9 16"/>',
    download:'<path d="M12 3v12M7 10l5 5 5-5M4 21h16"/>', jumpToLatest:'<path d="M12 4v14M6 12l6 6 6-6"/>',
    remove:'<path d="M4 7h16M9 7V4h6v3M7 7l1 14h8l1-14"/>', edit:'<path d="m4 20 4-1 11-11-3-3L5 16l-1 4ZM14 6l3 3"/>', block:'<circle cx="12" cy="12" r="9"/><path d="m6 6 12 12"/>',
    success:P.check, warning:P.warning, error:'<circle cx="12" cy="12" r="9"/><path d="M12 7v7M12 18h.01"/>',
    online:'<path d="M3 9a14 14 0 0 1 18 0M6 13a9 9 0 0 1 12 0M9.5 16.5a4 4 0 0 1 5 0"/><circle cx="12" cy="20" r="1"/>',
    instant:'<path d="m13 2-8 12h7l-1 8 8-12h-7l1-8Z"/>',
    radio:'<path d="M7 19h10M9 19l1-10h4l1 10M12 5V2"/><path d="M5 7a10 10 0 0 1 14 0M8 10a6 6 0 0 1 8 0"/>',
    pushToTalk:'<rect x="9" y="3" width="6" height="11" rx="3"/><path d="M5 11a7 7 0 0 0 14 0M12 18v3"/>',
    play:'<path d="m8 5 11 7-11 7V5Z"/>', pause:'<path d="M7 5h3v14H7zM14 5h3v14h-3z"/>',
    file:P.file, image:'<rect x="3" y="4" width="18" height="16" rx="2"/><circle cx="9" cy="10" r="2"/><path d="m21 15-5-5L5 20"/>', video:'<rect x="3" y="5" width="13" height="14" rx="2"/><path d="m16 10 5-3v10l-5-3"/>', audio:'<path d="M11 5 6 9H3v6h3l5 4V5Z"/><path d="M15 9a4 4 0 0 1 0 6M18 6a8 8 0 0 1 0 12"/>',
    pdf:'<path d="M6 2h9l4 4v16H6z"/><path d="M9 16v-5h2a2 2 0 0 1 0 4H9M14 11h3M14 14h3"/>', document:P.file, archive:'<path d="M4 4h16v5H4zM6 9h12v11H6zM10 13h4"/>', textFile:'<path d="M6 2h9l4 4v16H6zM9 11h6M9 15h6"/>',
    info:'<circle cx="12" cy="12" r="9"/><path d="M12 11v6M12 7h.01"/>',
    identity:`${P.shield}<path d="m9 12 2 2 4-5"/>`, diagnostics:'<path d="M4 19V9M10 19V5M16 19v-7M22 19V3"/>',
    notifications:'<path d="M6 9a6 6 0 0 1 12 0v5l2 3H4l2-3V9ZM10 21h4"/>', appearance:'<path d="M12 3a9 9 0 1 0 0 18h1.5a1.5 1.5 0 0 0 0-3H12a2 2 0 0 1 0-4h4a5 5 0 0 0 5-5c0-3.3-4-6-9-6Z"/>', language:'<circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3c3 3 3 15 0 18M12 3c-3 3-3 15 0 18"/>', emoji:'<circle cx="12" cy="12" r="9"/><circle cx="9" cy="10" r="1"/><circle cx="15" cy="10" r="1"/><path d="M8 15c2 2 6 2 8 0"/>',
    open:'<path d="M14 4h6v6M20 4l-9 9"/><path d="M18 13v7H4V6h7"/>', save:'<path d="M5 3h12l2 2v16H5zM8 3v6h8V3M8 17h8"/>', scan:'<path d="M4 9V4h5M15 4h5v5M20 15v5h-5M9 20H4v-5"/>', link:P.link,
    queued:P.clock, sending:'<path d="M12 3v3M12 18v3M3 12h3M18 12h3M5.6 5.6l2.1 2.1M16.3 16.3l2.1 2.1M18.4 5.6l-2.1 2.1M7.7 16.3l-2.1 2.1"/>', sent:P.check, delivered:P.doubleCheck, read:'<path d="M2 12s4-6 10-6 10 6 10 6-4 6-10 6S2 12 2 12Z"/><circle cx="12" cy="12" r="2"/>', cancelled:'<circle cx="12" cy="12" r="9"/><path d="m8 8 8 8M16 8l-8 8"/>',
    plus:'<path d="M12 5v14M5 12h14"/>', mic:'<rect x="9" y="3" width="6" height="11" rx="3"/><path d="M5 11a7 7 0 0 0 14 0M12 18v3M8 21h8"/>', check:P.check, doubleCheck:P.doubleCheck, clock:P.clock,
    transfer:'<path d="M7 3v14M3 13l4 4 4-4M17 21V7M13 11l4-4 4 4"/>', logs:'<rect x="3" y="4" width="18" height="16" rx="2"/><path d="m7 9 3 3-3 3M12 15h5"/>', battery:'<rect x="3" y="7" width="17" height="10" rx="2"/><path d="M22 10v4M6 10h7"/>', incident:P.warning, palette:'<path d="M12 3a9 9 0 1 0 0 18h1.5a1.5 1.5 0 0 0 0-3H12a2 2 0 0 1 0-4h4a5 5 0 0 0 5-5c0-3.3-4-6-9-6Z"/>', about:'<circle cx="12" cy="12" r="9"/><path d="M12 11v6M12 7h.01"/>'
  };
  const terminalOverrides = {
    chats:'<path d="M3 5h18v12H9l-4 4v-4H3V5Z"/><path d="M7 9h10M7 13h7"/>', contacts:'<path d="M4 20v-5h10v5H4ZM6 4h6v7H6V4ZM16 6h4v6h-4M15 15h6v5"/>', settings:'<path d="M5 5h14M8 5v5M5 12h14M16 12v5M5 19h14M11 19v-5"/>',
    send:'<path d="M3 4h18v16H3V4Z"/><path d="m8 8 6 4-6 4V8ZM15 8h2v8h-2"/>', instant:'<path d="M13 2H8l-3 10h5l-1 10 10-13h-6V2Z"/>', radio:'<path d="M5 19h14M9 19l2-11h2l2 11M12 8V3M7 6h2M15 6h2"/>', diagnostics:'<path d="M3 20h18V4H3v16ZM6 16v-5h2v5H6Zm5 0V7h2v9h-2Zm5 0v-3h2v3h-2Z"/>', logs:'<path d="M3 4h18v16H3V4ZM6 8l3 3-3 3M11 15h6"/>', identity:'<path d="M12 2 4 5v7c0 5 3 8 8 10 5-2 8-5 8-10V5l-8-3ZM8 12l3 3 5-6"/>', more:'<path d="M4 10h4v4H4zM10 10h4v4h-4zM16 10h4v4h-4z"/>', invitations:'<path d="M3 3h7v7H3V3Zm11 0h7v7h-7V3ZM3 14h7v7H3v-7Zm11 0h3v3h-3v-3Zm4 4h3v3h-3v-3Z"/>', notifications:'<path d="M6 17h12l-2-3V8H8v6l-2 3ZM10 20h4"/>'
  };
  Torca.components.icon = function icon(name, className) {
    const terminal = (document.body.dataset.theme || '').startsWith('terminal-');
    const paths = terminal ? { ...modern, ...terminalOverrides } : modern;
    return `<svg class="icon ${terminal ? 'icon--terminal' : ''} ${className || ''}" viewBox="0 0 24 24" aria-hidden="true">${paths[name] || paths.info}</svg>`;
  };
  Torca.components.iconNames = Object.keys(modern).sort();
}(window.Torca));
