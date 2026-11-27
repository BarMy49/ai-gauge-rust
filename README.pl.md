# AI Gauge 0.3 — Rust / native desktop

Aplikacja do monitorowania limitów i środków usług AI w systemowym tray.
Kod aplikacji i integracji jest w Rust. Okno ma natywny interfejs Slint w stylu Fluent, z jasnym i ciemnym motywem;
tray korzysta z Win32 na Windows i StatusNotifier na Linux. Nie ma Pythona, Electron ani webview w interfejsie aplikacji.

## Uruchomienie gotowej aplikacji

W paczce z gotowymi programami:

- **Windows 11 x64:** uruchom `Windows/AI-Gauge.exe`. Możesz przenieść samo EXE.
  Wymaga wyłącznie bibliotek systemowych Windows. Nie wymaga Rust, Pythona ani Qt.
- **Fedora 44 x86-64:** uruchom `Linux/ai-gauge`. W razie potrzeby nadaj plikowi
  uprawnienie wykonywania: `chmod +x Linux/ai-gauge`. Wymagane biblioteki pulpitu:
  `sudo dnf install fontconfig libxkbcommon libX11 libXcursor libXi libXrandr gnome-keyring`.
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
dla ChatGPT/Codex, Claude i Gemini: np. 37% wykorzystania staje się 63% pozostałego
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
| Gemini | [Gemini CLI](https://github.com/google-gemini/gemini-cli): `npm install -g @google/gemini-cli` | `gemini`, potem „Login with Google” |

AI Gauge tylko odczytuje te logowania (`~/.codex`, `~/.claude/.credentials.json`,
`~/.gemini/oauth_creds.json`) i nigdy ich nie zmienia. Gdy logowanie Claude Code wygaśnie,
uruchom na chwilę `claude` i odśwież. Bez Claude Code lub Gemini CLI aplikacja używa
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
| Gemini | Gemini CLI zalogowany na tym komputerze (zalecane) albo logowanie Google w przeglądarce | Limity modeli Gemini CLI lub procenty z panelu Limity wykorzystania |

**OpenAI i Anthropic:** raport kosztów nie jest saldem przedpłaty. Zwykły klucz do
generowania nie ma wymaganych uprawnień do tego raportu. Budżet jest wyłącznie
własnym progiem do wyświetlania procentu i nie ogranicza wydatków na serwerze.
Raporty mogą być opóźnione. Ta wersja nie pokazuje RPM/TPM.

Procent API = `koszt miesiąca / własny budżet × 100`.
Procent DeepSeek = `max(0, (kwota odniesienia − saldo) / kwota odniesienia × 100)`
i jest liczony tylko w wybranej walucie. Po doładowaniu można zmienić kwotę
odniesienia. Bez dodatniej kwoty odniesienia/budżetu pokazujemy samą kwotę.

### Logowanie kont

Dla Claude i Gemini aplikacja otwiera zainstalowany Edge, Chrome lub Chromium z **własnym profilem
danej usługi**. Na Windows Edge jest wykrywany automatycznie. Nie przejmuje
codziennego profilu przeglądarki, nie importuje jego cookies i nie odczytuje haseł.
W otwartym oknie zaloguj się na stronie dostawcy. Nie trzeba logować do synchronizacji
samej przeglądarki. Okno logowania jest zwykłym oknem bez automatyzacji, więc działa
też **Kontynuuj z Google**. Dla Claude zamknij okno po zalogowaniu i naciśnij Odśwież.
Jeśli wybrana przeglądarka nie jest zainstalowana, używana jest dowolna zainstalowana.
Nie ma ręcznego wybierania elementów strony ani konfiguracji selektorów CSS.

**Gemini — logowanie w dwóch krokach:** w ustawieniach konta wybierz automatyczne
wykrywanie przeglądarki, Chrome lub Edge i zapisz ustawienia. Uruchom logowanie,
zaloguj się do Google w otwartym oknie, a następnie zamknij wszystkie okna tego
profilu. W AI Gauge kliknij **Kontynuuj**. Okno logowania nie używa argumentów
automatyzacji ani zdalnego debugowania; odczyt rozpoczyna się dopiero po jego
zamknięciu. Samo otwarcie przeglądarki nie potwierdza połączenia.
W razie odrzucenia logowania lub wygaśnięcia sesji aplikacja pokazuje błąd
i wstrzymuje automatyczne próby; można ponowić logowanie w innej przeglądarce.
Anulowanie wyłącza monitorowanie Gemini, zachowując lokalny profil.

**Claude najlepiej działa przez Claude Code.** Jeśli Claude Code jest zalogowany na tym
komputerze (`claude`, potem `/login`), limity są odczytywane przez jego oficjalne logowanie
(tylko odczyt, bez zmian w plikach Claude Code). claude.ai blokuje odczyty w tle
weryfikacją Cloudflare, więc przeglądarka jest tylko zapasową drogą.

Zapasowo Claude używa panelu `https://claude.ai/settings/usage`. Jeśli konto ma kilka
organizacji, można wpisać UUID właściwej organizacji w ustawieniach. Bez UUID
aplikacja wybiera organizację automatycznie tylko wtedy, gdy jest dokładnie jedna.

Gemini próbuje otworzyć panel **Ustawienia → Limity wykorzystania**. Jeżeli
nie otworzy się automatycznie, otwórz go w oknie tej sesji. Odczytywane są widoczne
paski ARIA i jednoznacznie opisane procenty. Gdy panel istnieje wyłącznie jako
dialog, automatyczny odczyt może być niedostępny. Strona musi rzeczywiście udostępniać
procenty dla konta. Odczyt strony może przestać działać po zmianie jej interfejsu.
Nie ma gwarancji, że każdy plan i każde konto udostępnia taki licznik.

Po zamknięciu okna logowania program może otworzyć własny profil w tle na czas
odczytu, a następnie go zamknąć. Przy otwartej karcie odczytuje jej aktualny widok;
jeżeli strona sama nie aktualizuje wartości, odśwież stronę/panel. Sesje dostawców
mogą wygasać i wymagać ponownego logowania. Logowanie oraz limity nie są omijane.
Profile Flatpak nie są automatycznie obsługiwane; potrzebna jest przeglądarka
desktopowa dostępna jako program systemowy.

**ChatGPT:** użyj **Zaloguj się z ChatGPT**. Aplikacja uruchamia oficjalny moduł
OpenAI i otwiera autoryzację w domyślnej przeglądarce. Samo otwarcie strony nie
potwierdza połączenia: program czeka na zakończenie logowania, sprawdza konto
(`account/read`), pokazuje email i plan, a następnie pobiera liczniki Codex.
Okna logowania nie zamykaj przed potwierdzeniem. Możesz anulować operację.

Gdy powrót z przeglądarki nie działa, wybierz **Zaloguj kodem**, otwórz podaną
stronę i wpisz kod widoczny w aplikacji. Logowanie kodem urządzenia może wymagać
włączenia tej opcji w ustawieniach bezpieczeństwa konta ChatGPT.

Nie musisz instalować CLI. Przy pierwszym połączeniu, jeśli nie ma Codex w PATH,
program pobiera oficjalny `codex-app-server` z wydania `openai/codex` na GitHub
(około 90 MB do pobrania, około 300 MB po rozpakowaniu). Weryfikuje SHA-256
z metadanych oficjalnego wydania, a potem sumę lokalnego pliku przy każdym starcie
modułu. Pobieranie odbywa się po kliknięciu logowania, z postępem i anulowaniem;
nie zachodzi podczas odświeżania w tle. Gotowy moduł pozostaje w `auth-client/`.
Jeśli jest już Codex CLI w PATH, używany jest jego `app-server`.

Moduł dostaje własny katalog `CODEX_HOME`; sesja innej instalacji nie jest kopiowana.
Program korzysta wyłącznie z metod konta i limitów, bez wysyłania promptów.
Limity dotyczą uprawnień Codex na koncie ChatGPT; nie są uniwersalnymi limitami
wszystkich modeli i rozmów w chatgpt.com. Stare ustawienie logowania przez
profil CDP jest automatycznie pomijane dla ChatGPT.

Przeglądarka jest adapterem logowania i danych, a nie silnikiem GUI. Jej otwarte
procesy oczywiście zużywają dodatkową pamięć. Do samych integracji API nie jest potrzebna.

### Klucze i dane lokalne

Klucze są przechowywane w Windows Credential Manager lub Secret Service na Linux.
Po odznaczeniu magazynu nowy klucz pozostaje tylko w pamięci procesu. Poprzedni
klucz systemowy pozostaje wtedy bez zmian i może wrócić po restarcie; do jego
usunięcia służy osobny przycisk. Puste pole zachowuje zapisany klucz. Do konfiguracji
JSON nigdy nie trafia wartość klucza. Magazyn systemowy może wymagać odblokowania.

Ustawienia, blokada drugiej instancji i profile znajdują się w katalogu danych
aplikacji wyznaczonym przez system dla **AI Gauge Rust**. Możesz ustalić go samodzielnie:

```text
AI-Gauge.exe --data-dir C:\Users\TwojUzytkownik\AppData\Local\AI-Gauge-Rust
```

Profilów `browser/` oraz `codex/` nie udostępniaj: mogą zawierać sesje logowania.
Szyfrowaniem cookies zarządza przeglądarka i system. Katalogi na Linux są tworzone
z uprawnieniami 0700. Wylogowanie usuwa lokalną sesję aktualnego sposobu logowania;
nie wylogowuje innych przeglądarek ani innych instalacji CLI.

Wersja Rust ma osobną konfigurację i osobne wpisy magazynu kluczy. Nie importuje
automatycznie ustawień ani sesji poprzedniej wersji Python. Brak własnego serwera,
telemetrii i zapytań generujących tekst. Do oficjalnych API wysyłane są tylko
żądania odczytu salda/raportów. Strony dostawców ładują swoje własne zasoby.

„Uruchamiaj schowane” dotyczy startu programu, nie instaluje autostartu systemu.
Opcjonalnie dodaj skrót EXE z `--hidden` do `shell:startup` w Windows.

## Budowanie ze źródeł

Potrzebne są Rust/Cargo **1.92 lub nowszy** i systemowy linker.
Na Windows najprościej użyć standardowego toolchainu MSVC i Build Tools for Visual Studio
z komponentem Desktop development with C++. Na Fedorze wystarczy m.in. `gcc`.
Na Fedorze do budowania: `sudo dnf install gcc gcc-c++ fontconfig-devel pkgconf-pkg-config`.
Slint kompiluje układ okna do Rust, renderer działa bez webview.

```bash
cargo build --locked --release
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
```

Wynik: `target/release/ai-gauge.exe` na Windows lub `target/release/ai-gauge` na Linux.
Nie ma skryptów instalujących Pythona. Moduł ChatGPT jest przygotowywany na żądanie logowania.
`Cargo.lock` jest dołączony. Układ Slint i skrypt adaptera strony są osadzone
w programie; nie trzeba przenosić ich obok binariów.

Dołączony workflow GitHub Actions buduje i testuje natywnie wersje Windows/Linux,
w tym uruchamia trzysekundowy test startu GUI. Do zwykłego korzystania workflow
i narzędzia programistyczne nie są potrzebne.

## Zakres sprawdzenia

Informacje o rzeczywiście wykonanych testach są w `VALIDATION.md`. Kompilacja
nie oznacza potwierdzenia logowania na wszystkie konta. Bez sesji i kluczy użytkownika
nie potwierdzono działania rzeczywistych paneli ChatGPT, Claude i Gemini.
Integracje tych paneli są zależne od ich bieżącego formatu.

## Źródła interfejsów

- OpenAI costs: https://developers.openai.com/api/reference/resources/admin/subresources/organization/subresources/usage/methods/costs
- Codex app-server: https://learn.chatgpt.com/docs/app-server
- DeepSeek balance: https://api-docs.deepseek.com/api/get-user-balance/
- Anthropic costs: https://platform.claude.com/docs/en/manage-claude/usage-cost-api
- Claude Usage: https://support.claude.com/en/articles/11647753-how-do-usage-and-length-limits-work
- Gemini limits: https://support.google.com/gemini/answer/16275805?hl=pl
- Chrome profiles / CDP: https://developer.chrome.com/blog/remote-debugging-port

Źródła aplikacji: MIT. Slint: Royalty-free Desktop, Mobile, and Web Applications License 2.0,
atrybucja w oknie O aplikacji. Zależności zachowują swoje licencje; patrz THIRD-PARTY.md.
