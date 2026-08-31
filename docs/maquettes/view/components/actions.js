(function (Torca) {
  'use strict';
  const C = Torca.components;
  const e = Torca.util.escape;

  const labels = {
    message: { reply:'reply', react:'react', forward:'forward', edit:'edit', copy:'copy', details:'messageDetails', delete:'delete' },
    conversation: { mute:'mute', pin:'pin', markRead:'markRead', archive:'archive', delete:'delete' },
    contact: { message:'message', connection:'connectionDetails', rename:'rename', block:'block', remove:'remove' }
  };
  const icons = { reply:'reply', react:'emoji', forward:'forward', edit:'edit', copy:'copy', details:'info', delete:'remove', mute:'muted', pin:'pin', markRead:'read', archive:'archive', message:'send', connection:'link', rename:'edit', block:'block', remove:'remove' };

  C.actionModel = function actionModel(kind, context) {
    const name = Torca.i18n.t.bind(Torca.i18n); const base = labels[kind] || {}; const model={...base};
    if(kind==='conversation'&&context){model.mute=context.muted?'unmute':'mute';model.pin=context.pinned?'unpin':'pin';}
    if(kind==='contact'&&context?.blocked)model.block='unblock';
    return Object.keys(model).map((id) => ({ id, label:name(model[id]), icon:icons[id]||'info', tone:['delete','remove','block'].includes(id)?'danger':'', disabled:kind==='conversation'&&id==='markRead'&&!context?.unread }))
      .filter((item) => !(kind==='message'&&item.id==='edit'&&(context?.direction!=='out'||context?.deleted)))
      .filter((item) => !(kind==='message'&&context?.deleted&&['reply','react','forward','copy'].includes(item.id)));
  };

  C.showActionMenu = function showActionMenu(app, options) {
    const compact = app.isCompact();
    const actions = options.actions || C.actionModel(options.kind, options.context);
    C.modal({
      title: options.title || 'Actions',
      className: `${compact ? 'modal--sheet' : 'modal--menu'} action-menu`,
      body: `<div class="menu-list" role="menu">${actions.map((item) => `<button type="button" role="menuitem" class="menu-row ${item.tone || ''}" data-action-id="${e(item.id)}" ${item.disabled ? 'disabled' : ''}>${C.icon(item.icon)}<span>${e(item.label)}</span><span class="icon-turn">${C.icon('back','sm')}</span></button>`).join('')}</div>`,
      bind(root, close) {
        root.querySelectorAll('[data-action-id]').forEach((node) => node.addEventListener('click', () => { const id = node.dataset.actionId; close(); if (options.onAction) options.onAction(id, options.context); }));
      }
    });
  };

  C.showConfirm = function showConfirm(options) {
    const t = Torca.i18n.t.bind(Torca.i18n);
    C.modal({ title: options.title || 'Confirm', body: `<p class="modal-copy">${e(options.message || '')}</p>`, actions: `${C.button(options.cancelLabel || t('cancel'), 'close', 'data-confirm-cancel', 'outline')}${C.button(options.confirmLabel || t('confirm'), options.icon || 'check', 'data-confirm-ok', options.tone || 'danger')}`, bind(root, close) { root.querySelector('[data-confirm-cancel]').addEventListener('click', close); root.querySelector('[data-confirm-ok]').addEventListener('click', () => { close(); if (options.onConfirm) options.onConfirm(); }); }
    });
  };

  C.showPrompt = function showPrompt(options) {
    const t = Torca.i18n.t.bind(Torca.i18n);
    C.modal({ title: options.title || t('edit'), body: `<label class="field-label">${e(options.label || '')}<input class="text-input" data-prompt-value maxlength="200" value="${e(options.value || '')}"></label><p class="field-error" data-prompt-error></p>`, actions: `${C.button(t('cancel'), 'close', 'data-prompt-cancel', 'outline')}${C.button(options.confirmLabel || t('save'), 'check', 'data-prompt-ok', 'primary')}`, bind(root, close) { const input = root.querySelector('[data-prompt-value]'); const error = root.querySelector('[data-prompt-error]'); input.focus(); root.querySelector('[data-prompt-cancel]').addEventListener('click', close); root.querySelector('[data-prompt-ok]').addEventListener('click', () => { const value = input.value.trim(); if (!value) { error.textContent = options.requiredMessage || 'Required'; input.focus(); return; } close(); if (options.onConfirm) options.onConfirm(value); }); input.addEventListener('keydown', (event) => { if (event.key === 'Enter') root.querySelector('[data-prompt-ok]').click(); }); }
    });
  };

  C.copyText = function copyText(value, message) {
    if (navigator.clipboard && navigator.clipboard.writeText) navigator.clipboard.writeText(String(value)).catch(() => {});
    C.toast(message || 'Copied');
  };

  C.showMessageActions = function showMessageActions(app, screen, message) {
    C.showActionMenu(app, { kind:'message', title:'Message actions', context:message, onAction: (id) => {
      if (id === 'reply') screen.replyToMessage(message.id);
      if (id === 'react') screen.reactToMessage(message.id);
      if (id === 'forward') screen.forwardMessage(message.id);
      if (id === 'edit') screen.editMessage(message.id);
      if (id === 'copy') C.copyText(message.body || message.attachment?.name || '', 'Message copied');
      if (id === 'details') screen.showMessageDetails(message.id);
      if (id === 'delete') screen.deleteMessage(message.id);
    }});
  };

  C.showContactActions = function showContactActions(app, screen, contact) {
    C.showActionMenu(app, { kind:'contact', title:contact.name, context:contact, onAction: (id) => {
      if (id === 'message') { let conversation=app.store.state.conversations.find((x)=>x.contactId===contact.id&&!x.deleted);if(!conversation){const conversationId=`c-${contact.id}`;app.store.update((s)=>s.conversations.unshift({id:conversationId,contactId:contact.id,lastMessage:'',lastAt:Date.now(),direction:'out',unread:0,pinned:false,muted:false,draft:false}),'create-conversation');conversation=app.store.conversation(conversationId);}app.router.navigate(`/chat/${conversation.id}`); }
      if (id === 'connection') app.router.navigate(`/connection/${contact.id}`);
      if (id === 'rename') screen.renameContact(contact.id);
      if (id === 'block') screen.toggleBlock(contact.id);
      if (id === 'remove') screen.removeContact(contact.id);
    }});
  };

  C.showConversationActions = function showConversationActions(app, screen, conversation) {
    C.showActionMenu(app, { kind:'conversation', title:'Conversation actions', context:conversation, onAction: (id) => screen.conversationAction(id, conversation.id) });
  };

  C.globalSearchResults = function globalSearchResults(app, query) {
    const q = String(query || '').toLowerCase();
    return app.store.state.messages.filter((m) => `${m.body || ''} ${m.attachment?.name || ''}`.toLowerCase().includes(q)).map((m) => ({ message:m, conversation:app.store.conversation(m.conversationId), contact:app.store.contact(app.store.conversation(m.conversationId)?.contactId) }));
  };

  C.showSearch = function showSearch(app, options) {
    const t = Torca.i18n.t.bind(Torca.i18n); let selected = 0;
    const scope = options.scope ? app.store.messagesFor(options.scope) : app.store.state.messages;
    C.modal({ title:t('search'), className:'modal--wide search-modal', body:`<div class="search-box"><input class="text-input" data-search-input placeholder="${e(t('search'))}" autocomplete="off"><button class="icon-button" type="button" data-search-clear aria-label="${e(t('clear'))}">${C.icon('close')}</button></div><div class="search-meta" data-search-meta></div><div class="search-results" data-search-results></div>`, bind(root, close) {
      const input=root.querySelector('[data-search-input]'); const meta=root.querySelector('[data-search-meta]'); const list=root.querySelector('[data-search-results]');
      const render=()=>{const q=input.value.trim().toLowerCase();const rows=scope.filter((m)=>`${m.body || ''} ${m.attachment?.name || ''}`.toLowerCase().includes(q));meta.textContent=q ? `${rows.length} ${t('results')}` : t('typeToSearch');list.innerHTML=rows.length?rows.map((m)=>`<button type="button" class="search-result" data-result="${e(m.id)}"><span class="search-result__time">${e(Torca.util.formatTime(m.createdAt))}</span><span>${e(m.body || m.attachment?.name || t('attachment'))}</span></button>`).join(''):`<div class="search-empty">${e(q ? t('noResults') : t('searchHint'))}</div>`;};
      input.addEventListener('input',render);root.querySelector('[data-search-clear]').addEventListener('click',()=>{input.value='';render();input.focus();});list.addEventListener('click',(event)=>{const node=event.target.closest('[data-result]');if(!node)return;close();if(options.onSelect)options.onSelect(node.dataset.result);});input.addEventListener('keydown',(event)=>{if(event.key==='Escape')close();if(event.key==='Enter'){const first=list.querySelector('[data-result]');if(first)first.click();}});render();input.focus();
    }});
  };

  C.showScanner = function showScanner(app, onScan) {
    C.modal({ title:Torca.i18n.t('scanQr'), className:'scanner-modal', body:`<div class="scanner-frame"><span class="scanner-corner tl"></span><span class="scanner-corner tr"></span><span class="scanner-corner bl"></span><span class="scanner-corner br"></span><div class="scanner-line"></div><span class="scanner-label">QR / invitation code</span></div><label class="field-label">${e(Torca.i18n.t('invitationLink'))}<input class="text-input" data-scan-value value="torca://pair?provider=iroh&code=SCANNED&bootstrap=MAQUETTE"></label>`, actions:C.button(Torca.i18n.t('continue'),'scan','data-scan-confirm','primary'), bind(root,close){root.querySelector('[data-scan-confirm]').addEventListener('click',()=>{const value=root.querySelector('[data-scan-value]').value.trim();close();if(onScan)onScan(value);});} });
  };

  C.showTransferCenter = function showTransferCenter(app) {
    const t=Torca.i18n.t.bind(Torca.i18n); const transfers=app.store.state.transfers;
    const renderRows=()=>transfers.map((item)=>`<div class="transfer-row" data-transfer-row="${e(item.id)}"><div class="transfer-row__icon">${C.icon('transfer')}</div><div class="transfer-row__body"><strong>${e(item.name)}</strong><small>${e(item.direction==='in'?'RX':'TX')} · ${e(item.size||'')} · ${e(t(item.state)||item.state)}</small>${item.progress<100?C.progress(item.progress):''}</div><span class="transfer-row__value">${item.progress>=100?C.icon('check','sm'):`${Number(item.progress)||0}%`}</span><div class="transfer-row__actions">${item.state==='complete'?C.iconButton('download',t('download'),'data-transfer-action="save" data-transfer-id="'+e(item.id)+'"'):item.state==='cancelled'?'':C.iconButton(item.state==='paused'?'play':'pause',item.state==='paused'?'Resume':'Pause','data-transfer-action="toggle" data-transfer-id="'+e(item.id)+'"')}${!['complete','cancelled'].includes(item.state)?C.iconButton('close',t('cancel'),'data-transfer-action="cancel" data-transfer-id="'+e(item.id)+'"'):''}</div></div>`).join('');
    C.modal({title:t('transferCenter'),className:'modal--wide transfer-modal',body:transfers.length?`<div class="transfer-list">${renderRows()}</div><p class="page-note">Transfers are resumable and remain visible while the peer is reconnecting.</p>`:C.emptyState('transfer',t('noTransfers'),''),actions:C.button(t('close'),'close','data-close-transfer','outline'),bind(root,close){root.querySelector('[data-close-transfer]').addEventListener('click',close);root.querySelectorAll('[data-transfer-action]').forEach((node)=>node.addEventListener('click',()=>{const id=node.dataset.transferId;const action=node.dataset.transferAction;app.store.update((s)=>{const x=s.transfers.find((v)=>v.id===id);if(!x)return;if(action==='toggle')x.state=x.state==='paused'?(x.direction==='in'?'downloading':'uploading'):'paused';if(action==='cancel')x.state='cancelled';},`transfer-${action}`);close();C.showTransferCenter(app);}));}
    });
  };
}(window.Torca));
