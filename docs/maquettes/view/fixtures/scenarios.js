(function (Torca) {
  'use strict';
  const scenarios = {
    normal() { return Torca.fixtures.baseState(); },
    empty() {
      const s = Torca.fixtures.baseState(); s.contacts=[]; s.conversations=[]; s.messages=[]; s.pairings=[]; s.transfers=[]; s.runtime.peersReady=0; return s;
    },
    offline() {
      const s = Torca.fixtures.baseState(); s.runtime.state='offline'; s.runtime.path='none'; s.runtime.peersReady=0; s.contacts.forEach((c) => { c.online=false; }); return s;
    },
    long() {
      const s = Torca.fixtures.baseState();
      s.contacts[0].name='Aleksandra Z bardzo długim nazwiskiem testującym responsywność';
      s.conversations[0].lastMessage='Bardzo długa wiadomość sprawdzająca zawijanie tekstu, zachowanie layoutu przy nietypowej długości i to, czy interfejs nadal utrzymuje prawidłową hierarchię informacji.';
      s.messages.push({ id:'long', conversationId:'c-alice', direction:'in', body:'To jest celowo bardzo długa wiadomość. Powinna wyglądać jak zwykła część rozmowy, a nie jak ogromna karta. Testujemy tutaj szerokość, łamanie bardzo-długiego-ciągu-bez-spacji-abcdefghijklmnopqrstuvxyz0123456789, skalowanie typografii oraz zachowanie stopki wiadomości.\n\nDrugi akapit sprawdza też pionowy rytm.', createdAt:Date.now()-20_000, status:'read' });
      return s;
    },
    transfer() {
      const s=Torca.fixtures.baseState(); s.runtime.queue=1; s.transfers=[{id:'t2',name:'video-demo.mp4',state:'uploading',progress:46}];
      s.messages.push({id:'upload',conversationId:'c-alice',direction:'out',body:'',createdAt:Date.now()-12_000,status:'sending',attachment:{name:'video-demo.mp4',size:'38.7 MB',kind:'video',progress:46}}); return s;
    },
    pairing() {
      const s=Torca.fixtures.baseState(); s.pairings.push({id:'p2',role:'creator',state:'awaiting',code:'91MXPQ',invite:'torca://pair?provider=iroh&code=91MXPQ&bootstrap=DEMO',expiresAt:Date.now()+120_000,remoteName:'Kasia'}); return s;
    },
    identity() {
      const s=Torca.fixtures.baseState(); s.contacts[0].identityChanged=true; s.contacts[0].verified=false; s.alerts.push({kind:'identity',contactId:'alice'}); return s;
    }
  };
  Torca.fixtures.scenarios = scenarios;
  Torca.fixtures.scenario = function scenario(name) { return (scenarios[name] || scenarios.normal)(); };
}(window.Torca));
