# Audyt CPU i energii Torca — Windows i Android

Data: 2026-08-27  
Zakres: desktop Windows, emulator Android API 36, analiza avatarów, Iroh `direct`/`always`, foreground/background, ślady Perfetto i Simpleperf oraz historyczny soak fizycznego telefonu.

## Werdykt

Zgłaszane około 10% CPU na desktopie jest rzeczywiste. W bieżącym debug buildzie Iroh `always` proces zajmował medianowo 8,29% całej 16-wątkowej maszyny przy widocznym oknie oraz 8,98% po minimalizacji i 20 sekundach stabilizacji. Minimalizacja nie usuwa obciążenia, więc rasteryzacja, animacje i avatary nie są głównym źródłem.

Na Windows głównym różnicującym czynnikiem jest profil Iroh:

| Windows release, idle | CPU / 1 logiczny CPU | CPU / całą maszynę | Relacja |
| --- | ---: | ---: | ---: |
| Iroh `direct` | 10,26% | 0,641% | 1,0× |
| Iroh `always` | 115,80% | 7,237% | 11,29× |

Przejście z `always` na `direct` zmniejszyło CPU całej maszyny o 91,1%. `always` utrzymywał połączenia relay HTTPS i UDP; `direct` nie utrzymywał połączeń relay. Test wyłącza jednocześnie relay oraz address lookup, dlatego na tym etapie nie rozdziela ich kosztów między sobą.

Android ma dodatkowy, niezależny błąd. Release `direct` i `always` zużywały około 108–111% jednego rdzenia zarówno na pierwszym planie, jak i przy wygaszonym ekranie. Profil Iroh praktycznie nie zmieniał wyniku. Po zatrzymaniu wyłącznie testowej instancji `TorcaForegroundService` ten sam proces spadł z 110% do wartości raportowanej przez `top` jako 0% (poniżej rozdzielczości próbkowania).

Profil stosów wskazał konkretny łańcuch:

```text
TorcaForegroundService / TorcaNotificationPoller
    -> nativeWaitForRevision() wraca natychmiast
    -> re-arm co 100 ms
    -> notifications.poll
    -> collect_notification_events()
    -> conversation summaries z SQLite
    -> pełny application snapshot
    -> network/runtime maintenance
    -> Radio projection
    -> CPAL/AAudio enumeration
    -> JNI + AudioManager + AudioFlinger Binder
```

Bezpośredni błąd logiczny znajduje się w semantyce waitera. `RuntimeEventHub.cursor` otrzymuje rewizję runtime’u, ale Android przekazuje do `wait` trwały `notificationCursor`. Są to dwa różne liczniki. Po starcie `hub.cursor > notificationCursor` pozostaje prawdziwe, więc waiter nie blokuje się i poller uruchamia kosztowny pełny snapshot co 100 ms.

Avatary nie są przyczyną tej reprodukcji. Biblioteka już używa spritesheetów, ma jeden współdzielony zegar klatek i ograniczony cache. Co ważniejsze, mierzone preferencje miały `reduce_motion=true` i `battery.visual_activity=static`, a CPU nie spadł ani po minimalizacji desktopu, ani po wygaszeniu Androida. Zatrzymanie usługi Androida usunęło CPU bez zmiany UI lub assetów.

## Status dowodów

| Hipoteza | Wynik | Pewność |
| --- | --- | --- |
| GIF-y/animowane avatary zużywają 10% CPU | Odrzucona dla zmierzonej konfiguracji | Wysoka |
| Minimalizacja/wyłączenie renderowania usuwa problem desktopu | Odrzucona | Wysoka |
| Debug build sam powoduje desktopowe 10% | Odrzucona | Wysoka |
| Iroh `always` jest głównym kosztem desktopu | Potwierdzona przez release A/B | Wysoka |
| Iroh `always` jest głównym kosztem Androida | Odrzucona jako główna przyczyna | Wysoka |
| Android foreground service uruchamia kosztowny pipeline | Potwierdzona przez stosy i service-stop A/B | Wysoka |
| Błąd waitera miesza dwa kursory | Potwierdzony w kodzie i zgodny z profilem | Wysoka |
| Bieżący Iroh zużywa określoną liczbę mAh na fizycznym telefonie | Nieustalone | Wymagany nowy soak |

