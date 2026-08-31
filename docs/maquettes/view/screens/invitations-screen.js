(function (Torca) {
  'use strict';
  const C=Torca.components; const e=Torca.util.escape;

  class InvitationsScreen extends Torca.core.Screen {
    render(){
      const t=Torca.i18n.t.bind(Torca.i18n);
      const active=this.app.store.state.pairings.filter((p)=>!['completed','rejected','expired'].includes(p.state)&&p.expiresAt>Date.now());
      const cards=active.map((p)=>`<div class="setting-row pairing-row"><div><strong>${e(p.remoteName||(p.role==='creator'?t('createInvitation'):t('joinInvitation')))}</strong><small>${e(p.code)} · ${e(p.state)} · ${e(t('expires'))} ${Torca.util.formatRelative(p.expiresAt)}</small></div>${C.button(t('viewDetails'),'info',`data-open-pairing="${e(p.id)}"`,'outline')}</div>`).join('');
      const body=`<div class="screen-scroll"><div class="page narrow"><div class="page-header"><div><h2>${e(t('invitations'))}</h2><p>QR/link-first pairing for Iroh.</p></div></div><div class="pairing-primary-actions">${C.button(t('createInvitation'),'invitations','data-create-invite','primary')}${C.button(t('joinInvitation'),'plus','data-join-invite','outline')}</div>${active.length?`<div class="card"><div class="setting-row"><strong>${e(t('activeInvitations'))}</strong><span>${active.length}</span></div>${cards}</div>`:C.emptyState('invitations',t('noInvitations'),t('noInvitationsBody'))}</div></div>`;
      return C.shell(this.app,{title:t('invitations'),body});
    }
    bind(){C.bindShell(this.root,this.app);this.on('[data-create-invite]','click',()=>this.showCreate());this.on('[data-join-invite]','click',()=>this.showJoin());this.on('[data-open-pairing]','click',(event)=>this.showPairing(event.currentTarget.dataset.openPairing));}
    showCreate(){
      const t=Torca.i18n.t.bind(Torca.i18n);const pairing=this.app.store.state.pairings.find((p)=>p.role==='creator'&&p.state==='open')||this.app.store.state.pairings[0];
      C.modal({title:t('createInvitation'),body:`<p class="modal-intro">Share this temporary QR code or invitation link.</p>${C.qr()}<div class="invite-link">${e(pairing?.invite||'torca://pair?provider=iroh&demo=1')}</div>`,actions:C.button(t('copyLink'),'copy','data-copy-link','primary'),bind:(root)=>root.querySelector('[data-copy-link]').addEventListener('click',()=>C.copyText(pairing?.invite||'',t('copyLink')))});
    }
    showJoin(){
      const t=Torca.i18n.t.bind(Torca.i18n);
      C.modal({title:t('joinInvitation'),body:`<p class="modal-intro">Paste an invitation link or scan a QR code.</p><label class="field-label">${e(t('invitationLink'))}<input class="text-input" data-join-value value="torca://pair?provider=iroh&code=DEMO&bootstrap=MAQUETTE"></label>${C.button(t('scanQr'),'scan','data-scan','outline')}`,actions:C.button(t('joinInvitation'),'plus','data-join-confirm','primary'),bind:(root,close)=>{root.querySelector('[data-scan]').addEventListener('click',()=>{close();C.showScanner(this.app,(value)=>this.joinValue(value));});root.querySelector('[data-join-confirm]').addEventListener('click',()=>{this.joinValue(root.querySelector('[data-join-value]').value);close();});}});
    }
    joinValue(value){this.app.store.update((s)=>s.pairings.push({id:Torca.util.id('pair'),role:'joiner',state:'joining',code:(String(value).match(/code=([^&]+)/)||[])[1]||'DEMO',invite:value,expiresAt:Date.now()+180000,remoteName:'New contact'}),'join-pairing');C.toast('Pairing started');}
    showPairing(id){
      const p=this.app.store.state.pairings.find((x)=>x.id===id);if(!p)return;const t=Torca.i18n.t.bind(Torca.i18n);const awaiting=p.state==='awaiting';const candidate={id:p.id,name:p.remoteName||'New contact',initials:Torca.util.initials(p.remoteName),online:true,genome:p.code};
      C.modal({title:p.remoteName||t('createInvitation'),body:awaiting?`<div class="pairing-candidate">${C.avatar(candidate,'large')}<h3>${e(candidate.name)}</h3><p>This person joined your invitation. Approve explicitly before the contact becomes durable.</p></div>`:`${C.qr()}<p class="pairing-code">${e(p.code)} · ${e(p.state)}</p>`,actions:awaiting?`${C.button(t('reject'),'close','data-pair-reject','outline')}${C.button(t('approve'),'check','data-pair-approve','primary')}`:'',bind:(root,close)=>{root.querySelector('[data-pair-approve]')?.addEventListener('click',()=>{this.app.store.update((s)=>{const current=s.pairings.find((x)=>x.id===id);if(current)current.state='completed';if(!s.contacts.some((c)=>c.name===p.remoteName))s.contacts.push({id:Torca.util.id('contact'),name:candidate.name,initials:candidate.initials,online:true,blocked:false,verified:false,identityChanged:false,lastSeen:Date.now(),route:'direct',safety:'1000 2000 3000 4000 5000 6000',genome:p.code,instant:false,radioEnabled:false,peerHealth:{state:'ready',quality:'good',rtt:32,lastSuccess:Date.now(),failures:0,reconnectAttempt:0}});},'approve-pairing');close();});root.querySelector('[data-pair-reject]')?.addEventListener('click',()=>{this.app.store.update((s)=>{const current=s.pairings.find((x)=>x.id===id);if(current)current.state='rejected';},'reject-pairing');close();});}});
    }
  }
  Torca.screens.InvitationsScreen=InvitationsScreen;
}(window.Torca));
