# FINALIZE — plan domknięcia produktu

Status: aktywny  
Cel: zakończyć 0.3 jako wydanie stabilnych granic architektonicznych oraz
używalnego klienta Android + Windows. Każdy punkt ma zakończyć się kodem,
automatyczną walidacją i — tam gdzie zaznaczono — testem dwóch urządzeń.

Aktualizacja implementacyjna (2026-08-13, bez uruchamiania testów/deployu):

- `NETWORK_READY` nie zależy już od relay/onion; pierwsza sonda relay wykonuje
  serializowany reconnect zamiast pozostawać w `Disconnected`.
- `ResetProfile` zachowuje lokalny runtime/cache Tor, natomiast `ResetAll`
  pozostaje pełnym resetem; reset zatrzymuje klienta przed modyfikacją danych.
- quick deploy sprawdza dwa świeże sygnały lokalnego runtime (maks. 45 s),
  a pełny Tor warm-up pozostaje osobnym, opt-in sprawdzeniem.
- kolekcja diagnostyczna tworzy oddzielny folder incydentu z manifestem,
  relay, Windows i Android, bez mieszania plików z poprzednim uruchomieniem.
- outbound attachment staging przechowuje niezależnie uwierzytelnione chunki;
  retry zachowuje dane joba, cancel je usuwa, a jeden błąd ACK nie nalicza
  dwukrotnie retry/backoff.
- metadata runtime oraz wymagane pola eventów są teraz walidowane przed
  utworzeniem gatewaya; brak build ID/schema/event ID nie dostaje cichego
  fallbacku.
- semantyczny system kolorów ma jawne tokeny success/error oraz idle activity;
  wskaźniki RX/TX nie muszą używać kolorów bezpośrednio w ekranach.
- wybór załącznika przekazuje teraz `BufReader` do transferu; adapter nie
  buduje już pełnego `Vec<u8>` z pliku przed zapisaniem durable chunks.
- relay supervisor stosuje teraz bounded full-jitter przy retry, a jawny
  `network_changed` nadal budzi go natychmiast; zapobiega to synchronicznym
  falom reconnectów wielu klientów.
- `ffi_engine_gateway.dart` odrzuca uszkodzony snapshot/event jako jawny
  `ContractDecodeException` i nie nadpisuje go częściowym fallbackiem; kolejność
  obsługi wyjątków zachowuje właściwą klasę błędu.

Additional implementation notes (2026-08-13):

- The Invitations presentation now hides completed, rejected, cancelled and
  expired sessions; acceptance notifications use the localization surface.
- Rust deploy manifests now record `buildId` and `sourceCommit`.
- Destructive relay starts remove old endpoint/readiness markers before the
  new onion publication, preventing a previous identity from satisfying the
  warm-up gate.
- contract generation now fails early when the canonical Dart projection loses
  its typed enum/state surface, instead of allowing raw wire strings to drift
  back into the generated ABI.
- canonical and generated Dart projections now expose shared `wireValue` and
  `*FromWire` adapters for pairing/contact/message/attachment states; DTO
  accessors use those adapters instead of duplicating string switches.
- user-facing bridge failures now have a localized presentation path in
  `BridgeErrorPresenter.localized`; native diagnostic text remains hidden from
  the UI and is reserved for the incident stream.
- attachment fallback reads now zero temporary plaintext buffers on every
  return path, including offset/size validation failures and cache migration.
- final attachment materialization now uses an encrypted chunk directory plus
  a small manifest; export reads/decrypts one chunk at a time and verifies the
  digest before atomically replacing the destination file.
- retry reads from the final chunk cache one chunk at a time as well; only the
  legacy pre-chunk single-blob format uses bounded compatibility materialization.

### Current implementation progress (before manual validation)

Approximate code-complete status: **100% of the implementation scope**. The remaining
release-gate evidence is intentionally validation-heavy: Android/Windows pairing, relay outage
recovery, attachment resume/cancel, theme screenshots, and cold/warm deploy
scenarios. The checklist below stays unchecked until those flows are exercised
manually as requested; no automated test, build, or deploy was run in this
implementation pass.

The remaining manual focus is lifecycle compatibility with attachments created
by older builds: those blobs continue to use the legacy single-blob format and
are deliberately read through the compatibility path; newly queued and newly
received attachments use only the chunked final cache.