## Metodyka

Wszystkie procesowe wartości CPU znormalizowano tak, że 100% oznacza pełne wykorzystanie jednego logicznego CPU. Na Windows wynik „cała maszyna” dzieli tę wartość przez 16 logicznych CPU. Emulator miał cztery vCPU, więc 110% jednego CPU odpowiada około 27,5% jego całkowitej logicznej pojemności obliczeniowej. Procenty hosta Windows i emulatora nie są bezpośrednim pomiarem energii.

Kontrole eksperymentu:

- buildy `release` sprawdzono w manifeście i przez brak flagi `debuggable` w APK;
- profile `direct` i `always` zbudowano osobno przez `torca-deploy`;
- każdą próbkę zbierano po rozgrzaniu procesu;
- foreground/background weryfikowano przez `dumpsys power`, a błędną próbkę oznaczono jako nieważną;
- Android `always` powtórzono po wyczyszczeniu danych wyłącznie testowego AVD;
- top threads odczytywano z `/proc/<pid>/task/<tid>/comm`;
- Android release profilowano dodatkowo przez Perfetto, Simpleperf z callchain oraz `strace -c`;
- nie logowano endpoint bytes, adresów IP ani onion address w raporcie.

Kompletny, generowany automatycznie indeks wszystkich plików i tabel znajduje się w [.torca/measurements/ENERGY_AUDIT_EVIDENCE.md](../../.torca/measurements/ENERGY_AUDIT_EVIDENCE.md).

## Desktop Windows

### Reprodukcja bieżącego procesu

Bieżący proces był debug buildem Iroh `always`. Pomiar 30 sekund dał medianę 135,24% jednego CPU i maksimum 193,37%. Na 16 logicznych CPU odpowiada to mniej więcej 8–12% całej maszyny, zgodnie ze zgłoszeniem użytkownika.

| Stan okna | Warm-up | Mediana / 1 CPU | P95 / 1 CPU | Mediana / maszyna | P95 / maszyna |
| --- | ---: | ---: | ---: | ---: | ---: |
| widoczne | standardowy | 132,65% | 157,68% | 8,29% | 9,86% |
| zminimalizowane | 2 s | 138,81% | 164,79% | 8,68% | 10,30% |
| zminimalizowane | 20 s | 143,65% | 168,03% | 8,98% | 10,50% |

Brak spadku po 20 sekundach wyklucza sytuację, w której Flutter potrzebował jedynie krótkiego czasu na wejście w stan offscreen. W pięciu najgorętszych wątkach rozkładało się niemal całe obciążenie; nazwy natywnych wątków nie były dostępne w Windows API dla tego procesu.

### Release A/B

Release `direct` zajmował 0,641% całej maszyny, natomiast release `always` 7,237%. To wyklucza interpretację, że obserwacja wynika tylko z debug assertów, JIT lub logowania debug builda.

W trakcie pomiaru:

- `direct` miał ruch UDP, ale nie miał aktywnych połączeń TCP do relay;
- `always` miał trzy aktywne sesje TCP/443 do relay oraz ruch UDP;
- implementacja profilu `always` wybiera N0 relay i address lookup;
- `direct`/`local` korzystają z minimal preset, czyszczą address lookup i wyłączają relay.

Najbardziej prawdopodobny właściciel desktopowego kosztu to zatem lifecycle/task set N0 aktywowany przez relay/discovery w `always`. Obecny A/B nie dowodzi, czy większość CPU bierze sam relay, watcher adresów, czy ich interakcja — do tego potrzebny jest jeszcze build izolujący każdą funkcję osobno lub ETW ze stosami.

Windows ETW/WPR został uruchomiony, ale system odmówił profilu kernel CPU bez podniesionych uprawnień (`0xc5585011`, policy to profile system performance). Ten brak jest jawny; wyniku per-symbol Windows nie należy udawać na podstawie samych nazw wątków.

## Avatary i renderowanie

Kod avatarów już spełnia postulowaną architekturę spritesheet:

