(function (Torca) {
  'use strict';
  Torca.fixtures.baseState = function baseState() {
    const now = Date.now();
    return {
      profile: {
        id:'me', displayName:'Paweł', fingerprint:'9E7A 31C8 54D0 6B2F', version:'0.2.0-alpha.0', build:1,
        sourceCommit:'2c584fc', sourceFingerprint:'maquette-reference', contract:25, wire:1, storageEpoch:3, nativeAbi:1
      },
      ui: {
        locale:'pl', theme:'modern-ocean', mode:'dark', density:'comfortable', viewport:'fluid', platform:'desktop', view:'chats', scenario:'normal', reduceMotion:false,
        live:true, logsPaused:false, diagnosticsTab:'runtime', logLevel:'all'
      },
      runtime: {
        provider:'iroh', providerProfile:'always', state:'ready', path:'direct', routeState:'fresh', endpoint:'iroh:endpoint:…7d2a', peersReady:2, queue:0,
        batteryMode:'automatic', build:'0.3-maquette', contract:25, storageEpoch:3, bytesTx:184320, bytesRx:512440,
        communication:{state:'ready',txSeq:7,rxSeq:11,txActive:false,rxActive:false,latency:21,queued:0,inFlight:0,code:'IROH_READY'},
        peer:{state:'ready',txSeq:5,rxSeq:9,txActive:false,rxActive:false,latency:24,queued:0,inFlight:0,code:'P2P_READY'},
        rendezvous:{state:'idle',txSeq:1,rxSeq:1,txActive:false,rxActive:false,latency:null,queued:0,inFlight:0,code:'PAIRING_IDLE'},
        batteryObservation:{active:false,totalWork:0,energyScore:0,wakeSources:{communication:0,lifecycle:0,delivery:0,radio:0}},
        whyAwake:{leases:0,leaseReasons:{},scheduledWork:{},nextDeadline:null},
        incidents:[],
        logs:[
          {id:'l1',at:now-42_000,level:'INFO',subsystem:'runtime',message:'application runtime ready'},
          {id:'l2',at:now-39_000,level:'INFO',subsystem:'iroh',message:'provider route fresh · profile always'},
          {id:'l3',at:now-31_000,level:'DEBUG',subsystem:'peer',message:'authenticated peer Alice ready · 23 ms'},
          {id:'l4',at:now-18_000,level:'TRACE',subsystem:'runtime',message:'idle · no application deadline'}
        ],
        bootstrap:{phase:'ready',progress:1,startedAt:now-18_000,steps:[
          {id:'storage',label:'localStorage',state:'ready',progress:100},
          {id:'identity',label:'deviceIdentity',state:'ready',progress:100},
          {id:'communication',label:'communicationRuntime',state:'ready',progress:100}
        ]}
      },
      preferences: {
        readReceipts:true, notifications:true, closeToTray:true, batteryMode:'automatic', allowDelayedBackgroundDelivery:true,
        metered:'pause-large', visualActivity:'follow-system', reduceMotion:false, audioInput:'system', audioOutput:'system'
      },
      contacts: [
        { id:'alice', name:'Alice', initials:'AL', online:true, blocked:false, verified:true, identityChanged:false, lastSeen:now-30_000, route:'direct', safety:'1572 4418 8319 2205 7211 0394', instant:true, radioEnabled:false, peerHealth:{state:'ready',quality:'excellent',rtt:23,lastSuccess:now-18_000,failures:0,reconnectAttempt:0} },
        { id:'jan', name:'Jan Kowalski', initials:'JK', online:false, blocked:false, verified:false, identityChanged:false, lastSeen:now-3_600_000, route:'relay', safety:'6880 1202 4419 3301 5982 1044', instant:false, radioEnabled:false, peerHealth:{state:'reconnecting',quality:'fair',rtt:92,lastSuccess:now-3_600_000,failures:2,reconnectAttempt:3} },
        { id:'marta', name:'Marta Zielińska', initials:'MZ', online:true, blocked:false, verified:true, identityChanged:false, lastSeen:now-90_000, route:'direct', safety:'2044 9120 6421 7883 0156 2270', instant:false, radioEnabled:true, peerHealth:{state:'ready',quality:'good',rtt:37,lastSuccess:now-44_000,failures:0,reconnectAttempt:0} }
      ],
      conversations: [
        { id:'c-alice', contactId:'alice', lastMessage:'Jasne, podeślę Ci to wieczorem.', lastAt:now-62_000, direction:'in', unread:2, pinned:true, muted:false, draft:false },
        { id:'c-marta', contactId:'marta', lastMessage:'Super, działa po restarcie.', lastAt:now-18*60_000, direction:'out', unread:0, pinned:false, muted:false, draft:false },
        { id:'c-jan', contactId:'jan', lastMessage:'Mam jeszcze jedną rzecz do sprawdzenia…', lastAt:now-7_200_000, direction:'in', unread:0, pinned:false, muted:true, draft:true }
      ],
      messages: [
        { id:'m1', conversationId:'c-alice', direction:'in', body:'Hej, testuję nową wersję Torca. Jak wygląda u Ciebie?', createdAt:now-42*60_000, status:'read' },
        { id:'m2', conversationId:'c-alice', direction:'out', body:'Znacznie lepiej. Najbardziej chcę teraz dopracować sam wygląd rozmowy.', createdAt:now-40*60_000, status:'read' },
        { id:'m3', conversationId:'c-alice', direction:'out', body:'Balony powinny być lżejsze i mniej przypominać karty.', createdAt:now-39.5*60_000, status:'read', grouped:true },
        { id:'m4', conversationId:'c-alice', direction:'in', body:'Zdecydowanie. Timestamp i status też mogą być dużo spokojniejsze.', createdAt:now-34*60_000, status:'read', reactions:['👍','💯'] },
        { id:'m5', conversationId:'c-alice', direction:'out', body:'Sprawdzę też composer na małym Androidzie.', createdAt:now-31*60_000, status:'delivered', replyTo:'m4' },
        { id:'m6', conversationId:'c-alice', direction:'in', body:'Jasne, podeślę Ci to wieczorem.', createdAt:now-62_000, status:'read' },
        { id:'m7', conversationId:'c-marta', direction:'in', body:'Po zmianie sieci widzę reconnect, ale wiadomość doszła.', createdAt:now-24*60_000, status:'read' },
        { id:'m8', conversationId:'c-marta', direction:'out', body:'Super, działa po restarcie.', createdAt:now-18*60_000, status:'read' },
        { id:'m9', conversationId:'c-jan', direction:'in', body:'Mam jeszcze jedną rzecz do sprawdzenia…', createdAt:now-7_200_000, status:'read' },
        { id:'m-file', conversationId:'c-alice', direction:'out', body:'', createdAt:now-28*60_000, status:'read', attachment:{ name:'torca-ui-notes.pdf', size:'2.4 MB', kind:'pdf', progress:100 } }
      ],
      pairings: [
        { id:'p1', role:'creator', state:'open', code:'7KQ2FD', invite:'torca://pair?provider=iroh&code=7KQ2FD&bootstrap=DEMO', expiresAt:now+4*60_000, remoteName:null }
      ],
      transfers: [
        { id:'t1', name:'design-reference.png', direction:'out', state:'complete', progress:100, size:'1.8 MB' },
        { id:'t2', name:'voice-note.opus', direction:'in', state:'complete', progress:100, size:'184 KB' }
      ],
      alerts: []
    };
  };
}(window.Torca));