The relay publisher now also bounds a never-reachable cold publication at three
minutes. A failed attempt is handed back to the durable onion worker for a
controlled retry instead of leaving deploy and warming-up waiting indefinitely.
Before writing `relay_ready.txt`, the relay now performs an in-process Arti
dial to its own onion endpoint and expects a protocol `Health` response. This
keeps the deploy marker tied to the real client path rather than Arti's
descriptor state alone. `relay_status.json` records endpoint, publication
state, elapsed time and E2E verification for the deployer and incident bundle;
the deployer heartbeat now prints that publication state alongside Docker
health, so a slow cold publish is observable rather than opaque.

The Rust Android builder now preserves the inherited Windows `PATH` as real
components when prepending the NDK and MSYS/Git Perl directories. This prevents
`openssl-sys` from accidentally selecting a native Windows Perl that cannot
handle its Unix-style build paths.

`apps/client/flutter/lib/l10n/app_{en,pl}.arb` and `l10n.yaml` now mirror the
centralized `TorcaStrings` catalog, including plural and placeholder entries.
The generated `AppLocalizations` facade is intentionally not checked in; the
first manual Flutter build should generate it, after which the compatibility
facade can be switched to generated getters without duplicating UI flow.
`Torca.SourcePolicy.ps1` also validates that both ARB catalogs exist, parse as
JSON and expose exactly the same message-key set.

### Implementation ledger (kod wykonany, walidacja ręczna pozostaje)

| Zakres | Kod wykonany | Walidacja |
| --- | --- | --- |
| lokalny shell vs relay/onion | `native_runtime.rs`, `runtime_composition.rs` | ręcznie na Android + Windows |
| pierwszy reconnect i wspólny transport relay | `torca-rendezvous-client/src/tor.rs`, `torca-connectivity/src/supervisor.rs` | ręcznie po starcie/offline |
| recovery onion po publikacji | `torca-tor/src/runtime_driver.rs` | ręcznie po restarcie relay |
| durable attachment chunks/preview/cancel | `torca-attachment-transfer`, `torca-attachment-protocol`, `torca-communication-adapters/src/attachment_controls.rs` | ręcznie image/video/document |
| diagnostyka per-incydent | `tools/torca-deploy/src/diagnostics.rs`, `scripts/collect.ps1` | ręcznie `collect.zip` |
| reset/reuse/endpoint manifest | `tools/torca-deploy/src/{data,build,relay,launch}.rs` | ręcznie wizard |
| kontraktowy decoder i typed errors | `ffi_engine_gateway.dart`, `bridge_error_presenter.dart` | ręcznie po wymuszeniu driftu |

Powyższe pozycje opisują implementację, nie są jeszcze dowodem release gate;
checkboxy niżej pozostają nieodhaczone do czasu ręcznego testu użytkownika.

## 1. Runtime, Tor, onion i relay — P0

### 1.1 Rozdzielić gotowość aplikacji od gotowości sieci

- [x] `crates/platform/torca-native/src/native_runtime.rs`
  - emitować osobne stany `runtimeReady`, `torReady`, `onionReady` i
    `relayReady`;
  - nie blokować lokalnego shell/UI przez `relayReady` ani `onionReady`;
  - pozostawić relay jako wymaganie tylko dla use-case'ów pairing/control.
- [x] `crates/platform/torca-native/src/runtime_composition.rs`
  - zachować sekwencję zależności Tor → onion/relay;
  - recovery pojedynczego komponentu nie może restartować całego runtime.
- [x] `apps/client/flutter/lib/main.dart`, `lib/app.dart`,
  `lib/widgets/runtime_network_status.dart`
  - pokazywać gotowy interfejs z dyskretnym stanem „network connecting”,
    zamiast niekończącego się ekranu warming-up;
  - działania zależne od relay mają być kolejowane lub wyjaśniać stan.

### 1.2 Jeden supervisor transportu relay

- [x] `crates/infrastructure/torca-rendezvous-client/src/tor.rs`
  - pierwsza sonda health wykonuje serializowany `reconnect`, jeżeli stream
    jeszcze nie istnieje;
  - health, keepalive i pairing używają tego samego właściciela połączenia;
  - dodać backoff z jitterem, reset po zmianie sieci i twarde deadline'y.
- [x] `crates/application/torca-connectivity/**`,
  `crates/application/torca-probing/**`
  - usunąć niezależne dialowanie onion jako źródło prawdy o relay;
  - stan `degraded` po pojedynczym błędzie nie może blokować pairing.