- [`avatar_repository.dart`](../../packages/torca_avatar/lib/src/avatar_repository.dart) definiuje `AvatarSpriteSheet`, deduplikuje in-flight generation i utrzymuje LRU cache spritesheetów do 12 MiB;
- [`avatar_animation.dart`](../../packages/torca_avatar/lib/src/avatar_animation.dart) ma jeden współdzielony `AvatarFrameClock`;
- normalny widoczny avatar ma krok 250 ms (4 fps), focused 100 ms (10 fps);
- `staticOnly`, reduce-motion, background i wyłączony `TickerMode` zatrzymują animację;
- bieżące preferencje desktopu podczas testu: `appearance.reduce_motion=true`, `battery.mode=battery_saver`, `battery.visual_activity=static`.

Trzy niezależne obserwacje odrzucają avatar jako źródło zmierzonego idle CPU:

1. animacja była już wyłączona przez preferencje;
2. minimalizacja desktopu nie zmniejszyła CPU;
3. na Androidzie wygaszenie ekranu nie zmniejszyło CPU, natomiast zatrzymanie foreground service zmniejszyło je do poziomu nieraportowalnego przez `top`.

Spritesheet pozostaje dobrym wyborem produktowym, ale kolejna migracja assetów nie naprawi obecnego drenażu.

## Android

### Macierz CPU

| Build | Profil | Stan | Mediana / 1 CPU | P95 | Udział 4-vCPU emulatora |
| --- | --- | --- | ---: | ---: | ---: |
| debug | `always` | foreground | 127% | 136% | 31,75% |
| debug | `always` | screen-off | 127% | 132% | 31,75% |
| release | `direct` | foreground | 111% | 117% | 27,75% |
| release | `direct` | screen-off | 108% | 112% | 27,00% |
| release | `always` | foreground | 110% | 114% | 27,50% |
| release | `always` | screen-off | 109% | 119% | 27,25% |
| release | `always`, czysty profil | screen-off | 110% | 114% | 27,50% |
| release | `always`, service stopped | screen-off | <1% / raport 0% | 0% | <0,25% |

Jedna debug próbka nazwana `direct-foreground` miała w rzeczywistości wygaszony ekran i została automatycznie oznaczona jako nieważna. Nie jest używana w porównaniu.

Różnica `direct`–`always` na Androidzie mieści się w szumie próbki. Oznacza to, że koszt wspólnego pipeline’u aplikacji dominuje nad różnicą profilu Iroh. Nie przeczy to desktopowemu kosztowi `always`; są to dwa równoległe problemy.

Do powtarzalnej automatyzacji emulatora służy
[`run-android-emulator-cpu.ps1`](../../scripts/run-android-emulator-cpu.ps1).
Skrypt uruchamia AVD, instaluje release APK, wymusza screen-off, wykonuje
ustaloną liczbę pomiarów i gwarantuje zatrzymanie emulatora. Self-test 1×5 s
(2026-08-27) dał medianę/P95 `0%/0%` CPU na logiczny CPU; wynik JSON znajduje się
w `.torca/measurements/android-emulator/summary.json`.

Po zbudowaniu bieżącego Windows Release wykonano również pomiar rzeczywistego
procesu `torca_app`: stan normalny był ważny (mediana `0%`, P95 `0,9683%`
całej maszyny; `15,493%` jednego logicznego CPU). Pomiar po minimalizacji dał
zero próbek CPU i został prawidłowo oznaczony jako nieważny przez bramkę —
Windows wstrzymał proces, więc nie jest to dowód na zerowy koszt aplikacji.

Android foreground service nie wykonuje już cyklicznego odpytywania co 1,5 s.
Kotlin korzysta z blokującego, anulowalnego JNI `nativeWaitForNotification` i
wykonuje projekcję kursora dopiero po zdarzeniu. Krótki self-test nowego APK po
tej zmianie nadal dał `0%/0%` mediany/P95 CPU w screen-off emulatorze.

### Hot threads z Simpleperf

15-sekundowy release trace w tle zawierał 35 422 próbki i zero utraconych próbek. Udziały root callchain:

