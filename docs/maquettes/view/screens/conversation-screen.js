(function (Torca) {
  'use strict';
  class ConversationScreen extends Torca.core.Screen {
    constructor(app,params){ super(app,params); this.replyTo=null; }
    conversationHtml(conversation,contact){
      const store=this.app.store; const t=Torca.i18n.t.bind(Torca.i18n); const messages=store.messagesFor(conversation.id); const identityBlocked=contact.identityChanged;
      const alert=identityBlocked?`<div style="padding:8px 14px;background:color-mix(in srgb,var(--danger) 12%,var(--surface));color:var(--danger);font-size:12px;font-weight:700;display:flex;align-items:center;gap:8px">${Torca.components.icon('warning','sm')}<span style="flex:1">${Torca.util.escape(t('identityChanged'))}</span><button class="button outline" type="button" data-route="/contact/${contact.id}" style="min-height:30px">${Torca.util.escape(t('verifyIdentity'))}</button></div>`:'';
      let previous=null; const bubbles=messages.map((m)=>{const html=Torca.components.messageBubble(store,m,previous);previous=m;return html;}).join('');
      const reply=this.replyTo?store.state.messages.find((m)=>m.id===this.replyTo):null;
      return `<div class="conversation"><header class="conversation__header">${this.app.isCompact()?Torca.components.iconButton('back','Back','data-route="/chats"'):''}${Torca.components.avatar(contact,'small')}<div class="conversation__header-main"><strong>${Torca.util.escape(contact.name)}</strong><span>${Torca.util.escape(Torca.components.contactStatus(contact))}</span></div>${Torca.components.iconButton('search',t('search'),'data-conversation-search')}${Torca.components.iconButton('info',t('contactDetails'),`data-route="/contact/${contact.id}"`)}${this.app.isCompact()?Torca.components.iconButton('settings',t('settings'),'data-route="/settings"'):''}</header>${alert}<div class="timeline" data-timeline><div class="timeline__day">${Torca.util.escape(t('today'))}</div>${bubbles}</div>${Torca.components.composer({reply,disabled:identityBlocked})}</div>`;
    }
    render(){
      const store=this.app.store; const c=store.conversation(this.params.id); if(!c)return Torca.components.shell(this.app,{title:'Torca',body:Torca.components.emptyState('warning','Conversation missing','Fixture does not contain this conversation.')});
      const contact=store.contact(c.contactId); const chat=this.conversationHtml(c,contact);
      let body=chat;
      if(!this.app.isCompact()){
        const list=`<div class="conversation-list">${store.state.conversations.map((item)=>Torca.components.conversationTile(store,item,item.id===c.id)).join('')}</div>`;
        const context=this.app.isWide()?`<aside class="context-pane"><div style="text-align:center">${Torca.components.avatar(contact,'large')}<h3 style="margin:12px 0 4px">${Torca.util.escape(contact.name)}</h3><p style="color:var(--muted)">${Torca.util.escape(Torca.components.contactStatus(contact))}</p></div><div class="card card-pad"><div class="section-title">${Torca.util.escape(Torca.i18n.t('connection'))}</div><p style="margin:0">Iroh · ${Torca.util.escape(contact.route)}</p></div></aside>`:'';
        body=`<div class="split-view ${context?'with-context':''}"><aside class="list-pane">${list}</aside><section class="content-pane">${chat}</section>${context}</div>`;
      }
      return Torca.components.shell(this.app,{title:contact.name,subtitle:Torca.components.contactStatus(contact),hideHeader:true,body});
    }
    bind(){
      Torca.components.bindShell(this.root,this.app);
      this.on('[data-conversation]','click',(event)=>this.app.router.navigate(`/chat/${event.currentTarget.dataset.conversation}`));
      this.on('[data-conversation-search]','click',()=>Torca.components.toast('Conversation search — result navigation prototype'));
      this.on('[data-message]','dblclick',(event)=>{this.replyTo=event.currentTarget.dataset.message;this.mount(this.root);});
      this.on('[data-cancel-reply]','click',()=>{this.replyTo=null;this.mount(this.root);});
      this.on('[data-retry-message]','click',(event)=>this.retry(event.currentTarget.dataset.retryMessage));
      const input=this.q('[data-message-input]'); const send=this.q('[data-send]');
      if(input&&send){
        const sync=()=>{send.innerHTML=Torca.components.icon(input.value.trim()? 'send':'mic');send.title=input.value.trim()?Torca.i18n.t('send'):Torca.i18n.t('voice');};
        input.addEventListener('input',sync); sync();
        input.addEventListener('keydown',(event)=>{if(event.key==='Enter'&&!event.shiftKey){event.preventDefault();this.send(input.value);}});
        send.addEventListener('click',()=>input.value.trim()?this.send(input.value):this.voice());
      }
      this.on('[data-attach]','click',()=>this.attach());
      requestAnimationFrame(()=>{const timeline=this.q('[data-timeline]');if(timeline)timeline.scrollTop=timeline.scrollHeight;});
    }
    send(value){
      const body=String(value||'').trim(); if(!body)return; const id=Torca.util.id('msg'); const cid=this.params.id; const now=Date.now(); const reply=this.replyTo;
      this.replyTo=null;
      this.app.store.update((s)=>{s.messages.push({id,conversationId:cid,direction:'out',body,createdAt:now,status:'sending',replyTo:reply});const c=s.conversations.find((x)=>x.id===cid);if(c){c.lastMessage=body;c.lastAt=now;c.direction='out';c.draft=false;}},'send-message');
      window.setTimeout(()=>this.setStatus(id,'delivered'),650); window.setTimeout(()=>this.setStatus(id,'read'),1600);
    }
    setStatus(id,status){this.app.store.update((s)=>{const m=s.messages.find((x)=>x.id===id);if(m)m.status=status;},`message-${status}`);}
    retry(id){this.setStatus(id,'sending');window.setTimeout(()=>this.setStatus(id,'delivered'),800);}
    attach(){
      const id=Torca.util.id('file'); const now=Date.now(); const cid=this.params.id;
      this.app.store.update((s)=>{s.messages.push({id,conversationId:cid,direction:'out',body:'',createdAt:now,status:'sending',attachment:{name:'design-reference.png',size:'1.8 MB',kind:'image',progress:42}});},'mock-attachment');
      window.setTimeout(()=>this.app.store.update((s)=>{const m=s.messages.find((x)=>x.id===id);if(m){m.status='delivered';m.attachment.progress=100;}},'attachment-complete'),1200);
    }
    voice(){Torca.components.toast('Voice/Radio primary action — interaction placeholder');}
  }
  Torca.screens.ConversationScreen=ConversationScreen;
}(window.Torca));
