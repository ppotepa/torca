# IROH CPU/BATTERY — domknięcie wydajności runtime

## Stan po przerwanym commicie

Zaimplementowane i zweryfikowane testami automatycznymi: event-driven startup
zamiast stałego pollingu, source-specific wake gates z coalescingiem, bounded
peer-recovery backoff, brak pending-spin przed podpięciem runtime, provider
descriptors Tor/Iroh, wspólny `DeployPlan`/preflight/planned steps, fingerprint
checkpointu, cancellation/retry oraz themed execution dashboard. Workspace:
442 testów Rust, `cargo check --workspace` i clippy przechodzą.

Niepotwierdzone empirycznie: rzeczywisty profil ~10% CPU na Windows, idle
screen-off na Androidzie oraz porównanie Iroh/Tor na tych samych urządzeniach.
Harness do pomiaru Windows CPU i macierzy Android Tor/Iroh jest już dostępny,
ale nie został uruchomiony bezpośrednio na urządzeniach w tym środowisku.
Do rozdzielenia aktywności per-thread i wzorców wake dodano również
`capture-iroh-windows-etw.ps1` oraz `capture-iroh-android-perfetto.ps1`.
Macierz baterii odrzuca teraz przebieg po zmianie zanonimizowanego fingerprintu
sieci przed/po pomiarze albo między wariantami.
`summarize-iroh-energy.ps1` tworzy końcowy raport wyłącznie wtedy, gdy obecne
są Tor oraz wszystkie trzy profile Iroh, każdy z minimum trzema stabilnymi
przebiegami; brak danych nie jest interpretowany jako przewaga Iroh.
Logika wyboru akcji/opcji jest już własnością modelu i ekranu kontekstowego;
fasada `tui.rs` tylko uruchamia selektor akcji i przekazuje plan do ekranu.
Nieukończone w sensie produktu pozostaje zebranie raportu z minimum trzech
rzeczywistych powtórzeń każdego wariantu. Dashboard wykonawczy jest już
wydzielony do `tui/app.rs`; na ekranie błędu `d` zbiera diagnostykę, a `l`
zbiera bounded log bundle, bez zmiany checkpointu deployu.

Windowsowy pomiar procesu wykonuje `scripts/measure-iroh-cpu.ps1`. Przykład:

```powershell
.\scripts\measure-iroh-cpu.ps1 -ProcessName torca_app -Profile direct `
  -Mode foreground -DurationSeconds 300 `
  -Output .torca/measurements/iroh-direct-foreground.json
```

Exit code `0` oznacza spełnienie progu, a `2` jego przekroczenie. Pomiar nie
uruchamia ani nie restartuje klienta — proces musi być wcześniej uruchomiony
w wybranym providerze/profile.

## Problem i zasada nadrzędna

Ostatnio uruchomiony klient Windows zużywał około 10% CPU w sposób ciągły.
To jest błąd blokujący wydanie i potencjalny sygnał wysokiego zużycia energii
na Androidzie. Nie zakładamy, że winny jest sam Iroh ani że Iroh zawsze zużywa
mniej energii niż Tor. Najpierw identyfikujemy konkretny wątek, task, deadline
lub źródło wake, a dopiero potem zmieniamy implementację.

Docelowy runtime pozostaje event-driven:

```text
command / provider event / platform event / durable deadline
                         ↓
               pojedynczy skojarzony wake
                         ↓
              tylko właściciel źródła pracy
                         ↓
                  ponowne zaśnięcie
```

Nie naprawiamy problemu przez dodanie arbitralnego `sleep`, globalnego pollingu
ani zwiększenie interwałów. Takie rozwiązanie ukrywa pętlę i pogarsza
responsywność.

## Obecny stan

Już zaimplementowano:

