//! Deutsch.

use super::{Appli, Item, Ligne, Systeme, Textes};

pub const TEXTES: Textes = Textes {
    // ---- Allgemein
    accroche: "Ein Dokument. Alle Möglichkeiten.",
    nav: ["Überblick", "Syntax", "Installieren", "Ausprobieren"],
    nav_aria: "Hauptnavigation",
    accueil_aria: "Varium, Startseite",
    langue_choisir: "Sprache wählen",
    installer_varium: "Varium installieren",
    code_source: "Quellcode",
    pied: "Varium, eine Office-Suite mit Variablen, geschrieben in Rust.",
    pied_paquets: "Diese Website ist mit {} gebaut, dem Full-Stack-Framework für Rust.",
    lire_syntaxe: "Zur Syntax",
    voir_exemple: "Beispiel ansehen",

    // ---- Startseite
    description_accueil: "Varium ist eine in Rust geschriebene Office-Suite: Documents, Classeur (Tabellen) und Présentation (Folien). Schreiben Sie {{vorname}} einmal, ändern Sie den Wert, und alles zieht mit. Sechs Schreibsprachen, für Linux, macOS und Windows.",
    surtitre: "Office-Suite mit Variablen · Rust",
    chapo: "Documents, Classeur und Présentation: eine vollständige Office-Suite, die auf Ihrem Computer läuft. Schreiben Sie {{vorname}}, {{nachname}} oder {{betrag}} in einen Text oder auf eine Folie, ändern Sie einen Wert ein einziges Mal, und jedes Vorkommen wird aktualisiert.",
    sous_hero: "Desktop-App für Linux, macOS und Windows · Ihre Dokumente bleiben bei Ihnen · 100 % Rust",
    apercu: ["Die Vorlage", "Das Dokument"],
    apercu_note: "Ein französisches Beispiel, beim Erzeugen dieser Seite von der echten Engine von Varium (varium-core) berechnet.",
    applis_titre: "Drei Anwendungen, ein Varium",
    applis_intro: "Textverarbeitung, Tabellenkalkulation und Präsentationsprogramm, vereint in einer einzigen Desktop-Anwendung.",
    applis: [
        Appli {
            genre: "Textverarbeitung",
            texte: "Briefe, Angebote und Verträge, die sich selbst ausfüllen.",
            points: [
                "Überschriften, Listen, Tabellen und echte Seiten in A4, A5 oder Letter",
                "Globale, berechnete und bedingte {{…}}-Felder, Serienbriefe",
                "Symbolleiste wie in Writer, Seitenleiste wie in Pages",
                "Export als PDF, DOCX, ODT, RTF, EPUB, HTML und LaTeX",
                "Rechtschreibprüfung in sechs Sprachen, mit echten Hunspell-Wörterbüchern",
            ],
        },
        Appli {
            genre: "Tabellenkalkulation",
            texte: "Tabellen mit Zellen, Formeln und Diagrammen.",
            points: [
                "Zellen, Formeln und mehrere Tabellenblätter pro Arbeitsmappe",
                "Funktionen auf Französisch oder Englisch: SOMME oder SUM, SI oder IF, RECHERCHEV oder VLOOKUP",
                "Bedingte Formatierung, Farbskalen, Datenbalken und fixierte Zeilen und Spalten",
                "Acht Diagramme: Balken, gestapelte Balken, Linien, Flächen, Kreis, Ring, Punkte (XY)",
                "Öffnet und speichert Excel- (XLSX) und LibreOffice-Dateien (ODS) sowie CSV",
            ],
        },
        Appli {
            genre: "Präsentationen",
            texte: "Foliensätze zum Vorführen, mit Ihren Feldern darin.",
            points: [
                "Layouts, Designs und Übergänge",
                "Bilder und Formen mit der Maus platzieren, {{…}}-Felder auf den Folien",
                "Referentenansicht auf einem zweiten Bildschirm: Notizen, nächste Folie, Stoppuhr",
                "Öffnet PowerPoint- (PPTX) und Impress-Präsentationen (ODP)",
                "Export nach HTML, ODP und PDF",
            ],
        },
    ],
    langues_titre: "In sechs Sprachen schreiben",
    langues_intro: "Jedes Dokument hat seine Sprache: Französisch, Englisch, Spanisch, Deutsch, Italienisch oder Portugiesisch. Varium hält sich an ihre Schreibregeln.",
    langues: [
        Item {
            titre: "Automatische Typografie",
            texte: "Anführungszeichen «\u{a0}» im Französischen, “\u{a0}” im Englischen oder „\u{a0}“ im Deutschen, typografische Apostrophe und im Französischen das schmale Leerzeichen vor ;\u{a0}:\u{a0}!\u{a0}? setzt Varium von selbst.",
        },
        Item {
            titre: "Korrektur",
            texte: "Die Prüfung Révision meldet doppelte Leerzeichen, wiederholte Wörter, fehlende Großschreibung, Leerzeichen vor Satzzeichen, ein fehlendes ¿ oder ¡ im Spanischen, „a“ oder „an“ im Englischen …",
        },
        Item {
            titre: "Datum, Zahlen und Währungen",
            texte: "Jede Sprache hat ihre Formate: 12\u{202f}500,50 im Französischen, 12,500.50 im Englischen, 12.500,50 im Deutschen; Datumsangaben, Monatsnamen und Beträge folgen der Sprache des Dokuments.",
        },
        Item {
            titre: "Rechtschreibung",
            texte: "Fehler werden beim Tippen unterstrichen, Vorschläge gibt es per Rechtsklick. Varium nutzt bereits installierte Wörterbücher oder lädt sie von LibreOffice herunter und merkt sich Ihre Wörter in einem persönlichen Wörterbuch.",
        },
    ],
    langues_note: "Die Oberfläche von Varium gibt es in diesen sechs Sprachen; die Sprache des Textes wird für jedes Dokument gewählt.",
    fonctions_titre: "Alles für Ihre wiederkehrenden Dokumente",
    fonctions_intro: "Briefe, Angebote, Verträge, Bescheinigungen: eine Vorlage, so viele Dokumente wie nötig.",
    fonctions: [
        Item {
            titre: "Globale Felder",
            texte: "Ein einmal eingegebener Wert aktualisiert jedes Vorkommen, im Dokument wie in der Präsentation. {{vorname}} und {{nachname}} bleiben immer getrennt.",
        },
        Item {
            titre: "Berechnete Felder",
            texte: "{{= menge * preis | money}}: Summen, Mehrwertsteuer, Monatsraten und Prozente rechnen sich von selbst neu.",
        },
        Item {
            titre: "Bedingter Text",
            texte: "{{si rabatt}}…{{sinon}}…{{fin}} (wenn, sonst, Ende): Ein Absatz erscheint nur, wenn das Feld ausgefüllt ist.",
        },
        Item {
            titre: "Serienbriefe",
            texte: "Eine CSV-Tabelle aus Excel oder Calc genügt: Varium erstellt ein Dokument pro Empfänger, in einer einzigen Datei.",
        },
        Item {
            titre: "Fertige Vorlagen",
            texte: "Brief, Rechnung, Angebot, Bescheinigung, Vertrag, Einladung: ein Klick, und es geht los.",
        },
        Item {
            titre: "Typisierte Felder und Filter",
            texte: "Datum mit Kalender, Betrag, Zahl, E-Mail, im Format der Dokumentsprache; Filter wie upper, date:full, money:usd …",
        },
        Item {
            titre: "Echte Seiten",
            texte: "Seitenweise Vorschau in A4, A5 oder Letter, mit Rändern, Kopfzeile, Fußzeile, Seitenzahlen und Zoom.",
        },
        Item {
            titre: "Export für LibreOffice und Microsoft Office",
            texte: "Dokumente als natives PDF, DOCX, ODT, RTF und EPUB; Arbeitsmappen als XLSX, ODS und CSV; Präsentationen als ODP, HTML und PDF.",
        },
        Item {
            titre: "Dateien wie in Writer",
            texte: "Strg+S, „Speichern unter“, zuletzt verwendete Dokumente, Rückfrage vor dem Schließen und automatische Sicherung.",
        },
        Item {
            titre: "Suchen, ersetzen, rückgängig",
            texte: "Mit oder ohne Groß-/Kleinschreibung suchen, einzeln oder alle auf einmal ersetzen, Rückgängig und Wiederholen praktisch ohne Grenze.",
        },
        Item {
            titre: "LaTeX-Formeln",
            texte: "$…$ im Text, $$…$$ zentriert. Ein $ aus einem Wert (10,00 US$) öffnet nie eine Formel.",
        },
        Item {
            titre: "Eine echte Mac-App",
            texte: "Menüs in der Menüleiste, ⌘-Kurzbefehle, eigenes Symbol, Vollbild und Fokusmodus; auch für Windows und Linux.",
        },
    ],
    etapes_titre: "So funktioniert's",
    etapes: [
        Item {
            titre: "Schreiben",
            texte: "Verfassen Sie Ihren Text und setzen Sie {{Variablen}} überall dort ein, wo er sich von Dokument zu Dokument ändert.",
        },
        Item {
            titre: "Ausfüllen",
            texte: "Varium erkennt jede Variable und bietet Ihnen ein Feld mit dem passenden Typ an: Datum, Betrag, E-Mail …",
        },
        Item {
            titre: "Exportieren",
            texte: "Das fertige Dokument, Formeln inklusive, als PDF, DOCX, ODT, HTML, LaTeX oder reiner Text.",
        },
    ],
    bureau: Item {
        titre: "Auf dem Desktop",
        texte: "Eine schnelle native Anwendung, gebaut mit egui, für Linux, macOS und Windows.",
    },
    prive: Item {
        titre: "Ihre Dokumente bleiben bei Ihnen",
        texte: "Kein Konto, kein Server, keine Online-Version: Varium speichert Ihre Projekte auf Ihrer Festplatte, und nichts verlässt Ihren Computer.",
    },
    capot_titre: "Unter der Haube",
    capot_intro: "Ein Cargo-Workspace: reine Engines ohne Oberfläche und die Anwendung, die sie zusammenführt.",
    capot_colonnes: ["Crate", "Aufgabe"],
    capot_roles: [
        "Die Dokument-Engine: Parser, Rendering, Filter, Formeln, Sprachen, Exporte, Projektformat. Getestet, ohne Grafikabhängigkeit.",
        "Die Tabellen-Engine: Zellen, Formeln, Blätter und Exporte.",
        "Die Präsentations-Engine: Folien, Layouts, Designs und Exporte.",
        "Die Desktop-Anwendung mit egui/eframe, die die drei Module vereint, für Linux, macOS und Windows.",
    ],
    appel_titre: "Bereit zum Ausprobieren?",
    appel_texte: "Installieren Sie Varium auf Ihrem Computer: Vorlagen schreiben, Werte ausfüllen, rechnen, präsentieren, exportieren.",

    // ---- Syntax
    syntaxe_description: "Die Syntax von Varium: {{…}}-Variablen, Filter, Typen, LaTeX-Formeln, Tabellenfunktionen und Exporte.",
    syntaxe_chapo: "Alles, was man in ein Varium-Dokument schreiben kann.",
    sommaire: [
        "Variablen",
        "Filter",
        "Typen",
        "Formeln",
        "Tabellen",
        "Exporte",
        "API",
    ],
    ecrire_titre: "Variablen schreiben",
    ecrire_texte: "Eine Variable steht zwischen doppelten geschweiften Klammern. Namen dürfen Umlaute und Akzente, Ziffern, _, . und - enthalten: {{vorname}}, {{kunde.ort}}, {{datum-1}}.",
    syntaxe_colonnes: ["Schreibweise", "Wirkung"],
    syntaxe: [
        Ligne {
            code: "{{name}}",
            effet: "ein Feld: überall derselbe Wert",
        },
        Ligne {
            code: "{{ name | upper | trim }}",
            effet: "ein Feld mit verketteten Filtern",
        },
        Ligne {
            code: "{{betrag | money}}",
            effet: "ein Betrag in Euro (money:usd, money:gbp, money:chf …)",
        },
        Ligne {
            code: "{{datum | date}}",
            effet: "ein langes Datum in der Dokumentsprache: 15. März 2026 (date:full, date:short)",
        },
        Ligne {
            code: "{{x | number:2}}",
            effet: "eine Zahl mit zwei Nachkommastellen: 3,14",
        },
        Ligne {
            code: "{{t | default:\"Text\"}}",
            effet: "ein Standardwert, wenn das Feld leer ist",
        },
        Ligne {
            code: "{{= menge * preis | money}}",
            effet: "ein berechnetes Feld: + − * / ( ), 20%, round(x; 2), min, max, sum",
        },
        Ligne {
            code: "{{si rabatt}}…{{sinon}}…{{fin}}",
            effet: "bedingter Text (wenn, sonst, Ende)",
        },
        Ligne {
            code: "{{aujourdhui}}",
            effet: "das heutige Datum, automatisch eingesetzt",
        },
        Ligne {
            code: "# Titel, ## Untertitel, ### Titel 3",
            effet: "Überschriften",
        },
        Ligne {
            code: "**fett**, *kursiv*",
            effet: "fett und kursiv",
        },
        Ligne {
            code: "__unterstrichen__, ~~durchgestrichen~~",
            effet: "unterstrichen und durchgestrichen",
        },
        Ligne {
            code: "- Punkt, 1. Punkt",
            effet: "Aufzählungen und nummerierte Listen (2 Leerzeichen Einzug pro Ebene)",
        },
        Ligne {
            code: "> Zitat",
            effet: "ein Zitat",
        },
        Ligne {
            code: "-> zentriert <-, -> rechts ->",
            effet: "Absatzausrichtung",
        },
        Ligne {
            code: "| a | b |  dann  |---|---:|",
            effet: "eine Tabelle, ihre Kopfzeile und die Spaltenausrichtung",
        },
        Ligne {
            code: "---, \\newpage",
            effet: "horizontale Linie, Seitenumbruch",
        },
        Ligne {
            code: "\\sommaire",
            effet: "ein aus den Überschriften erzeugtes Inhaltsverzeichnis",
        },
        Ligne {
            code: "$E = mc^2$, $$ \\sum_i x_i $$",
            effet: "LaTeX-Formeln, im Text oder zentriert",
        },
        Ligne {
            code: "\\{{name}}",
            effet: "geschweifte Klammern als Text, kein Feld",
        },
        Ligne {
            code: "\\$, \\*, \\|",
            effet: "ein Zeichen als Text",
        },
    ],
    filtres_titre: "Filter",
    filtres_texte: "Filter verändern den Wert eines Vorkommens, und nur diesen. Sie werden von links nach rechts verkettet: {{name | trim | upper}}. Die Ergebnisse unten berechnet varium-core für ein französisches Dokument; in einer anderen Sprache folgen Zahlen, Beträge und Datumsangaben deren Konventionen.",
    filtres_colonnes: ["Filter", "Wert", "Ergebnis"],
    filtre_defaut: "Sehr geehrte Damen und Herren",
    types_titre: "Variablentypen",
    types_texte: "Jede Variable hat einen Typ: Er bestimmt das Eingabefeld, prüft den Wert und liefert ein Standardformat, wenn das Vorkommen keinen Filter hat. Auf Deutsch erscheint ein Datum als „1.\u{a0}März\u{a0}2026“, ein Betrag als „12.500,00\u{a0}€“.",
    types: ["Text", "Langer Text", "Datum", "Zahl", "Betrag", "E-Mail"],
    formules_titre: "Formeln",
    formules_texte: "Zwischen $…$ (im Text) oder $$…$$ (zentriert) steht LaTeX, und auch dort funktionieren Variablen. Nur die im Quelltext geschriebenen $ öffnen eine Formel: Ein Wert wie „10,00 US$“ bringt das Dokument nie durcheinander.",
    formules_exemple: "Die Monatsrate beträgt $m = \\frac{M}{n}$, mit\n$$ C_n = C_0 \\times (1 + t)^n $$",
    classeur_titre: "Formeln in Classeur",
    classeur_texte: "In Classeur beginnt eine Formel mit =, und Funktionsnamen schreiben Sie wahlweise auf Französisch oder Englisch: =SOMME(A1:A10) und =SUM(A1:A10) ergeben dieselbe Summe.",
    classeur_colonnes: ["Funktion (Französisch · Englisch)", "Wirkung"],
    classeur_fonctions: [
        "addiert die Werte eines Bereichs",
        "liefert je nach Bedingung den einen oder den anderen Wert",
        "sucht einen Wert in der ersten Spalte eines Bereichs und liefert den passenden Wert aus einer anderen Spalte",
    ],
    classeur_note: "In einer Präsentation schreiben Sie {{Felder}} genau wie in einem Dokument.",
    exports_titre: "Exporte",
    exports_noms: ["Text", "Vorlage", "Projekt"],
    exports: [
        "nativ, ohne Browser, mit Seiten im richtigen Format, Kopf- und Fußzeile.",
        "ein Word-Dokument, das sich auch mit LibreOffice und Google Docs öffnen lässt.",
        "ein LibreOffice-Writer-Dokument mit Formatvorlagen, Kopfzeile und Seitenzahlen.",
        "für Pages und TextEdit.",
        "ein E-Book.",
        "eine eigenständige Datei, Formeln mit KaTeX gesetzt, druckfertig.",
        "eine vollständige .tex-Datei, direkt kompilierbar.",
        "das fertige Dokument, ohne Formatierung.",
        "der unbearbeitete Quelltext, mit seinen {{Variablen}}.",
        "eine .varium.json-Datei mit Quelltext, Werten und Typen.",
    ],
    api_titre: "Render-API",
    api_intro_statique: "Der Server dieser Website (cd site && cargo run) stellt die Engine als JSON bereit, zum Ausprobieren im Terminal:",
    api_intro_serveur: "Diese Website stellt die Engine als JSON bereit, zum Ausprobieren im Terminal:",
    api_reponse: "Die Antwort enthält den gerenderten Text (texte), den vollständigen HTML-Export (html) und die erkannten Variablen mit der Anzahl ihrer Vorkommen (variables).",

    // ---- Installieren
    installer_description: "Varium herunterladen, die Office-Suite mit Variablen (Documents, Classeur, Présentation), für Windows, macOS und Linux.",
    installer_chapo: "Varium ist eine kostenlose Office-Suite für Windows, macOS und Linux: Documents, Classeur und Présentation. Kein Konto, keine Online-Version: Ihre Dateien bleiben auf Ihrem Computer.",
    telecharger_pour: "Für {} herunterladen",
    systemes: [
        Systeme {
            compat: "Windows 10 und 11, 64 Bit",
            etapes: [
                "Entpacken Sie die Datei (Rechtsklick → Alle extrahieren).",
                "Starten Sie varium.exe.",
                "Zeigt Windows „Der Computer wurde durch Windows geschützt“: „Weitere Informationen“ → „Trotzdem ausführen“.",
            ],
        },
        Systeme {
            compat: "macOS 11 oder neuer, Apple Silicon und Intel",
            etapes: [
                "Entpacken Sie die Datei und ziehen Sie Varium in den Ordner Programme.",
                "Beim ersten Start: Rechtsklick auf Varium → Öffnen → Öffnen.",
                "Verweigert macOS weiterhin den Start: Systemeinstellungen → Datenschutz & Sicherheit → „Dennoch öffnen“.",
            ],
        },
        Systeme {
            compat: "x86_64, glibc 2.35 oder neuer (Ubuntu 22.04, Debian 12, Fedora 36 …)",
            etapes: [
                "Entpacken: tar xzf varium-linux-x86_64.tar.gz",
                "Starten: ./varium/varium",
                "Für das Anwendungsmenü: varium nach ~/.local/bin, varium.desktop nach ~/.local/share/applications und varium.svg nach ~/.local/share/icons kopieren.",
            ],
        },
    ],
    versions: "Alle Versionen mit Versionshinweisen: {}. Die Apps sind noch nicht von Apple oder Microsoft signiert, daher die Warnung beim ersten Start.",
    versions_lien: "Release-Seite auf GitHub",
    source_titre: "Aus dem Quellcode",
    source_texte: "Mit Rust (rustup.rs) lässt sich der Befehl varium auf jedem System installieren:",
    source_linux: "Unter Linux nutzt Varium die üblichen Grafikbibliotheken (libxkbcommon, Wayland oder X11, ein OpenGL- oder Vulkan-Treiber), die auf jedem Desktop bereits vorhanden sind.",
    source_github: "Quellcode auf GitHub",

    // ---- Ausprobieren
    essayer_titre: "Varium ausprobieren",
    essayer_description: [
        "Schreiben Sie eine Vorlage mit {{Variablen}}, geben Sie ihnen Werte und sehen Sie zu, wie sich das Dokument aktualisiert: die Online-Demo von Varium.",
        "Eine Vorlage mit {{Variablen}}, ihre Werte und das von Varium erzeugte Dokument. Die komplette Suite installieren Sie auf Ihrem Computer.",
    ],
    essayer_explication: [
        "Ändern Sie die Vorlage oder die Werte und aktualisieren Sie dann das Dokument. Es wird von der echten Engine von Varium (varium-core) auf dem Server erzeugt. Das Beispiel ist auf Französisch, ebenso seine Datums- und Betragsformate.",
        "Der Beispielbrief von Varium, auf Französisch: links die Vorlage und ihre Werte, rechts das von der echten Engine (varium-core) berechnete Dokument.",
    ],
    essayer_installer: Item {
        titre: "Um eigene Vorlagen zu schreiben, installieren Sie Varium.",
        texte: "Die komplette Suite ist eine Desktop-Anwendung für Linux, macOS und Windows: Seitenlayout, berechnete Felder, Serienbriefe, Tabellen, Präsentationen, Export als ODT, DOCX und PDF. Ihre Dokumente bleiben auf Ihrem Computer.",
    },
    modele: "Vorlage",
    modele_aide: "schreiben Sie {{eine_variable}} an beliebiger Stelle",
    lecture_seule: "schreibgeschützt",
    variables: "Variablen ({})",
    aucune_variable: "Noch keine Variablen: Fügen Sie {{vorname}} in die Vorlage ein und aktualisieren Sie.",
    mettre_a_jour: "Dokument aktualisieren",
    reinitialiser: "Zurücksetzen",
    variables_colonnes: ["Variable", "Typ", "Wert"],
    a_remplir: "auszufüllen",
    document: "Dokument",
    compteur: [
        "{} Variable",
        "{} Variablen",
        "{} Vorkommen",
        "{} Vorkommen",
    ],
    a_remplir_n: "{} ohne Wert",
    texte_brut: "Reiner Text",
    sans_valeur: "„{}“ hat noch keinen Wert",
    saut_de_page: "Seitenumbruch",
    type_de: "Typ von {}",
    format_date: "JJJJ-MM-TT",
    erreurs: [
        "Datum erwartet: JJJJ-MM-TT",
        "Zahl erwartet",
        "Ungültige E-Mail-Adresse",
    ],
    tronque: "Die Vorlage wurde auf {} Zeichen gekürzt.",

    // ---- Nicht gefunden
    introuvable_titre: "Seite nicht gefunden",
    introuvable_description: "Diese Seite existiert nicht.",
    introuvable_h1: "{{seite}} nicht gefunden",
    introuvable_texte: "Diese Variable hat keinen Wert: Die angeforderte Seite existiert nicht.",
    retour_accueil: "Zur Startseite",
};