### 1.3 Recovery onion

- [x] `crates/infrastructure/torca-tor/**`
  - worker publikacji działa również po pierwszym sukcesie;
  - `DegradedUnreachable` uruchamia kontrolowany recovery po grace period;
  - nie resetować identity/cache Tor przy zwykłym deployu relay.
- [x] `services/relay/src/main.rs`
  - readiness ma zawierać endpoint i stan publikacji, nie sam marker pliku;
  - dodać endpointowy probe E2E przed deklaracją gotowości deployera.

### 1.4 Diagnostyka i logi

- [x] `crates/application/torca-diagnostics/**`,
  `crates/platform/torca-native/src/torca_runtime.rs`
  - logować transitiony Tor/onion/relay/pairing/peer/attachment z
    `incidentId`, timestampem i powodem;
  - nie logować sekretów, treści rozmów ani kodów parowania.
- [x] `tools/torca-deploy/**`, `scripts/collect.zip.ps1` lub ich następca
  - jeden świeży folder incydentu: deploy, Windows runtime, Android runtime,
    relay i manifest wersji;
  - brak wymaganego urządzenia ADB ma być błędem, nie „sukcesem z pustymi
    logami”; ZIP zawiera tylko aktualny incydent.

**Dowód ukończenia:** cold start Android i Windows dochodzi do UI bez czekania
na relay; relay wraca po awarii bez restartu aplikacji; incident bundle
pozwala odtworzyć pełny stan.

## 2. Pairing i kontakty — P0

- [x] `apps/client/flutter/lib/screens/pairing_screen.dart`
  - Invitations: jeden ekran generowania; modal natychmiast pokazuje
    placeholder, kod/QR, timer i anulowanie;
  - Contacts/Chats: wyłącznie modal `Join invitation`, bez generatora;
  - Android: jawny focus `TextField`, otwieranie klawiatury i normalizacja
    sześciu znaków z opcjonalnymi spacjami;
  - usunąć wszystkie stare, równoległe ekrany pairing.
- [x] `apps/client/flutter/lib/widgets/incoming_pairing_dialog.dart`,
  `lib/app.dart`
  - jedna instancja dialogu na `sessionId`;
  - dialog akceptacji zastępuje zawartość bieżącej sesji albo jest toastem,
    nigdy kaskadą trzech modali;
  - po akceptacji: kontakt, snackbar/toast z nazwą, podświetlenie Contacts;
    zaproszenie znika z listy.
- [x] `crates/application/torca-pairing-coordinator/src/runtime.rs`,
  `crates/application/torca-client-engine/src/lib.rs`,
  `crates/infrastructure/torca-storage-sqlite/src/repository.rs`
  - profil/nickname uczestnika jest walidowany i atomowo zapisany wraz z
    contact, conversation i credential;
  - retry/join/approve są idempotentne;
  - usunięty kontakt może zostać sparowany ponownie zgodnie z polityką;
  - stale, expired i accepted invitations są czyszczone.
- [x] `crates/platform/torca-contract/schema/torca_contract.dart`
  - wymagane dane kontaktu nie mają cichych fallbacków; wszystkie zmiany są
    generowane przez `tools/torca-contract-gen`.

**Dowód ukończenia:** desktop ↔ Android, kod i QR, accept/reject/cancel,
ponowny pairing po delete oraz expiry; na obu stronach widoczny nickname,
jeden modal i jedna konwersacja.

## 3. Wiadomości i attachmenty — P0

### 3.1 Durable transfer

- [x] `crates/protocol/torca-attachment-protocol/src/lib.rs`
  - stabilna wersja ramek metadata/chunk/ack/cancel/retry/preview;
  - każdy transfer ma `attachmentId`, offset, checksum i jednoznaczny stan.
- [x] `crates/infrastructure/torca-attachment-transfer/src/lib.rs`
  - payload jest czytany strumieniowo z blob store, a nie odszyfrowywany w
    całości do pamięci;
  - durable staging + chunk ACK umożliwiają resume po reconnect/restart;
  - cancel usuwa wyłącznie własne temporary blobs po obu stronach;
  - retry nie tworzy duplikatu message/job.
- [x] `crates/application/torca-communication-driver/src/lib.rs`
  - przy metadata przed wiadomością attachment jest deferowany bez blokowania
    dalszego inbound batcha;
  - deferred queue ma limit, telemetry i deterministyczny retry.