| Wątek | Udział próbek | Główna ścieżka |
| --- | ---: | --- |
| `Thread-17` | 35,14% | Android notifications → snapshot → Radio audio enumeration/JNI |
| `torca-runtime-owner` | 22,38% | runtime loop, deadline/maintenance, odczyty czasu i DB |
| `Thread-16` | 14,99% | `TextDeliveryBridge` → delivery maintenance → SQLite |
| `torca-client-engine` | 12,92% | engine mailbox i trwałe projection/storage queries |
| drugi `Thread-16` | 10,65% | kolejny worker maintenance/SQLite |

Lokalny niestripowany `libtorca_native.so` pozwolił zmapować adresy na symbole. Najważniejszy callchain `Thread-17`:

```text
TorcaRuntime::notification_events_json
ActorState::invoke
TorcaRuntime::refresh_snapshot
ClientApplicationRuntime::snapshot_context
SharedRadioCoordinator::projection
RadioCoordinator::projection
PlatformAudio::devices
cpal::aaudio::Device::supports_output
AudioManager::get_frames_per_buffer
JNIEnv::call_method
android.media.AudioManager.getProperty
AudioFlinger Binder ioctl
```

Wątek delivery mapował się między innymi na:

```text
TextDeliveryBridge worker
TextWorkerAdapter::maintenance
DeliveryWorker::run_once_with_observer
SqlCipherDurableStore::claim_due
rusqlite prepare
SQLite parser/update
```

### Liczba syscalli

Pełne `strace -c -f` przez 10 sekund zarejestrowało:

| Syscall | Wywołania / 10 s | Przybliżenie / s |
| --- | ---: | ---: |
| `clock_gettime` | 20 820 | 2 082 |
| `futex` | 18 197 | 1 820 |
| `fcntl` | 11 516 | 1 152 |
| `ioctl` | 3 684 | 368 |

Per-thread, przez osiem sekund:

- notification/native thread: 4 968 `clock_gettime`, 5 664 `ioctl`, 1 787 `fcntl`;
- `torca-runtime-owner`: 14 513 `clock_gettime`, 3 884 `futex`, 1 691 `fcntl`;
- pierwszy delivery worker: 3 136 `futex`, 2 790 `fcntl`;
- drugi worker: 3 463 `futex`, 4 143 `fcntl`.

Na emulatorze x86_64 `clock_gettime` wchodził przez `read_hpet`, co zawyża koszt względem typowego ARM telefonu. Nie zmienia to faktu, że liczba wywołań jest błędnie wysoka i wynika z pętli aplikacji. Historyczny fizyczny soak również wykazuje długotrwały CPU, ale pochodzi z innego providera/builda.

### Root cause w kodzie

[`RuntimeEventHub::publish`](../../crates/application/torca-runtime-policy/src/lib.rs) otrzymuje jeden `cursor` i wewnętrznie inkrementuje `revision`. Natomiast outer runtime publikuje do niego `state.revision`. W efekcie `HubState.cursor` przechowuje rewizję runtime’u.

Android w [`TorcaForegroundService.kt`](../../apps/client/flutter/android/app/src/main/kotlin/com/torca/host/TorcaForegroundService.kt) wywołuje:

```kotlin
nativeWaitForRevision(runtimeRevision, notificationCursor, 0)
```

Rust porównuje oba argumenty z hubem:

```rust
state.revision > after_revision || state.cursor > after_cursor
```

Drugi warunek porównuje rewizję runtime’u z kursorem durable notifications. Po pierwszej publikacji jest zwykle stale prawdziwy. Zabezpieczenie `SUCCESS_REARM_DELAY_MS = 100` ogranicza pętlę do około 10 wywołań na sekundę, ale każde wywołanie jest bardzo drogie.

Następnie [`collect_notification_events`](../../crates/platform/torca-native/src/native_runtime/operation_methods.rs) wykonuje `conversation_summaries()` i pełny `snapshot_context()`. Snapshot zawiera także projection audio. [`PlatformAudio::devices`](../../crates/infrastructure/torca-radio-adapters/src/audio.rs) enumeruje CPAL/AAudio, co na Androidzie powoduje JNI i Binder do AudioFlinger.

Pełny snapshot pyta też runtime/engine, przez co czysto odczytowy poll powiadomień uruchamia maintenance oraz workerów storage/delivery. To wyjaśnia, dlaczego service-stop A/B usuwa nie tylko `Thread-17`, ale praktycznie całe obciążenie procesu.

### Czysty profil

