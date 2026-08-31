(function (Torca) {
  'use strict';
  const scenarios = {
    normal() { return Torca.fixtures.baseState(); },
    empty() {
      const s = Torca.fixtures.baseState();
      s.contacts=[]; s.conversations=[]; s.messages=[]; s.pairings=[]; s.transfers=[]; s.runtime.peersReady=0;
      s.runtime.peer.state='idle'; s.runtime.peer.code='NO_PEERS';
      return s;
    },
    offline() {
      const s = Torca.fixtures.baseState();
      s.runtime.state='offline'; s.runtime.path='none'; s.runtime.routeState='stale'; s.runtime.peersReady=0;
      s.runtime.communication.state='offline'; s.runtime.communication.code='NETWORK_OFFLINE';
      s.runtime.peer.state='offline'; s.runtime.peer.code='P2P_OFFLINE';
      s.contacts.forEach((c) => { c.online=false; c.peerHealth.state='reconnecting'; });
      s.runtime.logs.push({id:'loff',at:Date.now()-2000,level:'WARN',subsystem:'connectivity',message:'default network unavailable · durable work retained'});
      return s;
    },
    long() {
      const s = Torca.fixtures.baseState();
      s.contacts[0].name='Aleksandra Z bardzo długim nazwiskiem testującym responsywność';
      s.conversations[0].lastMessage='Bardzo długa wiadomość sprawdzająca zawijanie tekstu, zachowanie layoutu przy nietypowej długości i to, czy interfejs nadal utrzymuje prawidłową hierarchię informacji.';
      s.messages.push({ id:'long', conversationId:'c-alice', direction:'in', body:'To jest celowo bardzo długa wiadomość. Powinna wyglądać jak zwykła część rozmowy, a nie jak ogromna karta. Testujemy tutaj szerokość, łamanie bardzo-długiego-ciągu-bez-spacji-abcdefghijklmnopqrstuvxyz0123456789, skalowanie typografii oraz zachowanie stopki wiadomości.\n\nDrugi akapit sprawdza też pionowy rytm.', createdAt:Date.now()-20_000, status:'read' });
      return s;
    },
    transfer() {
      const s=Torca.fixtures.baseState();
      s.runtime.queue=1; s.runtime.communication.queued=1;
      s.transfers=[{id:'t-upload',name:'video-demo.mp4',direction:'out',state:'uploading',progress:46,size:'38.7 MB'},{id:'t-download',name:'archive.zip',direction:'in',state:'paused',progress:71,size:'120 MB'}];
      s.messages.push({id:'upload',conversationId:'c-alice',direction:'out',body:'',createdAt:Date.now()-12_000,status:'sending',attachment:{name:'video-demo.mp4',size:'38.7 MB',kind:'video',progress:46}});
      return s;
    },
    pairing() {
      const s=Torca.fixtures.baseState();
      s.pairings.push({id:'p2',role:'creator',state:'awaiting',code:'91MXPQ',invite:'torca://pair?provider=iroh&code=91MXPQ&bootstrap=DEMO',expiresAt:Date.now()+120_000,remoteName:'Kasia'});
      s.runtime.rendezvous.state='ready';
      return s;
    },
    identity() {
      const s=Torca.fixtures.baseState();
      s.contacts[0].identityChanged=true; s.contacts[0].verified=false; s.alerts.push({kind:'identity',contactId:'alice'});
      return s;
    },
    startup() {
      const s=Torca.fixtures.baseState();
      s.runtime.state='starting'; s.runtime.communication.state='starting'; s.runtime.peer.state='idle';
      s.runtime.bootstrap={phase:'starting',progress:.62,startedAt:Date.now()-14_000,steps:[
        {id:'storage',label:'localStorage',state:'ready',progress:100},
        {id:'identity',label:'deviceIdentity',state:'ready',progress:100},
        {id:'communication',label:'communicationRuntime',state:'working',progress:48}
      ]};
      return s;
    },
    profile() {
      const s=Torca.fixtures.baseState();
      s.profile.displayName='';
      s.contacts=[]; s.conversations=[]; s.messages=[];
      return s;
    }
  };
  Torca.fixtures.scenarios = scenarios;
  Torca.fixtures.scenario = function scenario(name) { return (scenarios[name] || scenarios.normal)(); };
}(window.Torca));