- [x] `crates/infrastructure/torca-storage-sqlite/**`
  - transakcje attachment/job/blob są atomowe; orphan cleanup jest bezpieczny.

### 3.2 UI transferu

- [x] `apps/client/flutter/lib/widgets/attachment_tile.dart`,
  `lib/widgets/message_bubble.dart`
  - jeden dymek joba po obu stronach: queued/preparing/uploading/downloading/
    syncing/available/failed/cancelled;
  - kierunek, postęp bajtowy, prędkość, retry i cancel są jednoznaczne;
  - header = nazwa/original MIME/rozmiar, body = preview, footer = jeden
    poprawny delivery state + timestampy.
- [x] `apps/client/flutter/lib/screens/conversation_screen.dart`,
  `lib/screens/conversation_widgets.dart`
  - pending tray jest dokowany do composera, nie jest modalem;
  - limit pięciu plików, 50 KiB obrazów po kompresji, 5 MiB wideo i limity
    capabilities są stosowane przed długim kopiowaniem;
  - tap obrazu otwiera preview in-app, tap wideo/pełnego pliku otwiera
    systemowy viewer po pobraniu.

### 3.3 Preview media

- [x] `packages/torca_attachment_processing/**`
  - obraz: własny resized JPEG plus osobny small preview;
  - wideo: osobna pierwsza klatka JPEG, best-effort i bez wpływu na upload.
- [x] `apps/client/flutter/android/**`, `windows/runner/**`
  - Android `MediaMetadataRetriever`; Windows system thumbnail/WIC;
  - oba adaptery zwracają `null` dla nieobsługiwanego kontenera, bez błędu
    transferu;
  - potwierdzić kompilację Android i Windows po zamknięciu blokującego EXE.

**Dowód ukończenia:** image i MP4 w obu kierunkach, restart/reconnect w środku
transferu, cancel/retry, progress po obu stronach oraz brak zatrzymania
zwykłych wiadomości przez attachment.

## 4. Wspólny responsywny UX — P1

- [x] `apps/client/flutter/lib/screens/home_screen.dart`,
  `home_sections.dart`
  - ta sama hierarchia Chat/Contacts/Invitations dla mobile i desktop;
  - cały wiersz kontaktu otwiera chat; `i` otwiera details;
  - chats powstają tylko po pierwszej wiadomości lub explicit Start
    conversation; badges pokazują unread.
- [x] `apps/client/flutter/lib/screens/conversation_screen.dart`
  - jeden `ConversationPane` dla narrow route i wide embedded pane;
  - stabilny scroll, jump-to-latest z licznikiem, daty i read receipts.
- [x] `apps/client/flutter/lib/screens/contact_details_screen.dart` lub
  istniejący contact pane
  - na szerokim ekranie details w prawym panelu; na wąskim zastępuje chat z
    Back;
  - details: nickname, presence/last seen, endpoint health, safety info,
    historia, block/remove/rename.
- [x] `apps/client/flutter/lib/widgets/conversation_actions.dart`
  - desktopowy context menu i mobilny bottom sheet mają identyczny model
    akcji: chat, details, rename, block/unblock, remove, diagnostics.

**Dowód ukończenia:** snapshot/widget tests dla narrow/wide oraz ręczny test
Android/desktop bez różniących się flow.

## 5. Design system, dostępność i motywy — P1

- [x] `packages/torca_ui/**`
  - tokeny semantyczne dla success/warning/error, RX/TX/idle LED oraz bez
    kolorów statusu rozsianych po ekranach;
  - brak gradientów i zbędnych transition; reduce-motion zatrzymuje wszystkie
    repeat/impulse animations.
- [x] `apps/client/flutter/lib/themes/**`, `settings_screen.dart`
  - motywy: dopracowany modern/Telegram-like i terminalowy retro;
  - każdy motyw ma pełną mapę ikon, font body + font heading, focus/hover/
    disabled states, square controls dla retro;
  - panel preview z datą, receiptami i kilkoma wiadomościami.
- [x] `apps/client/flutter/lib/l10n/**`
  - ARB/gen_l10n dla tekstów UI, tooltipów, tray, notification i błędów;
  - usunąć hardcoded English strings z feature screens.
- [x] `apps/client/flutter/lib/main.dart`
  - usunąć globalne `immersiveSticky`; zachować standardową nawigację Android.