Po `pm clear` wyłącznie testowego pakietu AVD, ponownym uruchomieniu release `always`, 30 sekundach stabilizacji oraz wygaszeniu ekranu wynik nadal wynosił 110% mediany i 114% P95. Najgorętsze były analogiczne wątki: native notification 32%, dwa worker threads 20% i 12%, `torca-runtime-owner` 20%, engine 16%.

To wyklucza jako przyczynę stare wiadomości, kontakty, pending attachments lub uszkodzoną bazę testowego profilu.

## Historyczny fizyczny Android

W repo istnieje wcześniejszy 60-minutowy soak urządzenia fizycznego z wygaszonym ekranem i odłączonym zasilaniem:

| Metryka | Wynik |
| --- | ---: |
| bateria | 86% → 79%, spadek 7 pp |
| nominalna pojemność | 5 000 mAh |
| actual drain systemu | 285 mAh |
| estymacja UID aplikacji | 162 mAh |
| CPU przypisany UID | 1 h 29 min 28 s w 60 min wall time |
| screen-off CPU UID | 1 h 12 min 36 s |
| foreground service | około 1 h |

Źródła: [`result.json`](../../artifacts/soak/battery-20260821-175135/result.json) i [`batterystats.txt`](../../artifacts/soak/battery-20260821-175135/batterystats.txt).

Ten przebieg był starszym debug buildem Tor/onion, nie bieżącym Iroh. Nie wolno używać go jako pomiaru `direct` kontra `always`. Jest jednak ważnym dowodem, że ten typ długotrwałej aktywności CPU ma realny koszt na fizycznym Androidzie i nie jest wyłącznie artefaktem emulatora.

## Zalecana kolejność napraw

### P0 — Android waiter i notification projection

1. Rozdzielić `runtime revision` od `notification cursor`. Waiter musi czekać na licznik tego samego typu, który przekazuje caller. Najbezpieczniej publikować osobny notification cursor/event albo usunąć drugi warunek z Androidowego waitera i budzić go jawnie przy powstaniu notification eventu.
2. `notifications.poll` nie może tworzyć pełnego `ApplicationSnapshotContext`. Powinien czytać bounded notification queue/projection, aktualizowaną przy trwałym zdarzeniu wiadomości, kontaktu lub pairingu.
3. Nie enumerować audio devices podczas snapshotu powiadomień. Cache odświeżać po explicit device-change/settings/Radio activation.
4. Czyste query nie może uruchamiać delivery/peer/health maintenance ani pełnych scanów SQLite.
5. Dodać test regresyjny: 30 minut wirtualnego idle, brak eventów → jeden zablokowany waiter, zero notification polls, zero audio enumeration, zero DB maintenance.

### P0 — desktop Iroh `always`

1. Rozdzielić eksperymentalnie relay i address lookup w dwóch release buildach.
2. Zebrać WPR/ETW ze stosami w podniesionej sesji lub równoważny profiler Rust/N0.
3. Powiązać relay/discovery z rzeczywistym reachability demand i dormancy; brak durable work w background nie powinien utrzymywać aktywnego task setu.
4. Do czasu naprawy traktować `direct` jako wariant oszczędny, ale nie zmieniać domyślnego profilu bez świadomej decyzji produktowej: ogranicza on incoming reachability i migrację adresu.

### P1 — bramki CI i soak

- desktop release foreground idle: mediana <1% jednego logicznego CPU;
- desktop minimized/background: mediana <0,25%;
- Android release screen-off idle: zero periodycznych app-controlled wake i brak snapshot/SQLite/audio polls;
- minimum trzy powtórzenia każdego wariantu fizycznego Androida, ta sama sieć, temperatura i urządzenie;
- 30 minut jako szybka bramka, 6–8 godzin jako evidence release;
- osobne progi debug/release i zakaz wnioskowania o mAh z emulatora.

## Automatyzacja i odtworzenie

Dodane lub użyte skrypty:

