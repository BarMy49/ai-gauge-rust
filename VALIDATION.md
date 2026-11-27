# Sprawdzenie AI Gauge Rust 0.3.0

## Aktualizacja: liczniki przez CLI, Gemini usunięte (27 listopada 2026)

- Gemini zostało całkowicie usunięte (kod, ustawienia, logowanie, UI). Stare
  pliki `settings.json` z wpisami `gemini` nadal się wczytują (test migracji).
- `ai-gauge --check` na komputerze autora: ChatGPT/Codex przez Codex CLI — OK,
  Claude przez logowanie Claude Code — OK (sesja 5 h, tydzień, dodatkowy limit),
  DeepSeek API — OK, OpenAI API — HTTP 403 (zapisany klucz nie jest kluczem
  ADMIN), Anthropic API — wyłączone, nie sprawdzono.
- `cargo test`, `cargo fmt --check`, `cargo clippy -- -D warnings`: OK.

Poniższe sekcje opisują wcześniejsze sprawdzenia i mogą wspominać Gemini.

## Aktualizacja: dostępny interfejs i personalizacja (2 listopada 2026)

Windows / MSVC, Slint 1.18.1. Interfejs przebudowano według podanych zasad:

- Jeden styl Fluent, standardowe kontrolki Slint, wspólna paleta i typografia
  oparta na rem. Motyw systemowy/jasny/ciemny, tekst 100–200%.
- Przegląd używa czytelnych wierszy z wyrównanymi wartościami i separatorami.
  Układy Slint i Flexbox zapewniają zawijanie; wąskie okno ma górną nawigację.
  Odstępy korzystają ze skali 4/8 px. Własne ikony mają wspólny styl konturowy.
- Widoczność i kolejność są wspólne dla przeglądu i nawigacji, zapisywane
  automatycznie; monitorowanie i przypięty licznik pozostają niezależne.
  Obsługiwane są uchwyt przeciągania, przyciski i strzałki klawiatury.
- Stany pusty, ładowania, nieaktualnych danych i błędu mają jawne komunikaty;
  kontrolki korzystają ze stanów Fluent, a aktywny wiersz ma obrys fokusu.
- Logowanie Gemini otwiera zwykłe okno osobnego profilu Chrome/Edge bez
  argumentów automatyzacji/debugowania. Odczyt wymaga zamknięcia okna
  i wybrania Kontynuuj. Oczekiwanie i błędy wstrzymują automatyczne odczyty.

Sprawdzenia:

- `cargo test --locked`: **27 testów przeszło**. Nowe testy obejmują migrację
  konfiguracji, zapis kolejności/widoczności bez zmiany monitorowania i tray,
  blokowanie odczytów podczas logowania/błędu Gemini oraz argumenty zwykłego
  procesu logowania Google.
- `cargo fmt --check` i `cargo clippy --locked --all-targets -- -D warnings`: OK.
- `cargo build --locked --release`: OK. Gotowe EXE przeszło `--smoke-test`
  i wykonało zrzut jasnego motywu, oba uruchomienia zakończone kodem 0.
- Uruchomiono rzeczywiste okna i obejrzano zrzuty renderera Slint w
  `target/visual-qa/`: 640×480, 960×680 i 1440×900, motywy jasny/ciemny,
  tekst 100%/200%, przewijane ustawienia, personalizacja i panel Gemini.
  Sprawdzono też stany pusty, ładowania, błędu i starych danych.
- Testy zdarzeń Slint `--drag-test` i `--controls-test` przeszły: przeciąganie,
  ukrywanie, przycisk przesunięcia, zgodność nawigacji oraz dalsze przesunięcie
  klawiaturą. Naprawiono utratę fokusu po zmianie kolejności.
- Zrzuty są danymi demo; nie potwierdzają rzeczywistego logowania Google ani
  dostępności procentów dla konkretnego konta. Nie wykonano nowego testu Linux.

Przykład odtwarzalnego zrzutu (tryb izolowany, bez kont i zapisu preferencji):

```text
target/debug/ai-gauge.exe --visual-test target/visual-qa/example.ppm --width 640 --height 480 --theme light --text-scale 200 --view settings
```

Testy interakcji wymagają domyślnego rozmiaru 960×680 i `--view customize`.
Format PPM zawiera bezpośredni wynik renderera; PNG służą do przeglądania.

## Aktualizacja: duże cyfry tray i pozostały limit

Wykonano 1 listopada 2026 na Windows 11 x64, Rust 1.97.0 / MSVC.