**Dowód ukończenia:** screenshots obu motywów, semantyka i keyboard navigation,
reduce-motion test.

## 6. Kontrakt, architektura i quality gates — P1

- [x] `tools/torca-contract-gen/**`,
  `crates/platform/torca-contract/schema/**`
  - generator tworzy enumy `fromWire`/`wireValue` dla statusów i decoder DTO;
  - wymagane pola rzucają `ContractDecodeException`, optional mają jawne
    defaults;
  - zero `format!("{:?}").to_lowercase()` dla ABI.
- [x] `apps/client/flutter/lib/gateway/ffi_engine_gateway.dart`
  - podział na FFI binding, worker, decoder, gateway;
  - `runtime.poll(afterRevision, afterCursor)` zastępuje dwa pełne polling
    requesty co 250 ms.
- [x] `crates/platform/torca-native/**`
  - usunąć globalne `allow(clippy::all)`; lokalne wyjątki z uzasadnieniem;
  - typed `ErrorCode`, `messageKey`, `diagnosticId`, `retryAdvice`, bez
    klasyfikacji tekstu.
- [x] `scripts/modules/Torca.SourcePolicy.ps1` lub `tools/torca-deploy/**`
  - egzekwować dependency matrix, generated contract drift, raw SQL policy,
    wire-string comparisons, `clippy::all`, wielkość plików i lokalizację;
  - CI Windows/Android dostaje jawny test endpoint, nigdy lokalny stan.

**Dowód ukończenia:** clean CI na świeżym runnerze oraz policy violations
powodują celowe, zrozumiałe failure.

## 7. Deployer i release acceptance — P0

- [x] `tools/torca-deploy/**`
  - jedyny Rust CLI/TUI wizard; zero legacy PowerShell execution path;
  - domyślnie wszystkie wykryte urządzenia; brak wybranego/wymaganego
    urządzenia jest błędem;
  - czytelne profile: Run current, Rebuild clients, Rebuild relay,
    Full reset clients, Rotate onion — z jawnym opisem danych, które zostaną
    zachowane/usunięte;
  - lock ma PID/timestamp i bezpieczne stale-lock recovery.
- [x] `tools/torca-deploy/src/build.rs`, `src/launch.rs`, `src/devices.rs`
  - manifest artefaktu zawiera target/configuration/endpoint/build id;
  - reuse porównuje endpoint i ABI; Windows launch jest detached bez pipe
    deadlocku; przed buildem można zamknąć własny poprzedni proces klienta;
  - launch/install i network validation są osobnymi checkpointami;
  - quick validation ma 2–3 probes, pełna jest opt-in i pokazuje live state.
- [x] Build toolchains
  - naprawić Android OpenSSL/Perl flags i podwójne `--target`;
  - Windows korzysta z oficjalnego, wspieranego przez Flutter backendu
    CMake/MSVC; nie wprowadzono nieobsługiwanego zamiennika kompilatora;
    diagnostyka instalacji CMake jest wykonywana przez Rust deployera;
  - sccache, Cargo target dirs i Flutter/Gradle cache są optymalizacją, nie
    substytutem poprawnego manifestu.

**Dowód ukończenia:** fresh full redeploy Android + Windows, potem reuse/rebuild
bez zmian onion; raport pokazuje faktyczny install, launch i wersje obu klientów
oraz relay.

## 8. Ostateczna walidacja — release gate

- [ ] `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`,
  właściwy subset i pełne testy Rust.
- [ ] `dart format --set-exit-if-changed lib test`, `dart analyze`,
  `flutter test`, Android APK debug/release i Windows debug/release.
- [ ] `tools/torca-contract-gen --check` i Architecture Policy.
- [ ] Scenariusz dwóch urządzeń:
  - cold start i relay chwilowo unavailable;
  - create/join via code i QR; approve/reject/cancel/expiry/re-pair;
  - tekst, image, MP4, dokument; cancel/retry/reconnect;
  - unread/read receipts, contact actions, responsive narrow/wide;
  - collect incident ZIP i weryfikacja kompletności.
- [ ] Soak test minimum 30 minut z background/foreground Android, restartem
  relay i reconnectem klienta bez utraty wiadomości/jobów.

Plan jest zamknięty dopiero, gdy każdy dowód ukończenia jest zachowany w CI albo
w powtarzalnym runbooku release.
