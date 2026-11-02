from pathlib import Path
p=Path('ui/app.slint');s=p.read_text(encoding='utf-8')
words='''Otwórz Przegląd przeglądzie przeglądu przeglądarki przeglądarce przeglądarka usług usługi listę liście Pokaż Przenieś górę dół Upuść Dostosuj Kolejność widoczność są wspólne listą układ domyślny Przywróć Twoje środków środki Odśwież odśwież wyłącza Ukryte Wybierz widzieć mogą być Wszystkie ukryte pozostały wykorzystanego miesięczny Miesięczny Odświeżanie sekund Wygląd czytelność Czytelność Duże Schowaj Zamknij aplikację aplikacji Dokończ połączenie Połączenie Połącz Zaloguj się ręcznie osobnym zwykłym oknie Następnie zamknij wszystkie tego wybierz Kontynuuj przeglądarkę Samo otwarcie potwierdza logowania Anuluj połączenie ponownie Wróć limitów Brak odczytanych Dodaj klucz włącz monitorowanie dostępne dostępnych konta Otwórz ustawienia Połączenie tę Przeglądarka Automatycznie logowanie odbywa osobnym zwykłym Aplikacja Twojej codziennej Puste zachowuje zapisany Zapisz systemowym Kwota odniesienia Organizacja UUID opcjonalnie Zapisz ustawienia Wyloguj Usuń O Rozmiar Ciemny Jasny Systemowy Limity pozostały własnych budżetów API salda zachowują swoje znaczenie Odświeżanie sekundy Uruchamiaj aplikację schowaną Natywny środków Zmiany zapisują automatycznie Ukrywanie licznika przeciągnij Przeciągnij użyj przycisków Przegląd Wybierz usługi Schowaj narzędzia Nie można usunąć Rozmiar dużym małym'''.split()
for word in sorted(set(words), key=len, reverse=True):
    broken=word.encode('ascii','replace').decode()
    if broken != word:s=s.replace(broken,word)
s=s.replace(' ? ',' — ')
s=s.replace('position-1','position - 1').replace('mouse-y-self','mouse-y - self').replace('mouse-y-drag','mouse-y - drag')
s=s.replace('mouse-cursor:move','mouse-cursor:MouseCursor.move')
for name,value in [('small','0.875'),('body','1'),('heading','1.25'),('title','1.75'),('value','2')]:
    s=s.replace(f'out property<length> {name}: {value}rem;',f'out property<float> {name}: {value};')
    s=s.replace('font-size:T.'+name+';', 'font-size:T.'+name+' * 1rem;')
s=s.replace('root.navigation-changed(i)','root.navigation-changed(self.current-index)')
s=s.replace('root.text-scale=i==1?125:i==2?150:i==3?200:100','root.text-scale=self.current-index==1?125:self.current-index==2?150:self.current-index==3?200:100')
# Keep the window minimum independent of the responsive layout's contents.
s=s.replace('    HorizontalLayout { spacing:0;','    Rectangle { width:parent.width; height:parent.height;\n    HorizontalLayout { spacing:0;')
s=s[:-2]+'    }\n}\n'
p.write_text(s,encoding='utf-8')