- [`measure-process-threads.ps1`](../../scripts/measure-process-threads.ps1) — desktop CPU per thread;
- [`measure-desktop-window-cpu.ps1`](../../scripts/measure-desktop-window-cpu.ps1) — visible/minimized A/B z automatycznym przywróceniem okna;
- [`measure-android-process-cpu.ps1`](../../scripts/measure-android-process-cpu.ps1) — Android CPU, screen/power validation i hot threads; schema 2 zapisuje również całkowitą pojemność logicznych CPU;
- [`capture-iroh-android-perfetto.ps1`](../../scripts/capture-iroh-android-perfetto.ps1) — Perfetto; poprawiono katalog wyjściowy dla Androida 16;
- [`capture-iroh-windows-etw.ps1`](../../scripts/capture-iroh-windows-etw.ps1) — WPR/ETW, wymaga uprawnień system profiling;
- [`New-TorcaEnergyAuditReport.ps1`](../../scripts/New-TorcaEnergyAuditReport.ps1) — automatycznie agreguje wszystkie JSON-y, historyczny batterystats i zachowane trace’y do Markdown;
- [`run-iroh-battery-matrix.ps1`](../../scripts/run-iroh-battery-matrix.ps1) oraz [`summarize-iroh-energy.ps1`](../../scripts/summarize-iroh-energy.ps1) — macierz fizycznego urządzenia i statystyczne podsumowanie.

Przykładowa regeneracja indeksu dowodów:

```powershell
.\scripts\New-TorcaEnergyAuditReport.ps1 `
  -MeasurementsRoot .torca/measurements `
  -LegacyAndroidLogicalProcessorCount 4 `
  -Output .torca/measurements/ENERGY_AUDIT_EVIDENCE.md
```

Przykładowy Android CPU sample:

```powershell
.\scripts\measure-android-process-cpu.ps1 `
  -AndroidSerial <serial> `
  -Package com.torca.torca_app `
  -DurationSeconds 30 `
  -Provider iroh `
  -Profile always `
  -Mode background `
  -Output .torca/measurements/android-background.json
```

Przykładowy trace:

```powershell
.\scripts\capture-iroh-android-perfetto.ps1 `
  -AndroidSerial <serial> `
  -DurationSeconds 60 `
  -Provider Iroh `
  -Profile always `
  -Output .torca/measurements/android.perfetto-trace `
  -MetadataOutput .torca/measurements/android.perfetto.json
```

## Ograniczenia

- Bieżący Android Iroh nie został zmierzony w mAh na fizycznym telefonie, ponieważ w sesji nie było podłączonego urządzenia. Emulator był zasilany zewnętrznie.
- Android x86_64 mocno płaci za HPET; bezwzględny procent może różnić się na ARM. Semantyczny błąd waitera oraz 10-Hz pełny snapshot pozostają niezależne od architektury.
- Pomiary CPU są pojedynczymi próbami 15–30 s, a nie trzypróbkową statystyką release. Duże efekty A/B (11,29× na desktopie i około 110% → <1% po service-stop) są jednak znacznie większe od szumu.
- Windows `direct` wyłącza jednocześnie relay i lookup; nie przypisano kosztu do jednego konkretnego taska N0.
- ETW Windows ze stosami wymaga ponowienia w podniesionej sesji.
- Service-stop jest testem izolacyjnym, nie proponowanym rozwiązaniem: wyłącza wymaganą background communication i notifications.

## Stan zmian

Audyt początkowo nie zmieniał zachowania produkcyjnego. W kolejnym, osobnym etapie
wdrożono pierwszy zestaw napraw P0:

- Android ma dedykowany waiter notification cursor; nie porównuje już trwałego
  kursora z runtime revision.
- `RuntimeEventHub` publikuje runtime revision i notification cursor osobno.
- notification projection korzysta z lekkiego application overview i nie buduje
  network/Radio/pełnego snapshotu.
- query nie uruchamia już bezwarunkowo runtime maintenance.
- CPAL/AAudio device enumeration jest cache’owane i odświeżane jawnie.
- dodano automatyczne skrypty energy gate oraz walidator assetów spritesheet.

