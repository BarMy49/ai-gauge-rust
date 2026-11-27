# AI Gauge 0.3 — Rust / native desktop

Aplikacja do monitorowania limitów i środków usług AI w systemowym tray.
Kod aplikacji i integracji jest w Rust. Okno ma natywny interfejs Slint w stylu Fluent, z jasnym i ciemnym motywem;
tray korzysta z Win32 na Windows i StatusNotifier na Linux. Nie ma Pythona, Electron ani webview w interfejsie aplikacji.

## Uruchomienie gotowej aplikacji

W paczce z gotowymi programami:

- **Windows 11 x64:** uruchom `Windows/AI-Gauge.exe`. Możesz przenieść samo EXE.
  Wymaga wyłącznie bibliotek systemowych Windows. Nie wymaga Rust, Pythona ani Qt.
- **Fedora x86_64 / aarch64:** pobierz z [Releases](https://github.com/BarMy49/ai-gauge-rust/releases)
  plik `ai-gauge-<wersja>-1.fc43.<arch>.rpm` i zainstaluj go poleceniem
  `sudo dnf install ./ai-gauge-*.rpm` (dnf doinstaluje potrzebne biblioteki pulpitu).
  Uruchom **AI Gauge** z menu aplikacji lub poleceniem `ai-gauge`. Odinstalowanie:
  `sudo dnf remove ai-gauge`. Do zapamiętywania kluczy potrzebny jest np. `gnome-keyring`.
- **Inny Linux x86_64:** `AI-Gauge-x86_64.AppImage` — nadaj prawo wykonywania
  (`chmod +x AI-Gauge-x86_64.AppImage`) i uruchom.
- W GNOME ikona tray wymaga rozszerzenia **AppIndicator and KStatusNotifierItem Support**.
  Bez obsługi tray aplikacja pozostawia okno dostępne. KDE obsługuje tray standardowo.

Windows może umieścić ikonę pod strzałką ukrytych ikon; można ją przeciągnąć na pasek.
Nie ma obowiązku uruchamiania programu jako administrator. EXE nie jest podpisane
certyfikatem wydawcy.

`--demo` uruchamia przykładowe dane offline. `--smoke-test` dodatkowo zamyka okno
po trzech sekundach; ten tryb służy do testowania startu programu bez kont i sieci.

## Korzystanie

1. Wybierz usługę w panelu po lewej i otwórz **Konto i ustawienia**.
2. Dla kont subskrypcyjnych użyj **Zaloguj się z ChatGPT** lub **Zaloguj w przeglądarce**. Dla API wklej klucz
   w zamaskowane pole, zaznacz monitorowanie i zapisz ustawienia.
3. Przycisk **Odśwież** pobiera dane aktywnych usług. Odczyt odbywa się w tle;
   sieć, logowanie i magazyn kluczy nie blokują głównego wątku GUI.
4. W podglądzie kliknij **Pokaż na ikonie** na karcie licznika.
   Możesz również wybrać licznik w menu prawego przycisku ikony.
5. Zamknięcie okna chowa je do tray. Całkowite zamknięcie jest osobną opcją menu
   oraz przyciskiem w oknie. Lewy przycisk ikony przywraca okno.

Całą ikonę zajmuje **duża liczba**, bez znaku `%` i paska postępu. Domyślnie jest to
wykorzystany procent. Aby pokazywać pozostały limit, otwórz **Ustawienia**,
zaznacz **Pokaż pozostały limit zamiast
wykorzystanego** i kliknij **Zapisz opcje aplikacji**. Wybór jest zapamiętywany.
Zmiana obejmuje ikonę, podpowiedź, menu, przegląd, karty limitów i paski postępu
dla ChatGPT/Codex i Claude: np. 37% wykorzystania staje się 63% pozostałego
limitu. Osobna opcja **Pokaż, ile procent kwoty odniesienia pozostało w API
rozliczanych w walucie** robi to samo dla salda DeepSeek oraz budżetów OpenAI API
i Anthropic API; kwota w walucie się nie zmienia.
Menu grupuje wszystkie
odczytane liczniki według usług; zawiera także środki, daty resetu, odświeżanie,
pokazanie okna i zamknięcie. `?` oznacza brak procentu, nieaktualne dane lub błąd,
a nie zerowe wykorzystanie. Dla bardzo dużych przekroczeń budżetu ikona pokazuje
`!`; dokładny procent pozostaje w oknie i menu. Wybór jest zapamiętywany.

### Wygląd i kolejność usług

W **Ustawieniach** wybierz motyw systemowy, jasny lub ciemny i wielkość tekstu
100%, 125%, 150% albo 200%. Te ustawienia zapisują się automatycznie.
W małym oknie nawigacja przenosi się nad treść, a długie widoki można przewijać.

**Dostosuj usługi** pozwala ukrywać usługi i zmieniać ich kolejność uchwytem,
przyciskami **Przenieś w górę / w dół** lub strzałkami klawiatury na aktywnym
wierszu. Przegląd i lista nawigacji korzystają ze wspólnej kolejności i widoczności.
Zmiany zapisują się automatycznie. Ukrywanie nie wyłącza monitorowania ani
przypiętego licznika tray. Ukryte usługi można przywrócić w tym samym widoku;
przycisk resetowania przywraca domyślną kolejność i widoczność.

## Integracje i znaczenie wartości

### Wymagane narzędzia CLI

Liczniki subskrypcji są odczytywane przez oficjalne narzędzia wiersza poleceń dostawców.
**Zainstaluj CLI i zaloguj się w nim raz** na tym samym komputerze i koncie użytkownika:

| Licznik | Narzędzie | Jednorazowe logowanie |
|---|---|---|
| ChatGPT / Codex | [Codex CLI](https://github.com/openai/codex) (`codex` w `PATH`) | `codex login` (lub przycisk logowania w aplikacji) |
| Claude Code / Claude | [Claude Code](https://docs.claude.com/en/docs/claude-code) | `claude`, potem `/login` |

AI Gauge tylko odczytuje te logowania (`~/.codex`, `~/.claude/.credentials.json`) i nigdy ich nie zmienia. Gdy logowanie Claude Code wygaśnie,
uruchom na chwilę `claude` i odśwież. Bez Claude Code aplikacja używa
logowania w przeglądarce, a claude.ai obecnie blokuje takie odczyty w tle.

Test wszystkich liczników z terminala: `ai-gauge --check`. Każda usługa jest odświeżana
raz, a wynik lub błąd trafia na ekran (bez kluczy i tokenów).

| Usługa | Dostęp | Dane |
|---|---|---|
| ChatGPT / Codex | Oficjalna autoryzacja OpenAI: przeglądarka lub kod urządzenia | Liczniki Codex dostępne dla konta; to nie uniwersalny licznik wszystkich rozmów ChatGPT |
| OpenAI API | Klucz **administracyjny** organizacji | Raport kosztów całej organizacji w bieżącym miesiącu UTC; % własnego budżetu |
| DeepSeek API | Zwykły własny klucz API | Rzeczywiste saldo USD/CNY, środki promocyjne i doładowane; opcjonalnie % własnej kwoty odniesienia |
| Claude Code / Claude | Claude Code zalogowany na tym komputerze (zalecane) albo logowanie w przeglądarce | Limity sesji, tygodnia i pozostałe zwrócone liczniki wykorzystania z panelu Usage |
| Anthropic API | Klucz **administracyjny** organizacji | Koszt organizacji w bieżącym miesiącu UTC; obejmuje też Claude Code rozliczane tym API |

**OpenAI i Anthropic:** raport kosztów nie jest saldem przedpłaty. Zwykły klucz do
generowania nie ma wymaganych uprawnień do tego raportu. Budżet jest wyłącznie
własnym progiem do wyświetlania procentu i nie ogranicza wydatków na serwerze.
Raporty mogą być opóźnione. Ta wersja nie pokazuje RPM/TPM.

Procent API = `koszt miesiąca / własny budżet × 100`.
Procent DeepSeek = `max(0, (kwota odniesienia − saldo) / kwota odniesienia × 100)`
i jest liczony tylko w wybranej walucie. Po doładowaniu można zmienić kwotę
odniesienia. Bez dodatniej kwoty odniesienia/budżetu pokazujemy samą kwotę.

### Logowanie kont

Bez Claude Code aplikacja otwiera dla Claude zainstalowany Edge, Chrome lub Chromium z **własnym profilem
danej usługi**. Na Windows Edge jest wykrywany automatycznie. Nie przejmuje
codziennego profilu przeglądarki, nie importuje jego cookies i nie odczytuje haseł.
W otwartym oknie zaloguj się na stronie dostawcy. Nie trzeba logować do synchronizacji
samej przeglądarki. Okno logowania jest zwykłym oknem bez automatyzacji, więc działa
też **Kontynuuj z Google**. Dla Claude zamknij okno po zalogowaniu i naciśnij Odśwież.
Jeśli wybrana przeglądarka nie jest zainstalowana, używana jest dowolna zainstalowana.
Nie ma ręcznego wybierania elementów strony ani konfiguracji selektorów CSS.

**Claude najlepiej działa przez Claude Code.** Jeśli Claude Code jest zalogowany na tym
komputerze (`claude`, potem `/login`), limity są odczytywane przez jego oficjalne logowanie
(tylko odczyt, bez zmian w plikach Claude Code). claude.ai blokuje odczyty w tle
weryfikacją Cloudflare, więc przeglądarka jest tylko zapasową drogą.

Zapasowo Claude używa panelu `https://claude.ai/settings/usage`. Jeśli konto ma kilka
organizacji, można wpisać UUID właściwej organizacji w ustawieniach. Bez UUID
aplikacja wybiera organizację automatycznie tylko wtedy, gdy jest dokładnie jedna.

## Zakres sprawdzenia

Informacje o rzeczywiście wykonanych testach są w `VALIDATION.md`. Kompilacja
nie oznacza potwierdzenia logowania na wszystkie konta. Bez sesji i kluczy użytkownika
nie potwierdzono działania rzeczywistych paneli ChatGPT i Claude.
Integracje tych paneli są zależne od ich bieżącego formatu.

## Źródła interfejsów

- OpenAI costs: https://developers.openai.com/api/reference/resources/admin/subresources/organization/subresources/usage/methods/costs
- Codex app-server: https://learn.chatgpt.com/docs/app-server
- DeepSeek balance: https://api-docs.deepseek.com/api/get-user-balance/
- Anthropic costs: https://platform.claude.com/docs/en/manage-claude/usage-cost-api
- Claude Usage: https://support.claude.com/en/articles/11647753-how-do-usage-and-length-limits-work
- Chrome profiles / CDP: https://developer.chrome.com/blog/remote-debugging-port

Źródła aplikacji: MIT. Slint: Royalty-free Desktop, Mobile, and Web Applications License 2.0,
atrybucja w oknie O aplikacji. Zależności zachowują swoje licencje; patrz THIRD-PARTY.md.
