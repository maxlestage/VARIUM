//! Italiano.

use super::{Appli, Item, Ligne, Systeme, Textes};

pub const TEXTES: Textes = Textes {
    // ---- Comune
    accroche: "Un documento. Tutte le sue possibilità.",
    nav: ["Panoramica", "Sintassi", "Installa", "Prova"],
    nav_aria: "Navigazione principale",
    accueil_aria: "Varium, home page",
    langue_choisir: "Scegli la lingua",
    installer_varium: "Installa Varium",
    code_source: "Codice sorgente",
    pied: "Varium, una suite per l'ufficio con variabili, scritta in Rust.",
    pied_paquets: "Questo sito è realizzato con {}, il framework full-stack per Rust.",
    lire_syntaxe: "Leggi la sintassi",
    voir_exemple: "Guarda un esempio",

    // ---- Home
    description_accueil: "Varium è una suite per l'ufficio scritta in Rust: Documents, Classeur (fogli di calcolo) e Présentation (diapositive). Scrivi {{nome}} una volta, cambia il suo valore e tutto si aggiorna. Sei lingue di scrittura, per Linux, macOS e Windows.",
    surtitre: "Suite per l'ufficio con variabili · Rust",
    chapo: "Documents, Classeur e Présentation: una suite per l'ufficio completa che funziona sul tuo computer. Scrivi {{nome}}, {{cognome}} o {{importo}} in un testo o in una diapositiva, cambia un valore una sola volta e tutte le sue occorrenze si aggiornano.",
    sous_hero: "App desktop per Linux, macOS e Windows · i tuoi documenti restano sul tuo computer · 100% Rust",
    apercu: ["Il modello", "Il documento"],
    apercu_note: "Un esempio in francese, generato dal vero motore di Varium (varium-core) alla creazione di questa pagina.",
    applis_titre: "Tre applicazioni, un solo Varium",
    applis_intro: "Un elaboratore di testi, un foglio di calcolo e un programma per presentazioni, riuniti nella stessa applicazione desktop.",
    applis: [
        Appli {
            genre: "Elaboratore di testi",
            texte: "Lettere, preventivi e contratti che si compilano da soli.",
            points: [
                "Titoli, elenchi, tabelle e vere pagine A4, A5 o Letter",
                "Campi {{…}} globali, calcolati e condizionali, stampa unione, rilevamento automatico dei campi",
                "Barra degli strumenti come in Writer, pannello laterale come in Pages",
                "Esportazione in PDF, DOCX, ODT, RTF, EPUB, HTML e LaTeX",
                "Ortografia in sei lingue, con veri dizionari Hunspell",
            ],
        },
        Appli {
            genre: "Foglio di calcolo",
            texte: "Fogli con celle, formule e grafici.",
            points: [
                "Celle, formule e più fogli per cartella, e domande in linguaggio semplice",
                "Funzioni in francese o in inglese: SOMME o SUM, SI o IF, RECHERCHEV o VLOOKUP",
                "Formattazione condizionale, scale di colori, barre dei dati e riquadri bloccati",
                "Otto grafici: barre, barre in pila, linee, aree, torta, anello, dispersione",
                "Apre e salva file Excel (XLSX) e LibreOffice (ODS), oltre al CSV",
            ],
        },
        Appli {
            genre: "Presentazioni",
            texte: "Diapositive pronte da proiettare, con i tuoi campi dentro.",
            points: [
                "Layout, temi e transizioni",
                "Immagini e forme da posizionare con il mouse, campi {{…}} nelle diapositive",
                "Modalità relatore su un secondo schermo: note, diapositiva successiva, cronometro",
                "Apre le presentazioni PowerPoint (PPTX) e Impress (ODP)",
                "Esportazione in HTML, ODP e PDF",
            ],
        },
    ],
    langues_titre: "Scrivere in sei lingue",
    langues_intro: "Ogni documento ha la sua lingua: francese, inglese, spagnolo, tedesco, italiano o portoghese. Varium ne segue le regole di scrittura.",
    langues: [
        Item {
            titre: "Tipografia automatica",
            texte: "Le virgolette «\u{a0}» in francese, “\u{a0}” in inglese o „\u{a0}“ in tedesco, l'apostrofo tipografico e, in francese, lo spazio fine prima di ;\u{a0}:\u{a0}!\u{a0}? si inseriscono da soli.",
        },
        Item {
            titre: "Revisione",
            texte: "Il correttore Révision segnala spazi doppi, parole ripetute, maiuscole dimenticate, spazi prima della punteggiatura, un ¿ o un ¡ mancante in spagnolo, «a» o «an» in inglese…",
        },
        Item {
            titre: "Date, numeri e valute",
            texte: "Ogni lingua ha i suoi formati: 12\u{202f}500,50 in francese, 12,500.50 in inglese, 12.500,50 in tedesco; date, nomi dei mesi e importi seguono la lingua del documento.",
        },
        Item {
            titre: "Ortografia",
            texte: "Gli errori vengono sottolineati durante la digitazione, con suggerimenti al clic destro. Varium usa i dizionari installati o li scarica da LibreOffice, e ricorda le vostre parole in un dizionario personale.",
        },
    ],
    langues_note: "L'interfaccia di Varium è disponibile in queste sei lingue; la lingua del testo si sceglie per ogni documento.",
    fonctions_titre: "Tutto ciò che serve per i tuoi documenti ripetitivi",
    fonctions_intro: "Lettere, preventivi, contratti, attestati: un solo modello, tutti i documenti che servono.",
    fonctions: [
        Item {
            titre: "Campi globali",
            texte: "Un valore inserito una volta aggiorna tutte le sue occorrenze, in un documento come in una presentazione. {{nome}} e {{cognome}} restano sempre distinti.",
        },
        Item {
            titre: "Campi calcolati",
            texte: "{{= quantità * prezzo | money}}: totali, IVA, rate mensili e percentuali si ricalcolano da soli.",
        },
        Item {
            titre: "Testo condizionale",
            texte: "{{si sconto}}…{{sinon}}…{{fin}} (se, altrimenti, fine): un paragrafo compare solo se il campo è compilato.",
        },
        Item {
            titre: "Stampa unione",
            texte: "Basta una tabella CSV di Excel o di Calc: Varium produce un documento per destinatario, in un unico file.",
        },
        Item {
            titre: "Modelli pronti all'uso",
            texte: "Lettera, fattura, preventivo, attestato, contratto, convocazione: un clic per iniziare.",
        },
        Item {
            titre: "Campi tipizzati e filtri",
            texte: "Data con calendario, importo, numero, e-mail, nel formato della lingua del documento; filtri upper, date:full, money:usd…",
        },
        Item {
            titre: "Pagine vere",
            texte: "Anteprima pagina per pagina in formato A4, A5 o Letter, con margini, intestazione, piè di pagina, numeri di pagina e zoom.",
        },
        Item {
            titre: "Esportazione per LibreOffice e Microsoft Office",
            texte: "Documenti in PDF nativo, DOCX, ODT, RTF ed EPUB; cartelle di lavoro in XLSX, ODS e CSV; presentazioni in ODP, HTML e PDF.",
        },
        Item {
            titre: "File come in Writer",
            texte: "Ctrl+S, «Salva con nome», documenti recenti, conferma prima di chiudere e salvataggio di emergenza automatico.",
        },
        Item {
            titre: "Cerca, sostituisci, annulla",
            texte: "Ricerca con o senza distinzione tra maiuscole e minuscole, sostituzione una per una o in blocco, annulla e ripeti senza limiti pratici.",
        },
        Item {
            titre: "Formule LaTeX",
            texte: "$…$ nel testo, $$…$$ centrate. Un $ che viene da un valore (10,00 US$) non apre mai una formula.",
        },
        Item {
            titre: "Una vera app per Mac",
            texte: "Menu nella barra dei menu, scorciatoie con ⌘, icona, schermo intero e modalità concentrazione; anche per Windows e Linux.",
        },
    ],
    etapes_titre: "Come funziona",
    etapes: [
        Item {
            titre: "Scrivi",
            texte: "Redigi il testo e inserisci delle {{variabili}} dove cambia da un documento all'altro.",
        },
        Item {
            titre: "Compila",
            texte: "Varium rileva ogni variabile e ti propone un campo del tipo giusto: data, importo, e-mail…",
        },
        Item {
            titre: "Esporta",
            texte: "Il documento finale, formule comprese, in PDF, DOCX, ODT, HTML, LaTeX o testo.",
        },
    ],
    bureau: Item {
        titre: "Sul desktop",
        texte: "Un'applicazione nativa e veloce, costruita con egui, per Linux, macOS e Windows.",
    },
    prive: Item {
        titre: "I tuoi documenti restano tuoi",
        texte: "Nessun account, nessun server, nessuna versione online: Varium salva i tuoi progetti sul tuo disco e nulla lascia il tuo computer.",
    },
    capot_titre: "Sotto il cofano",
    capot_intro: "Un workspace Cargo: motori puri, senza interfaccia, e l'applicazione che li riunisce.",
    capot_colonnes: ["Crate", "Ruolo"],
    capot_roles: [
        "Il motore dei documenti: analisi, rendering, filtri, formule, lingue, esportazioni, formato di progetto. Testato, senza dipendenze grafiche.",
        "Il motore del foglio di calcolo: celle, formule, fogli ed esportazioni.",
        "Il motore delle presentazioni: diapositive, layout, temi ed esportazioni.",
        "L'applicazione desktop egui/eframe che riunisce i tre moduli, per Linux, macOS e Windows.",
    ],
    appel_titre: "Pronto a provarlo?",
    appel_texte: "Installa Varium sul tuo computer: scrivi i tuoi modelli, compila i valori, calcola, presenta, esporta.",

    // ---- Sintassi
    syntaxe_description: "La sintassi di Varium: variabili {{…}}, filtri, tipi, formule LaTeX, funzioni del foglio di calcolo ed esportazioni.",
    syntaxe_chapo: "Tutto ciò che si può scrivere in un documento Varium.",
    sommaire: [
        "Variabili",
        "Filtri",
        "Tipi",
        "Formule",
        "Foglio di calcolo",
        "Esportazioni",
        "API",
    ],
    ecrire_titre: "Scrivere le variabili",
    ecrire_texte: "Una variabile si scrive tra doppie parentesi graffe. I nomi accettano lettere accentate, cifre, _, . e -: {{nome}}, {{cliente.città}}, {{data-1}}.",
    syntaxe_colonnes: ["Scrittura", "Effetto"],
    syntaxe: [
        Ligne {
            code: "{{nome}}",
            effet: "un campo: lo stesso valore ovunque",
        },
        Ligne {
            code: "{{ nome | upper | trim }}",
            effet: "un campo con filtri concatenati",
        },
        Ligne {
            code: "{{importo | money}}",
            effet: "un importo in euro (money:usd, money:gbp, money:chf…)",
        },
        Ligne {
            code: "{{data | date}}",
            effet: "una data estesa nella lingua del documento: 15 marzo 2026 (date:full, date:short)",
        },
        Ligne {
            code: "{{x | number:2}}",
            effet: "un numero con due decimali: 3,14",
        },
        Ligne {
            code: "{{t | default:\"testo\"}}",
            effet: "un valore predefinito se il campo è vuoto",
        },
        Ligne {
            code: "{{= quantità * prezzo | money}}",
            effet: "un campo calcolato: + − * / ( ), 20%, round(x; 2), min, max, sum",
        },
        Ligne {
            code: "{{si sconto}}…{{sinon}}…{{fin}}",
            effet: "testo condizionale (se, altrimenti, fine)",
        },
        Ligne {
            code: "{{aujourdhui}}",
            effet: "la data di oggi, inserita automaticamente",
        },
        Ligne {
            code: "# Titolo, ## Sottotitolo, ### Titolo 3",
            effet: "titoli",
        },
        Ligne {
            code: "**grassetto**, *corsivo*",
            effet: "grassetto e corsivo",
        },
        Ligne {
            code: "__sottolineato__, ~~barrato~~",
            effet: "sottolineato e barrato",
        },
        Ligne {
            code: "- voce, 1. voce",
            effet: "elenchi puntati e numerati (2 spazi di rientro per livello)",
        },
        Ligne {
            code: "> citazione",
            effet: "una citazione",
        },
        Ligne {
            code: "-> centrato <-, -> a destra ->",
            effet: "allineamento del paragrafo",
        },
        Ligne {
            code: "| a | b |  poi  |---|---:|",
            effet: "una tabella, la sua intestazione e l'allineamento delle colonne",
        },
        Ligne {
            code: "---, \\newpage",
            effet: "linea orizzontale, interruzione di pagina",
        },
        Ligne {
            code: "\\sommaire",
            effet: "un indice generato dai titoli",
        },
        Ligne {
            code: "\\note{texte}",
            effet: "una nota numerata [1], raccolta in fondo al documento",
        },
        Ligne {
            code: "$E = mc^2$, $$ \\sum_i x_i $$",
            effet: "formule LaTeX, nel testo o centrate",
        },
        Ligne {
            code: "\\{{nome}}",
            effet: "parentesi graffe letterali, senza campo",
        },
        Ligne {
            code: "\\$, \\*, \\|",
            effet: "un carattere letterale",
        },
    ],
    filtres_titre: "Filtri",
    filtres_texte: "I filtri trasformano il valore di un'occorrenza, e solo di quella. Si concatenano da sinistra a destra: {{nome | trim | upper}}. I risultati qui sotto sono calcolati da varium-core per un documento in francese; in un'altra lingua, numeri, importi e date ne seguono le convenzioni.",
    filtres_colonnes: ["Filtro", "Valore", "Risultato"],
    filtre_defaut: "Gentile cliente",
    types_titre: "Tipi di variabili",
    types_texte: "Ogni variabile ha un tipo, che sceglie il campo di inserimento, controlla il valore e dà un formato predefinito quando l'occorrenza non ha filtri: in italiano una data appare come «1º marzo 2026», un importo come «12.500,00\u{a0}€».",
    types: [
        "Testo",
        "Testo lungo",
        "Data",
        "Numero",
        "Importo",
        "E-mail",
    ],
    formules_titre: "Formule",
    formules_texte: "Tra $…$ (nel testo) o $$…$$ (centrata), il testo è LaTeX, e le variabili funzionano anche lì. Solo i $ scritti nel sorgente aprono una formula: un valore come «10,00 US$» non rompe mai il documento.",
    formules_exemple: "La rata mensile è $m = \\frac{M}{n}$, con\n$$ C_n = C_0 \\times (1 + t)^n $$",
    classeur_titre: "Formule di Classeur",
    classeur_texte: "In Classeur una formula inizia con = e i nomi delle funzioni si scrivono in francese o in inglese, a scelta: =SOMME(A1:A10) e =SUM(A1:A10) danno lo stesso totale.",
    classeur_colonnes: ["Funzione (francese · inglese)", "A cosa serve"],
    classeur_fonctions: [
        "somma i valori di un intervallo",
        "restituisce un valore o un altro in base a una condizione",
        "cerca un valore nella prima colonna di un intervallo e restituisce quello di un'altra colonna",
    ],
    classeur_note: "In una presentazione i {{campi}} si scrivono come in un documento.",
    exports_titre: "Esportazioni",
    exports_noms: ["Testo", "Modello", "Progetto"],
    exports: [
        "nativo, senza browser, con pagine del formato giusto, intestazione e piè di pagina.",
        "un documento Word, leggibile anche da LibreOffice e Google Docs.",
        "un documento LibreOffice Writer, con i suoi stili, l'intestazione e i numeri di pagina.",
        "per Pages e TextEdit.",
        "un libro digitale.",
        "un file autonomo, con le formule composte da KaTeX, pronto da stampare.",
        "un file .tex completo, da compilare così com'è.",
        "il documento finale, senza formattazione.",
        "il sorgente grezzo, con le sue {{variabili}}.",
        "un file .varium.json con il sorgente, i valori e i tipi.",
    ],
    api_titre: "API di rendering",
    api_intro_statique: "Il server di questo sito (cd site && cargo run) espone il motore in JSON, da provare da un terminale:",
    api_intro_serveur: "Questo sito espone il motore in JSON, da provare da un terminale:",
    api_reponse: "La risposta contiene il testo generato (texte), l'esportazione HTML completa (html) e le variabili rilevate con il numero delle loro occorrenze (variables).",

    // ---- Installa
    installer_description: "Scarica Varium, la suite per l'ufficio con variabili (Documents, Classeur, Présentation), per Windows, macOS e Linux.",
    installer_chapo: "Varium è una suite per l'ufficio gratuita per Windows, macOS e Linux: Documents, Classeur e Présentation. Nessun account, nessuna versione online: i tuoi file restano sul tuo computer.",
    telecharger_pour: "Scarica per {}",
    systemes: [
        Systeme {
            compat: "Windows 10 e 11, 64 bit",
            etapes: [
                "Decomprimi il file (clic destro → Estrai tutto).",
                "Avvia varium.exe.",
                "Se Windows mostra «Windows ha protetto il PC»: «Ulteriori informazioni» → «Esegui comunque».",
            ],
        },
        Systeme {
            compat: "macOS 11 o successivo, Apple Silicon e Intel",
            etapes: [
                "Decomprimi il file e trascina Varium nella cartella Applicazioni.",
                "Al primo avvio: clic destro su Varium → Apri → Apri.",
                "Se macOS lo blocca ancora: Impostazioni di Sistema → Privacy e sicurezza → «Apri comunque».",
            ],
        },
        Systeme {
            compat: "x86_64, glibc 2.35 o successiva (Ubuntu 22.04, Debian 12, Fedora 36…)",
            etapes: [
                "Decomprimi: tar xzf varium-linux-x86_64.tar.gz",
                "Avvia: ./varium/varium",
                "Per il menu delle applicazioni: copia varium in ~/.local/bin, varium.desktop in ~/.local/share/applications e varium.svg in ~/.local/share/icons.",
            ],
        },
    ],
    versions: "Tutte le versioni e le relative note: {}. Le app non sono ancora firmate da Apple né da Microsoft, da qui l'avviso al primo avvio.",
    versions_lien: "pagina delle versioni su GitHub",
    source_titre: "Dal codice sorgente",
    source_texte: "Con Rust (rustup.rs), il comando varium si installa su qualsiasi sistema:",
    source_linux: "Su Linux, Varium usa le consuete librerie grafiche (libxkbcommon, Wayland o X11, un driver OpenGL o Vulkan), già presenti su qualsiasi desktop.",
    source_github: "Codice sorgente su GitHub",

    // ---- Prova
    essayer_titre: "Prova Varium",
    essayer_description: [
        "Scrivi un modello con delle {{variabili}}, dai loro un valore e guarda il documento aggiornarsi: la demo online di Varium.",
        "Un modello con delle {{variabili}}, i loro valori e il documento generato da Varium. La suite completa si installa sul tuo computer.",
    ],
    essayer_explication: [
        "Modifica il modello o i valori, poi aggiorna il documento. Il risultato è calcolato dal vero motore di Varium (varium-core), sul server. L'esempio è in francese, come i suoi formati di data e importo.",
        "La lettera di esempio di Varium, in francese: il modello e i suoi valori a sinistra, il documento calcolato dal vero motore (varium-core) a destra.",
    ],
    essayer_installer: Item {
        titre: "Per scrivere i tuoi modelli, installa Varium.",
        texte: "La suite completa è un'applicazione desktop per Linux, macOS e Windows: impaginazione, campi calcolati, stampa unione, fogli di calcolo, presentazioni, esportazione in ODT, DOCX e PDF. I tuoi documenti restano sul tuo computer.",
    },
    modele: "Modello",
    modele_aide: "scrivi {{una_variabile}} dove vuoi",
    lecture_seule: "sola lettura",
    variables: "Variabili ({})",
    aucune_variable: "Ancora nessuna variabile: aggiungi {{nome}} al modello e aggiorna.",
    mettre_a_jour: "Aggiorna il documento",
    reinitialiser: "Reimposta",
    variables_colonnes: ["Variabile", "Tipo", "Valore"],
    a_remplir: "da compilare",
    document: "Documento",
    compteur: [
        "{} variabile",
        "{} variabili",
        "{} occorrenza",
        "{} occorrenze",
    ],
    a_remplir_n: "{} da compilare",
    texte_brut: "Testo semplice",
    sans_valeur: "«{}» non ha ancora un valore",
    saut_de_page: "interruzione di pagina",
    type_de: "Tipo di {}",
    format_date: "AAAA-MM-GG",
    erreurs: [
        "Data attesa: AAAA-MM-GG",
        "Numero atteso",
        "Indirizzo e-mail non valido",
    ],
    tronque: "Il modello è stato troncato a {} caratteri.",

    // ---- Non trovata
    introuvable_titre: "Pagina non trovata",
    introuvable_description: "Questa pagina non esiste.",
    introuvable_h1: "{{pagina}} non trovata",
    introuvable_texte: "Questa variabile non ha valore: la pagina richiesta non esiste.",
    retour_accueil: "Torna alla home",
};