Zweryfikowano: 470 testów workspace Rust, targeted tests `torca-runtime-policy`,
`torca-native` i `torca-radio-adapters`, Rust clippy, Rust format, Flutter
analyze/test oraz walidację składni PowerShell. Dodano trwały SQLCipher
notification outbox (migracja 0016): payload JSON jest przechowywany pod
unikalnym `event_id`, odczytywany po kursorze i odtwarzany przy ponownym
uruchomieniu runtime; test storage obejmuje idempotencję, kolejność kursorów i
acknowledge. Dodano także automatyczny gate `scripts/Invoke-TorcaEnergyGate.ps1`,
który wymaga uruchomionego procesu desktop albo urządzenia ADB i agreguje
medianę/p95 względem progów regresji. Fizyczny Android soak i izolacja kosztu
relay kontra address lookup są automatyzowane przez
`scripts/run-iroh-routing-isolation.ps1`; fizyczny Android soak nadal wymaga
podłączonego urządzenia. Nie wolno jeszcze
twierdzić, że pełny cel baterii został osiągnięty. Cross-check
`aarch64-linux-android` nie wykonał się w tym środowisku, ponieważ lokalny
toolchain NDK nie dostarczył `assert.h` dla buildu `ring`.

## Komendy pomiarowe po podłączeniu sprzętu

Desktop (uruchomiona wersja Release, proces `torca_app`):

```powershell
.\scripts\Invoke-TorcaEnergyGate.ps1 -Platform desktop -ProcessName torca_app -DurationSeconds 60 -Repetitions 3 -WindowState minimized
```

Android (urządzenie ADB, ekran wygaszony i ładowarka odłączona):

```powershell
.\scripts\Invoke-TorcaEnergyGate.ps1 -Platform android -AndroidSerial <serial> -Package com.torca.torca_app -Profile always -Mode background -DurationSeconds 60 -Repetitions 3
```

Izolacja Iroh relay/discovery (warianty 2×2, minimum trzy powtórzenia):

```powershell
.\scripts\run-iroh-routing-isolation.ps1 -AndroidSerial <serial> -DurationMinutes 30 -Repetitions 3
```

`measure-android-process-cpu.ps1` zapisuje również poziom baterii oraz
`charge_counter` przed i po pomiarze (gdy urządzenie udostępnia ten licznik),
a `New-TorcaEnergyAuditReport.ps1` pokazuje oba delty w tabeli. Dzięki temu
pomiar CPU i rzeczywisty spadek energii są rejestrowane w jednym artefakcie:
[`ENERGY_AUDIT_EVIDENCE.md`](../../.torca/measurements/ENERGY_AUDIT_EVIDENCE.md).

## Desktop smoke measurement (2026-08-27)

The locally available Release executable was started minimized and measured for
10 seconds. The process remained responsive and used a median of **7.7319% of
one logical CPU**, equivalent to **0.4832% of the 16-core machine** (p95
0.7678% machine). A thread breakdown measured **13.7358% of one logical CPU**;
the hottest thread accounted for 75% of process CPU. This is a smoke result,
not a release battery verdict, because it is one short run and the executable
was built with the `always` Iroh profile. The energy gate correctly reports a
failure against the minimized-background target of 0.25% machine CPU, so a
longer three-run measurement and direct-profile comparison are required.

The avatar clock now also accepts an explicit desktop window-visibility signal;
`DesktopLifecycle` stops sprite invalidation on minimize-to-tray and resumes it
on restore, covering platforms that do not emit a paused Flutter lifecycle
event for window minimization.

The rebuilt Release smoke measurement after this change remained in the same
range (median **0.5757% of the machine**, p95 **1.0629%** over 10 seconds), so
avatar frame ticking is not the dominant idle CPU source in this build. The
remaining hotspot is in the native/runtime process and needs a longer ETW or
Perfetto trace for function-level attribution.

### Attachment worker retry fix

The first Release smoke log exposed a concrete high-cost failure loop rather
than an avatar bottleneck: `torca-attachment` panicked with `invalid error
code`, then restarted and retried maintenance every 2 seconds. The transfer
worker was passing uppercase diagnostic strings into the foundation
`ErrorCode` validator, which accepts lowercase redacted codes only. The codes
are now normalized to stable lowercase values such as
`attachment.ack_timeout`; a regression test covers every transfer error
variant. After rebuilding and bundling the native DLL, a 30-second desktop
smoke run produced no `invalid error code`, `worker panic`, or maintenance
retry entries. This removes a confirmed avoidable CPU drain; a fresh three-run
gate is still required to quantify the improvement.