- centralny scheduler deadline'ów i source-selective maintenance;
- hot-loop diagnostic dla szybkich obrotów `RuntimeOwner`;
- koaleskowanie wake dla communication, lifecycle i Radio;
- Flutter revision waiter oparty na condvar z pollingiem tylko jako fallback;
- demand-driven, single-flight Iroh online probe ograniczony do trzech prób;
- Iroh `direct/local` bez relay i address lookup;
- krótką background grace i soft dormancy;
- bounded inbound queues, bounded connect/read/write oraz batchowanie wiadomości;
- provider route generation, stale-route rejection i jawny route refresh;
- diagnostykę battery/runtime oraz fizyczny soak harness.

To nie dowodzi poprawnego idle. Szczególnie wymagają sprawdzenia:

- deadline stale równy `now` lub przeterminowany i ponownie publikowany;
- `active_transport` utrzymujący 250-ms peer recovery tick bez końca;
- callback wywoływany ponownie podczas opróżniania własnej kolejki;
- nieprecyzyjne czyszczenie flag `*_wake_pending` po zbiorczym `Wake`;
- Flutter waiter wpadający w fallback polling albo produkujący rewizje bez zmian;
- wewnętrzne taski Iroh/N0 relay/address lookup;
- render/rebuild loop Flutter niezależny od natywnego runtime;
- debug logging lub diagnostics zapisujące dane przy każdym obrocie.

## Bramka 0 — zabezpieczenie reprodukcji

Przed zmianą kodu należy zachować dokładną reprodukcję:

1. Zapisać commit SHA, dirty diff, build ID, tryb debug/release, provider i profil.
2. Zanotować liczbę kontaktów, pending messages, attachments i stan Radio.
3. Uruchomić klient przez co najmniej pięć minut bez interakcji.
4. Zebrać zużycie CPU per thread, a nie tylko całego procesu.
5. Zachować log zawierający `torca-runtime: hot-loop suspected`, runtime snapshot
   oraz provider diagnostics.
6. Powtórzyć po restarcie na czystym profilu klienta.

Minimalna macierz izolacyjna Windows:

| Wariant | Cel |
| --- | --- |
| Flutter bez uruchomionego native runtime | wykrycie render/UI loop |
| headless runtime + provider-memory | scheduler, storage i peer-link bez sieci |
| headless Iroh `local` | koszt endpointu bez discovery/relay |
| headless Iroh `direct` | bez discovery/relay, pełny direct route |
| headless Iroh `always` | koszt N0 relay/address lookup |
| headless Tor | punkt odniesienia |
| pełny Flutter + każdy powyższy provider | koszt FFI i UI |

Każdy wariant mierzymy bez kontaktów oraz z jednym gotowym kontaktem. Jeśli CPU
pojawia się dopiero po kontakcie, osobno sprawdzamy stan połączenia
`connecting`, `handshaking`, `ready`, `reconnecting`.

## Etap 1 — obserwowalność bez okresowego kosztu

Rozszerzyć istniejącą diagnostykę o liczniki monotoniczne odczytywane na żądanie:

- obroty `RuntimeOwner` per wake source;
- czas oczekiwania przed każdym obrotem w kubełkach;
- liczbę deadline'ów publikowanych jako `0 ms`;
- liczbę kolejnych identycznych deadline'ów tego samego właściciela;
- wywołania i odrzucone wywołania każdego waker gate;
- liczbę `maintain_*` per subsystem;
- peer status i powód aktywnego 250-ms recovery tick;
- runtime revision increments oraz revision-wait wakeups;
- Flutter fallback polls, snapshot decode i realne snapshot changes;
- Iroh endpoint generation, task/probe counters i profile;
- zapisy diagnostics/event log per minute.

Liczniki nie mogą posiadać własnego timera. Snapshot pobiera je jawnie z Debug
console albo soak harnessu. Dodać eksport per-thread dla Windows ETW/WPR oraz
Android Perfetto/simpleperf, żeby rozdzielić:

- Flutter UI/raster;
- Dart worker isolate;
- `torca-runtime`;
- `torca-iroh-*` Tokio workers;
- SQLite;
- systemowe wątki QUIC/network watcher.

