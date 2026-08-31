(function (Torca) {
  'use strict';
  const en = {
    chats:'Chats', contacts:'Contacts', invitations:'Invitations', settings:'Settings', diagnostics:'Diagnostics', uiLab:'UI Lab',
    search:'Search', newMessage:'New message', noChats:'No conversations yet', noChatsBody:'Pair a contact to start a private conversation.',
    online:'Online', offline:'Offline', reconnecting:'Reconnecting', blocked:'Blocked', draft:'Draft', you:'You', today:'Today', newMessages:'New messages',
    messagePlaceholder:'Message', attach:'Attach', send:'Send', voice:'Voice', delivered:'Delivered', read:'Read', sending:'Sending', failed:'Failed', retry:'Retry',
    contactDetails:'Contact details', verifyIdentity:'Verify identity', sharedMedia:'Shared media', connection:'Connection', rename:'Rename', block:'Block', remove:'Remove',
    pairContact:'Pair contact', createInvitation:'Create invitation', joinInvitation:'Join invitation', activeInvitations:'Active invitations',
    copyLink:'Copy link', approve:'Approve', reject:'Reject', expires:'Expires', scanQr:'Scan QR', invitationLink:'Invitation link',
    appearance:'Appearance', themeFamily:'Theme', variant:'Variant', colorMode:'Color mode', density:'Density', language:'Language',
    privacy:'Privacy', readReceipts:'Send read receipts', notifications:'Notifications', battery:'Battery & availability',
    batteryMode:'Availability mode', automatic:'Automatic', alwaysAvailable:'Always available', batterySaver:'Battery saver',
    runtime:'Runtime', provider:'Provider', queue:'Pending work', transfers:'Transfers', build:'Build', healthy:'Healthy', degraded:'Degraded',
    viewDetails:'View details', normal:'Normal', empty:'Empty account', longContent:'Long content', transfer:'Attachment transfer', pairing:'Pairing attention', identityWarning:'Identity warning',
    reset:'Reset', close:'Close', themeModern:'Modern', themeTerminal:'Terminal', comfortable:'Comfortable', compact:'Compact', light:'Light', dark:'Dark',
    profile:'Profile', noContacts:'No contacts yet', noContactsBody:'Create or join an invitation to add your first contact.', lastSeen:'Last seen',
    safetyNumber:'Safety number', verified:'Verified', notVerified:'Not verified', identityChanged:'Identity changed — verify before sending.',
    noInvitations:'No active invitations', noInvitationsBody:'Create a QR/link when you want to add somebody.',
    engineeringOnly:'Engineering surface — technical detail is intentionally kept out of normal chat flows.',
    maquette:'0.3 maquette', scenario:'Scenario', viewport:'Viewport', locale:'Locale'
  };
  const pl = {
    chats:'Czaty', contacts:'Kontakty', invitations:'Zaproszenia', settings:'Ustawienia', diagnostics:'Diagnostyka', uiLab:'UI Lab',
    search:'Szukaj', newMessage:'Nowa wiadomość', noChats:'Brak rozmów', noChatsBody:'Sparuj kontakt, aby rozpocząć prywatną rozmowę.',
    online:'Online', offline:'Offline', reconnecting:'Ponowne łączenie', blocked:'Zablokowany', draft:'Wersja robocza', you:'Ty', today:'Dzisiaj', newMessages:'Nowe wiadomości',
    messagePlaceholder:'Wiadomość', attach:'Załącz', send:'Wyślij', voice:'Głos', delivered:'Dostarczono', read:'Odczytano', sending:'Wysyłanie', failed:'Błąd', retry:'Ponów',
    contactDetails:'Szczegóły kontaktu', verifyIdentity:'Zweryfikuj tożsamość', sharedMedia:'Wspólne pliki', connection:'Połączenie', rename:'Zmień nazwę', block:'Zablokuj', remove:'Usuń',
    pairContact:'Dodaj kontakt', createInvitation:'Utwórz zaproszenie', joinInvitation:'Dołącz do zaproszenia', activeInvitations:'Aktywne zaproszenia',
    copyLink:'Kopiuj link', approve:'Akceptuj', reject:'Odrzuć', expires:'Wygasa', scanQr:'Skanuj QR', invitationLink:'Link zaproszenia',
    appearance:'Wygląd', themeFamily:'Motyw', variant:'Wariant', colorMode:'Tryb kolorów', density:'Gęstość', language:'Język',
    privacy:'Prywatność', readReceipts:'Wysyłaj potwierdzenia odczytu', notifications:'Powiadomienia', battery:'Bateria i dostępność',
    batteryMode:'Tryb dostępności', automatic:'Automatyczny', alwaysAvailable:'Zawsze dostępny', batterySaver:'Oszczędzanie baterii',
    runtime:'Runtime', provider:'Provider', queue:'Oczekujące zadania', transfers:'Transfery', build:'Build', healthy:'Zdrowy', degraded:'Ograniczony',
    viewDetails:'Szczegóły', normal:'Normalny', empty:'Puste konto', longContent:'Długie treści', transfer:'Transfer załącznika', pairing:'Oczekujące parowanie', identityWarning:'Zmiana tożsamości',
    reset:'Resetuj', close:'Zamknij', themeModern:'Modern', themeTerminal:'Terminal', comfortable:'Wygodna', compact:'Kompaktowa', light:'Jasny', dark:'Ciemny',
    profile:'Profil', noContacts:'Brak kontaktów', noContactsBody:'Utwórz lub otwórz zaproszenie, aby dodać pierwszy kontakt.', lastSeen:'Ostatnio widziany',
    safetyNumber:'Numer bezpieczeństwa', verified:'Zweryfikowany', notVerified:'Niezweryfikowany', identityChanged:'Tożsamość uległa zmianie — zweryfikuj przed wysłaniem.',
    noInvitations:'Brak aktywnych zaproszeń', noInvitationsBody:'Utwórz kod QR/link, gdy chcesz dodać nową osobę.',
    engineeringOnly:'Ekran inżynierski — szczegóły techniczne celowo nie trafiają do zwykłych flow czatu.',
    maquette:'Makieta 0.3', scenario:'Scenariusz', viewport:'Widok', locale:'Język'
  };
  Torca.i18n = {
    locale: 'pl',
    setLocale(locale) { this.locale = locale === 'en' ? 'en' : 'pl'; document.documentElement.lang = this.locale; },
    t(key, vars) {
      let text = (this.locale === 'pl' ? pl : en)[key] || en[key] || key;
      if (vars) Object.entries(vars).forEach(([name, value]) => { text = text.replaceAll(`{${name}}`, value); });
      return text;
    }
  };
}(window.Torca));