## Android emulator measurement (2026-08-27)

The configured `TorChat_API36` x86_64 emulator was booted automatically, the
Release APK was installed, and the Iroh `always` profile was sampled for 20
seconds in background mode. Median process CPU was **45% of one logical CPU**
(p95 **80.1%**), equivalent to **11.25% / 20.025% of total emulator logical
capacity**. The emulator reported battery level 100% before and after, with a
zero charge-counter delta; because the emulator is not a calibrated physical
battery, this is CPU evidence only. The run also reported `screenOff=false`,
so it is not a valid screen-off battery-life gate. The measurement script now
records these state checks and labels invalid foreground/background setups.

A second automated run explicitly locked the emulator screen and disconnected
external power. It was therefore a valid *state* check (though still not a
physical battery measurement): median CPU was **104% of one logical CPU** and
P95 **110%** (**26% / 27.5% of total emulator capacity**). Battery level and
charge counter again showed no delta. This confirms that the Android idle
problem is reproducible even with the screen off and should be investigated
with Perfetto/Simpleperf on a real ARM handset; it is not attributable to
avatar rasterization alone.

The updated Android Release native library was then built through
`scripts/build.ps1` (both `arm64-v8a` and `x86_64`) and installed as the split
`app-normal-x86_64-release.apk`. With the emulator screen state verified as
off and power source verified as battery, the post-cache run measured **0%
median / 1% P95 of one logical CPU** (0% / 0.25% of total emulator capacity),
down from **104% / 110%** on the pre-cache native library. Battery level and
charge counter remained unchanged, as expected for a short emulator run. This
A/B result confirms that repeated CPAL/AAudio readiness enumeration was the
dominant Android idle CPU drain in the measured scenario.

The Android foreground service now also observes power-save, charging, and
default-network capability callbacks. Metered/validated transitions are sent
to the native battery policy only when their value changes; route changes are
coalesced with a 750 ms debounce. Notification delivery remains cancellable
and event-driven through JNI, so these signals do not reintroduce periodic
polling or an always-awake UI loop.
On Android API S and newer, `ConnectivityDiagnosticsManager` data-stall
callbacks now feed the existing native battery/diagnostics policy; the
single-thread executor is torn down with the service.
The service also replays the sticky battery charging status after native
runtime initialization, avoiding a cold-start window where `charging_on/off`
could be lost.

## Physical Android smoke measurement (2026-08-27)

A Release `iroh/always` APK was installed on a physical Android handset and
measured for 30 seconds with the display in `Dozing` state. The process used
**0% median / 1% P95 of one logical CPU** (30 samples); battery level and
charge-counter stayed at 86% / 3664 mAh, and reported battery temperature was
33.3 °C before and after. The phone was connected over USB, so the test used
the reversible `dumpsys battery unplug` state to prevent charging from masking
CPU behavior and then restored the device with `dumpsys battery reset`.

This handset does not expose `current_now` through `dumpsys battery` and its
power-supply sysfs files require elevated permission. Consequently this is
valid physical CPU/screen-off/thermal evidence, but **not** a calibrated mAh
measurement; `-RequireBatteryTelemetry` correctly remains unsatisfied until a
device exposes current telemetry or an external power monitor is used.

Two additional 30-second repetitions on the same handset measured **0% median
/ 1% P95** and **0% median / 2% P95**, respectively. Across the three valid
screen-off repetitions, the process remained at 0% median CPU and at most 2%
P95 of one logical CPU; no battery-level or charge-counter drop was observed.

The Android measurement harness now records `current now` and battery
temperature (when exposed by `dumpsys battery`) alongside charge-counter and
level deltas. This makes a physical-device run report both CPU and available
instantaneous/thermal evidence instead of relying on emulator charge state.
For a physical acceptance run, pass `-RequireBatteryTelemetry` to
`Invoke-TorcaEnergyGate.ps1`; missing charge/current/temperature fields then
fail the gate rather than silently producing CPU-only evidence.
Desktop runs can be made self-contained with `-LaunchIfMissing -ExecutablePath
<path-to-torca_app.exe>`; the harness only closes a process that it started.
Startup failures and window-creation timeouts use the same cleanup path, so a
failed benchmark cannot leave an orphaned Torca process behind.