## Etap 2 — usunięcie hot-loopów runtime

### 2.1 Scheduler

- Każdy executor zwracający deadline `<= now` musi albo wykonać postęp, albo
  jawnie wycofać deadline/zwrócić bounded backoff.
- Po `take_due` źródło nie może zostać ponownie wpisane z zerowym opóźnieniem,
  jeśli jego stan i licznik postępu się nie zmieniły.
- Dodać debug assertion i test deterministyczny dla stale deadline.
- `None` pozostaje jedynym poprawnym stanem całkowitego idle.

### 2.2 Wake gates

- Czyścić tylko flagę odpowiadającą źródłom odebranym w danym `Wake`.
- Nie czyścić communication, lifecycle i Radio razem po dowolnym wake.
- Waker ma być edge-triggered: kolejny callback przed opróżnieniem pracy jest
  koaleskowany, a callback podczas maintenance może zaplanować dokładnie jeden
  kolejny obrót.
- Dodać test burzy 100 000 callbacków: liczba obrotów ma pozostać ograniczona,
  a ostatni event nie może zaginąć.

### 2.3 Peer recovery tick

- 250-ms tick może istnieć tylko podczas udokumentowanego przejścia
  `connecting/handshaking/reconnecting`.
- Każde przejście musi mieć terminalny timeout, generation i licznik postępu.
- `ready`, `failed`, brak demand lub brak peerów natychmiast usuwa tick.
- Dodać regresję wielogodzinną z zegarem wirtualnym: kontakt bez pracy nie może
  utrzymać recovery tick.

### 2.4 Mailbox i storage

- Usunąć busy-yield w `send_with_timeout`; zastosować blokującą, bounded
  semantykę kanału albo condvar bez spinowania przy pełnym mailboxie.
- Potwierdzić, że runtime command/query timeout nie powoduje automatycznej
  lawiny ponowień po stronie Flutter.
- Żaden pusty maintenance nie może wykonywać zapytania SQLite ani zapisu
  diagnostics.

## Etap 3 — Flutter/FFI

- Revision waiter jest podstawową ścieżką; fallback polling musi być widoczny
  w diagnostyce i po kilku błędach przejść w bounded degraded state.
- Runtime revision zmienia się tylko po zmianie publicznego snapshotu lub
  pojawieniu się eventu, nie po samym odczycie/poll.
- Jeden revision wake może uruchomić jeden `runtime.poll`; równoległe poll są
  niedozwolone.
- Identyczny snapshot nie wywołuje event stream ani rebuild Flutter.
- Sprawdzić `runtime_network_status`, Home, diagnostics i lifecycle listeners
  pod kątem notify/rebuild loop.
- Test z fake native waiter: 30 minut bez rewizji daje zero poll i zero UI
  rebuild po pierwszym renderze.

## Etap 4 — Iroh provider

### 4.1 Runtime i task ownership

- Sprawdzić profilem per-thread, czy cztery desktopowe Tokio workers są
  faktycznie uśpione. Liczba wątków nie jest sama w sobie problemem; aktywny
  spin któregoś workera jest problemem.
- Rozważyć domyślnie dwa workery również na desktop dopiero po benchmarku
  throughput/Radio. Nie zmniejszać liczby wątków jako substytutu naprawy pętli.
- Każdy spawned task musi mieć właściciela, cancellation path i terminalny stan.

### 4.2 Profile

- `local`: zero relay, lookup i online probe; UDP listener może pozostać bound.
- `direct`: jak `local`, ale ze stabilnym pełnym route i jawnym refresh.
- `always`: relay/discovery wyłącznie gdy wymaga tego reachability lease;
  sprawdzić, czy samo utworzenie endpointu nie utrzymuje kosztownego tasku mimo
  braku demand.
- Nie przełączać profilu dynamicznie bez przebudowy artefaktu; profil pozostaje
  częścią fingerprintu i manifestu.

