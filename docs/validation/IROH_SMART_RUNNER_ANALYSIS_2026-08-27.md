# Iroh smart runner — analiza i wynik

Data: 2026-08-27  
Zakres: wyłącznie provider Iroh (`always`, `direct`, `local`)

## Co runner rozpoznaje

Runner działa jako automat z fail-closed:

1. **ADB/emulator boot** — czeka na `sys.boot_completed`, stan `device` i
   działający Android package manager; chwilowy `offline` jest retryowany.
2. **Instalacja** — pomija blokującą reinstalację, gdy pakiet jest już obecny;
   nowa instalacja ma limit czasu i zachowuje stderr.
3. **Ekran startowy** — uruchamia `MainActivity`, sprawdza `mResumedActivity` /
   `ResumedActivity` oraz skanuje logcat pod kątem `FATAL EXCEPTION`, błędów
   composition/contract/native startup i `StartupFailure`.
4. **Semantyczny UI** — na emulatorze z oknem można włączyć `-EnableUiProbe`;
   wtedy runner wymaga etykiet Torca/Contacts/Invitations/Settings/Profile.
   Domyślny benchmark używa `-no-window`, gdzie API 35/36 potrafi zawiesić
   `uiautomator dump`; `startup-ui.xml` zawiera wówczas jawne
   `UI_PROBE_SKIPPED`, a sygnałem gotowości jest activity + czysty logcat.
5. **Screen-off** — próbuje `KEYCODE_SLEEP` (223), następnie power toggle
   (26), ponownie sleep; wymaga `Dozing`/`Asleep` i zapisuje `screen-power.txt`.
6. **Pomiar** — mierzy proces aplikacji dla wybranego profilu Iroh, pilnuje
   limitu host CPU (domyślnie 15%) i sprząta emulator w `finally`.

Każdy nieudany etap zapisuje `startup-activity.txt`, `startup-logcat.txt`,
`startup-ui.xml`, `screen-power.txt` (jeśli etap power został osiągnięty) oraz
`last-failure.log` dla scenariusza rozmowy. Nie ma zielonego wyniku po samym
uruchomieniu procesu.

## Realistyczny scenariusz conversation

`Start-TorcaBackgroundTest.ps1 -Mode conversation` uruchamia produkcyjny
`torca-soak --scenario active-messaging --communication-provider iroh`:

- buduje/uruchamia izolowany SOAK client z `ScenarioBridge`;
- tworzy świeży kontakt bota i credentials, wykonuje invitation/join/approval;
- zamyka pairing transport przed częścią messaging;
- zestawia peer przez zapisany `ContactRoute` i czeka na Ready;
- wysyła tekst A→B i B→A, attachment/control frame oraz sprawdza receipt/
  persistence;
- zapisuje timeline, manifest, notification observations i logi ADB.

Pierwsza próba w tym środowisku przeszła start aplikacji, ale zatrzymała się na
pierwszym budowaniu `torca-lab-peer`; została przerwana po ograniczonym czasie.
To wynik **incomplete**, nie sukces messaging. Przed pełnym przebiegiem można
zbudować peer wcześniej albo przekazać gotowy `--lab-peer`. Artefakty:
`.torca/measurements/background/conversation-verify4/`.

## Wyniki CPU/energii

- Iroh `always` Android emulator, 3 × 60 s: median **0%**, P95 **1%**;
- smart smoke po poprawkach (10 s): median **8%**, P95 **11.5%** — koszt
  startu/wybudzenia, nie idle baseline;
- desktop Iroh `direct`: około **0.64%** całej maszyny, `always`: około
  **7.24%**; relay/discovery jest dominującym kosztem profilu `always`;
- wcześniejsze Android 100% CPU wynikało z pętli snapshot/audio w foreground
  service; waiter event-driven i cache gotowości audio usunęły polling;
- avatary używają spritesheetów i współdzielonego frame clock; nie są źródłem
  idle drainu. W tle clock jest zatrzymywany.

CPU emulatora nie jest mAh. Do kalibracji potrzebny jest `current_now` lub
zewnętrzny power monitor; fizyczny telefon w obecnym środowisku tego telemetry
nie udostępnia.

## Uruchamianie w tle

```powershell
.\scripts\Start-TorcaBackgroundTest.ps1 -Mode smoke -DurationSeconds 30
.\scripts\Start-TorcaBackgroundTest.ps1 -Mode soak -Profile always -DurationSeconds 60 -Repetitions 3
.\scripts\Start-TorcaBackgroundTest.ps1 -Mode conversation -Profile always -FakePeers 1 -DurationSeconds 180
```

Runner jest serializowany lockiem, działa z priorytetem BelowNormal i kończy
emulator po każdym przebiegu. Logi można przeglądać w trakcie gry; brak
procesów emulatora po zakończeniu jest warunkiem cleanup.

## Pozostałe bramki sprzętowe

- trzy powtórzenia Android release dla `always/direct/local`;
- 2×2 relay/discovery na realnej sieci;
- Wi-Fi→LTE dla Iroh `always`;
- skalibrowane mAh/current;
- pełny conversation po wcześniejszym prebuildzie `torca-lab-peer`.