- `cargo test --locked`: **23 testy przeszły**. Nowe przypadki obejmują
  pozostały limit (0%, 37%, ułamki, 100%), zachowanie procentów budżetu API,
  brak danych, podpowiedź i tray, zapis opcji i ponowne wczytanie konfiguracji.
- `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`
  i `cargo build --locked --release`: zakończone powodzeniem.
- `--smoke-test`: rzeczywisty start GUI i automatyczne zakończenie, kod 0.
  Dodano manifest Common Controls v6 dla MSVC: bez niego import
  `GetWindowSubclass` powodował błąd startu `0xc0000139`.
- Sprawdzono obrazy ikon przy 16, 20, 24, 32, 48 i 64 px na jasnym i ciemnym
  tle: 0, 7, 37, 100, 999, `?`, `!`. Testy sprawdzają wysokość cyfr, brak
  obcięcia, warianty ICO i wyrównanie masek przezroczystości.
  Systemowy `LoadImageW` poprawnie odczytał wszystkie 42 warianty ikon.
- Nie wykonano interaktywnego testu okna: narzędzie Windows Computer Use
  nie miało dostępnego połączenia z aplikacjami. Nie wykonano nowych testów
  Linux: lokalny Debian/WSL nie ma Cargo ani wymaganych pakietów budowania.
  Istniejący workflow Windows/Linux pozostaje dostępny; nie uruchomiono go tutaj.

## Wcześniejsze sprawdzenie wersji 0.3.0

Wykonano 1 listopada 2026, Rust 1.99.1, środowisko Linux x86-64.

- `cargo test --locked`: **18 testów przeszło**. Kwoty, waluty, wiele okien
  Codex, brak i starzenie danych, klucze RAM poza konfiguracją, blokada instancji,
  demo offline, format ikony, dozwolone domeny logowania, integralność modułu,
  odrzucanie niepotwierdzonej sesji. Test RPC używa osobnego procesu z opóźnioną
  inicjalizacją i powiadomieniem przychodzącym przed odpowiedzią.
- `cargo clippy --locked --all-targets -- -D warnings` oraz `cargo fmt --check`:
  zakończone powodzeniem na Linux.
- Zbudowano binaria release Linux x86-64 i Windows x86-64 GNU (cross-kompilacja
  przez Zig). EXE to PE32+ / Windows GUI. Sprawdzono importy DLL: system Windows
  i UCRT, bez Pythona, Qt, Electron i webview.
- `--smoke-test`: rzeczywisty start okna i automatyczne zamknięcie po 3,1 s, kod wyjścia 0.
- Rzeczywiste okno Slint uruchomiono pod Xvfb, renderer software, 960×680.
  Sprawdzono kliknięciami X11 podgląd, wybór ChatGPT i ustawienia konta.
  Poprawiono równe kolumny kart i wypełnianie pasków od lewej.
- Pobrano oficjalny `codex-app-server` rust-v0.166.0 dla Linux z openai/codex
  i sprawdzono SHA-256 z oficjalnego wydania. Uruchomiono go z pustym, odizolowanym
  CODEX_HOME: initialize zakończyło się poprawnie; account/read zwróciło brak konta.
  account/login/start zwróciło adres HTTPS auth.openai.com i loginId;
  account/login/cancel zwróciło status canceled. Nie wpisywano danych konta.
- Rzeczywista aplikacja uruchomiona z tym modułem w osobnym katalogu testowym:
  kliknięcie logowania pokazało oczekiwanie na autoryzację, z przyciskami otwarcia
  strony i anulowania; kliknięcie Anuluj przywróciło formularz i komunikat o anulowaniu.
  Samo otwarcie autoryzacji nie oznaczyło konta jako zalogowanego.

Nie wykonano pełnego logowania na prawdziwe konto ani odczytu jego limitów,
rzeczywistego uruchomienia EXE na Windows 11, testu na fizycznej Fedorze 44,
testu tray z panelem GNOME/KDE, ani prawdziwych systemowych magazynów kluczy.
Device-code jest zaimplementowany według protokołu, bez logowania prawdziwego konta.
Raporty API sprawdzono na danych testowych, bez kluczy użytkownika.
Odczyty Claude i Gemini nadal zależą od paneli dostawców i nie zostały potwierdzone
na zalogowanych kontach. ChatGPT pokazuje liczniki Codex, nie wszystkich modeli czatu.

Zrzuty w Preview/ pochodzą z prawdziwego okna, podgląd usług zawiera dane demo.
Źródła i binaria nie korzystają z Pythona.