### 4.3 Route i reconnect

- Po network change zachować aktywne migratable QUIC sessions.
- Nie uruchamiać all-contact reconnect.
- Direct/local bez authenticated session pokazuje jawny route-refresh/re-pair,
  bez okresowego retry starego adresu.
- Dodać test Wi-Fi/LTE z liczbą dial/probe/wake przed i po zmianie route.

### 4.4 Radio

- Brak aktywnej sesji oznacza brak media worker deadline i keepalive.
- `requires_application_keep_alive=false` dla Iroh musi faktycznie wyłączać
  heartbeat aplikacyjny.
- Worker oczekujący na inbound stream blokuje się na Notify/QUIC, nie polluje.

## Etap 5 — polityka baterii i semantyka produktu

Nie utożsamiać providera z profilem baterii:

```text
provider: Tor | Iroh
Iroh endpoint profile: always | direct | local
runtime battery policy: automatic | always-available | battery-saver
durable demand: message | attachment | pairing | radio
```

Decyzję o reachability podejmuje `RuntimeGovernor`; provider wykonuje ją przez
neutralny lifecycle. Iroh nie może posiadać drugiego, ukrytego schedulera
polityki aplikacji.

Oczekiwane znaczenie:

- `automatic`: foreground i durable work aktywują provider; background idle
  po grace przechodzi w dormancy;
- `battery-saver`: brak kosmetycznych probe, ograniczone transfery i szybka
  dormancy;
- `always-available`: świadomie utrzymuje incoming reachability i może zużywać
  więcej energii;
- `direct/local`: niższy koszt kosztem niezawodnej osiągalności w tle.

## Etap 6 — fizyczna walidacja

### Windows

Najpierw wymagane jest zejście z raportowanych ~10% CPU:

- pełny klient idle foreground: mediana procesu < 1% jednego logicznego CPU;
- minimized/background: mediana < 0,25%;
- brak trwałego wątku z aktywnością > 0,1% bez pracy;
- zero hot-loop warnings;
- zero deadline wake po grace, gdy nie ma durable work;
- wyniki powtarzalne w debug i release, z osobnymi progami dla debug.

### Android

Na fizycznym urządzeniu, unplugged i screen-off:

- 30 minut jako szybka bramka regresji;
- 6–8 godzin jako evidence release;
- zero app-controlled periodic wake w idle;
- zero reconnect/probe/contact scan/DB polling po grace;
- Perfetto bez cyklicznego CPU/network pattern po stronie Torca;
- zachowane `battery-start`, `battery-end`, `batterystats`, diagnostics i incident bundle.

Macierz A/B na tym samym urządzeniu i tej samej sieci:

| Provider/profile | Reachability | Cel pomiaru |
| --- | --- | --- |
| Tor | managed onion | punkt odniesienia privacy/reachability |
| Iroh `always` | relay/discovery | porównywalna osiągalność |
| Iroh `direct` | out-of-band route | minimalizacja idle bez relay |
| Iroh `local` | lokalna | dolna granica kosztu provider runtime |

Nie porównywać absolutnego procentu baterii między różnymi telefonami, radiami
lub temperaturą. Każdy wariant wykonać co najmniej trzy razy i raportować
medianę oraz rozrzut.

## Etap 7 — dokończenie DEPLOY1 dla kontroli eksperymentu

Po usunięciu hot-loopu dokończyć deployer tak, aby nie dało się przypadkiem
porównywać różnych artefaktów:

- provider descriptor jako jedyne źródło profili i maintenance;
- profile Iroh w review, manifeście i fingerprintcie;
- rzeczywiste `--preflight`, `--show-steps`, `--theme`, `--no-color`;
- device preflight bez mutacji;
- review pokazujący dokładny normalized plan przekazywany executorowi;
- execution dashboard i checkpoint retry;
- testy TUI 80×24, 100×30, 120×30 i 180×45;
- zapis UI settings bez wpływu na semantykę planu.

## Kolejność commitów

1. `perf: add zero-cost runtime wake diagnostics`
2. `fix: make runtime wake gates source-specific`
3. `fix: terminate stale peer recovery deadlines`
4. `fix: keep Flutter revision delivery fully event-driven`
5. `fix: make Iroh idle tasks demand-owned and cancellable`
6. `test: add deterministic long-idle and wake-storm regressions`
7. `test: add Windows provider CPU comparison harness`
8. `test: retain Android Tor/Iroh battery evidence`

## Audit status after implementation

Local implementation status: runtime, Iroh lifecycle, diagnostics, Flutter
bridge, deploy plan, contextual TUI, checkpoint retry and measurement scripts
are implemented. The peer recovery fallback is bounded to a 30 second window;
endpoint migration has a 30 second timeout and stale generations are rejected.

Verification status: 442 Rust tests pass across 134 suites, workspace check and
Clippy pass, Flutter tests/analyze pass, PowerShell scripts parse, and the
CodeGraph index is synchronized. Physical Windows CPU and Android battery
evidence is still required before claiming the release thresholds or an Iroh
energy advantage over Tor.

The Flutter revision waiter also guards disposal: a cancelled waiter cannot
schedule a fallback poll after the native handle has been released.

The native FFI bridge no longer busy-yields while its bounded mailbox is full;
it sleeps briefly between attempts and preserves the terminal enqueue timeout.

The client-engine mailbox follows the same bounded backpressure rule, so a
Flutter request burst cannot turn a full single-writer queue into a CPU spin.

The idle Radio worker now blocks on its command lane instead of using a
one-second defensive timeout; inbound listener callbacks and shutdown remain
the wake sources.

The client-engine mailbox also uses bounded blocking under backpressure; the
production runtime paths no longer use `yield_now()` retry loops.

The energy aggregator now requires battery start/end/drop values for every
matrix run; stable process exit and network state alone cannot produce a
complete energy verdict.

The Android matrix now resolves the nested `torca-soak` run directory and
checks its manifest provider/profile before accepting battery samples. A
successful process exit can no longer be attributed to the wrong Iroh profile.

Provider maintenance is also provider-only: normalization and the executor
skip client build, installation, data reset and launch for that action. This
prevents a maintenance run from spending CPU or battery on unrelated client
work.

PeerLink now applies the same terminal recovery window at the adapter boundary,
so its `next_maintenance_delay()` cannot silently reintroduce an endless
250-ms deadline after the central runtime recovery window expires.

Latest local validation count: 442 Rust tests across 134 suites.

When the peer recovery window expires, PeerLink now closes stuck non-ready
sessions and schedules a normal backoff reconnect; it does not leave a dead
handshake retained forever.
9. `deploy: finish contextual provider-aware wizard`
10. `docs: record measured provider energy trade-offs`

Każdy commit ma przechodzić fmt, workspace check, Clippy, Rust tests, Flutter
analyze/tests i odpowiedni headless idle gate. Nie łączyć wszystkich napraw w
jeden commit, ponieważ profilowanie musi wskazać, która zmiana faktycznie usuwa
koszt CPU.

## Kryteria ukończenia

Praca jest zakończona, gdy:

- źródło 10% CPU jest wskazane profilem i usunięte testem regresyjnym;
- idle scheduler ma `next deadline = none` bez durable work;
- Flutter nie polluje i nie renderuje bez revision/event;
- Iroh direct/local nie uruchamia relay/discovery/online probe;
- Iroh always nie wykonuje aplikacyjnego reachability work bez demand;
- network migration nie tworzy reconnect storm;
- Windows spełnia bramkę CPU;
- Android ma zachowane, powtarzalne evidence dla Tor i trzech profili Iroh;
- dokumentacja opisuje wynik pomiaru, a nie deklarowaną klasę energii;
- DEPLOY1 gwarantuje, że mierzony provider/profile odpowiada artefaktowi.
