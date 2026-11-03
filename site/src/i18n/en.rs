//! English.

use super::{Appli, Item, Ligne, Systeme, Textes};

pub const TEXTES: Textes = Textes {
    // ---- Common
    accroche: "One document. Every possibility.",
    nav: ["Overview", "Syntax", "Install", "Try it"],
    nav_aria: "Main navigation",
    accueil_aria: "Varium, home",
    langue_choisir: "Choose your language",
    installer_varium: "Install Varium",
    code_source: "Source code",
    pied: "Varium, an office suite with variables, written in Rust.",
    pied_paquets: "This site is built with {}, the full-stack Rust framework.",
    lire_syntaxe: "Read the syntax",
    voir_exemple: "See an example",

    // ---- Home
    description_accueil: "Varium is an office suite written in Rust: Documents, Classeur (spreadsheets) and Présentation (slides). Write {{first_name}} once, change its value, and everything follows. Six writing languages, for Linux, macOS and Windows.",
    surtitre: "Office suite with variables · Rust",
    chapo: "Documents, Classeur and Présentation: a complete office suite that runs on your computer. Write {{first_name}}, {{last_name}} or {{amount}} in a text or on a slide, change a value just once, and every occurrence updates.",
    sous_hero: "Desktop app for Linux, macOS and Windows · your documents stay with you · 100% Rust",
    apercu: ["The template", "The document"],
    apercu_note: "A French example, rendered by Varium's real engine (varium-core) when this page was generated.",
    applis_titre: "Three applications, one Varium",
    applis_intro: "A word processor, a spreadsheet and a presentation tool, together in a single desktop application.",
    applis: [
        Appli {
            genre: "Word processor",
            texte: "Letters, quotes and contracts that fill themselves in.",
            points: [
                "Headings, lists, tables and real A4, A5 or Letter pages",
                "Global, calculated and conditional {{…}} fields, mail merge",
                "A Writer-style toolbar and a Pages-style inspector",
                "Export to PDF, DOCX, ODT, RTF, EPUB, HTML and LaTeX",
                "Spell checking in six languages, with real Hunspell dictionaries",
            ],
        },
        Appli {
            genre: "Spreadsheet",
            texte: "Worksheets with cells, formulas and charts.",
            points: [
                "Cells, formulas and several sheets per workbook",
                "Function names in French or English: SOMME or SUM, SI or IF, RECHERCHEV or VLOOKUP",
                "Conditional formatting, colour scales, data bars and frozen panes",
                "Eight charts: bars, stacked bars, lines, areas, pie, doughnut, scatter",
                "Opens and saves Excel (XLSX) and LibreOffice (ODS) files, plus CSV",
            ],
        },
        Appli {
            genre: "Slideshows",
            texte: "Presentations ready to project, with your fields inside.",
            points: [
                "Layouts, themes and transitions",
                "Images and shapes placed with the mouse, {{…}} fields on your slides",
                "Presenter mode on a second screen: notes, next slide, timer",
                "Opens PowerPoint (PPTX) and Impress (ODP) presentations",
                "Export to HTML, ODP and PDF",
            ],
        },
    ],
    langues_titre: "Write in six languages",
    langues_intro: "Each document has its own language: French, English, Spanish, German, Italian or Portuguese. Varium follows its writing rules.",
    langues: [
        Item {
            titre: "Automatic typography",
            texte: "Quotation marks «\u{a0}» in French, “\u{a0}” in English or „\u{a0}“ in German, typographic apostrophes and, in French, the thin space before ;\u{a0}:\u{a0}!\u{a0}? all appear on their own.",
        },
        Item {
            titre: "Proofreading",
            texte: "The Révision checker flags double spaces, repeated words, a missing capital letter, a space before punctuation, a missing ¿ or ¡ in Spanish, “a” or “an” in English…",
        },
        Item {
            titre: "Dates, numbers and currencies",
            texte: "Each language has its own formats: 12\u{202f}500,50 in French, 12,500.50 in English, 12.500,50 in German; dates, month names and amounts follow the document's language.",
        },
        Item {
            titre: "Spelling",
            texte: "Mistakes are underlined as you type, with suggestions on right-click. Varium uses the dictionaries already installed or downloads them from LibreOffice, and remembers your words in a personal dictionary.",
        },
    ],
    langues_note: "Varium's interface is available in these six languages; the language of the text is chosen for each document.",
    fonctions_titre: "Everything you need for repetitive documents",
    fonctions_intro: "Letters, quotes, contracts, certificates: one template, as many documents as you need.",
    fonctions: [
        Item {
            titre: "Global fields",
            texte: "A value typed once updates every occurrence, in a document as in a presentation. {{first_name}} and {{last_name}} always stay separate.",
        },
        Item {
            titre: "Calculated fields",
            texte: "{{= quantity * price | money}}: totals, VAT, monthly payments and percentages recalculate themselves.",
        },
        Item {
            titre: "Conditional text",
            texte: "{{si discount}}…{{sinon}}…{{fin}} (if, else, end): a paragraph only appears when the field is filled in.",
        },
        Item {
            titre: "Mail merge",
            texte: "Give it a CSV table from Excel or Calc, and Varium produces one document per recipient, in a single file.",
        },
        Item {
            titre: "Ready-made templates",
            texte: "Letter, invoice, quote, certificate, contract, notice of meeting: one click to get started.",
        },
        Item {
            titre: "Typed fields and filters",
            texte: "Date with a calendar, amount, number, email, formatted for the document's language; filters such as upper, date:full, money:usd…",
        },
        Item {
            titre: "Real pages",
            texte: "Page-by-page preview in A4, A5 or Letter format, with margins, header, footer, page numbers and zoom.",
        },
        Item {
            titre: "LibreOffice and Microsoft Office exports",
            texte: "Documents as native PDF, DOCX, ODT, RTF and EPUB; workbooks as XLSX, ODS and CSV; presentations as ODP, HTML and PDF.",
        },
        Item {
            titre: "Files, the Writer way",
            texte: "Ctrl+S, “Save As”, recent documents, a prompt before closing and automatic backup.",
        },
        Item {
            titre: "Find, replace, undo",
            texte: "Match case or not, replace one by one or all at once, undo and redo with no practical limit.",
        },
        Item {
            titre: "LaTeX formulas",
            texte: "$…$ inline, $$…$$ centered. A $ that comes from a value (US$10.00) never opens a formula.",
        },
        Item {
            titre: "A real Mac app",
            texte: "Menus in the menu bar, ⌘ shortcuts, an app icon, full screen and focus mode; also for Windows and Linux.",
        },
    ],
    etapes_titre: "How it works",
    etapes: [
        Item {
            titre: "Write",
            texte: "Type your text and put {{variables}} wherever it changes from one document to the next.",
        },
        Item {
            titre: "Fill in",
            texte: "Varium detects each variable and gives you a field of the right type: date, amount, email…",
        },
        Item {
            titre: "Export",
            texte: "The final document, formulas included, as PDF, DOCX, ODT, HTML, LaTeX or plain text.",
        },
    ],
    bureau: Item {
        titre: "On your desktop",
        texte: "A fast native application built with egui, for Linux, macOS and Windows.",
    },
    prive: Item {
        titre: "Your documents stay with you",
        texte: "No account, no server, no online version: Varium saves your projects on your disk, and nothing leaves your computer.",
    },
    capot_titre: "Under the hood",
    capot_intro: "A Cargo workspace: pure engines with no user interface, and the application that brings them together.",
    capot_colonnes: ["Crate", "Role"],
    capot_roles: [
        "The document engine: parsing, rendering, filters, formulas, languages, exports, project format. Tested, with no graphics dependency.",
        "The spreadsheet engine: cells, formulas, sheets and exports.",
        "The presentation engine: slides, layouts, themes and exports.",
        "The egui/eframe desktop application that brings the three modules together, for Linux, macOS and Windows.",
    ],
    appel_titre: "Ready to try it?",
    appel_texte: "Install Varium on your computer: write your templates, fill in the values, calculate, present, export.",

    // ---- Syntax
    syntaxe_description: "Varium's syntax: {{…}} variables, filters, types, LaTeX formulas, spreadsheet functions and exports.",
    syntaxe_chapo: "Everything you can write in a Varium document.",
    sommaire: [
        "Variables",
        "Filters",
        "Types",
        "Formulas",
        "Spreadsheet",
        "Exports",
        "API",
    ],
    ecrire_titre: "Writing variables",
    ecrire_texte: "A variable goes between double curly braces. Names may contain accented letters, digits, _, . and -: {{first_name}}, {{client.city}}, {{date-1}}.",
    syntaxe_colonnes: ["You write", "Result"],
    syntaxe: [
        Ligne {
            code: "{{name}}",
            effet: "a field: the same value everywhere",
        },
        Ligne {
            code: "{{ name | upper | trim }}",
            effet: "a field with chained filters",
        },
        Ligne {
            code: "{{amount | money}}",
            effet: "an amount in euros (money:usd, money:gbp, money:chf…)",
        },
        Ligne {
            code: "{{date | date}}",
            effet: "a long date in the document's language: 15 March 2026 (date:full, date:short)",
        },
        Ligne {
            code: "{{x | number:2}}",
            effet: "a number with two decimals: 3.14",
        },
        Ligne {
            code: "{{t | default:\"text\"}}",
            effet: "a default value when the field is empty",
        },
        Ligne {
            code: "{{= quantity * price | money}}",
            effet: "a calculated field: + − * / ( ), 20%, round(x; 2), min, max, sum",
        },
        Ligne {
            code: "{{si discount}}…{{sinon}}…{{fin}}",
            effet: "conditional text (if, else, end)",
        },
        Ligne {
            code: "{{aujourdhui}}",
            effet: "today's date, filled in automatically",
        },
        Ligne {
            code: "# Title, ## Subtitle, ### Heading 3",
            effet: "headings",
        },
        Ligne {
            code: "**bold**, *italic*",
            effet: "bold and italic",
        },
        Ligne {
            code: "__underlined__, ~~struck~~",
            effet: "underline and strikethrough",
        },
        Ligne {
            code: "- item, 1. item",
            effet: "bulleted and numbered lists (indent by 2 spaces per level)",
        },
        Ligne {
            code: "> quote",
            effet: "a block quote",
        },
        Ligne {
            code: "-> centered <-, -> right ->",
            effet: "paragraph alignment",
        },
        Ligne {
            code: "| a | b |  then  |---|---:|",
            effet: "a table, its header row and column alignment",
        },
        Ligne {
            code: "---, \\newpage",
            effet: "horizontal rule, page break",
        },
        Ligne {
            code: "\\sommaire",
            effet: "a table of contents generated from the headings",
        },
        Ligne {
            code: "$E = mc^2$, $$ \\sum_i x_i $$",
            effet: "LaTeX formulas, inline or centered",
        },
        Ligne {
            code: "\\{{name}}",
            effet: "literal braces, not a field",
        },
        Ligne {
            code: "\\$, \\*, \\|",
            effet: "a literal character",
        },
    ],
    filtres_titre: "Filters",
    filtres_texte: "Filters transform the value of one occurrence, and only that one. They chain from left to right: {{name | trim | upper}}. The results below are computed by varium-core for a French document; in another language, numbers, amounts and dates follow its conventions.",
    filtres_colonnes: ["Filter", "Value", "Result"],
    filtre_defaut: "Dear Sir or Madam",
    types_titre: "Variable types",
    types_texte: "Each variable has a type, which picks its input field, checks the value and gives a default format when the occurrence has no filter: in English, a date shows as “1 March 2026” and an amount as “€12,500.00”.",
    types: ["Text", "Long text", "Date", "Number", "Amount", "Email"],
    formules_titre: "Formulas",
    formules_texte: "Between $…$ (inline) or $$…$$ (centered), the text is LaTeX, and variables work there too. Only the $ signs written in the source open a formula: a value such as “US$10.00” never breaks the document.",
    formules_exemple: "The monthly payment is $m = \\frac{M}{n}$, with\n$$ C_n = C_0 \\times (1 + t)^n $$",
    classeur_titre: "Spreadsheet formulas (Classeur)",
    classeur_texte: "In Classeur, a formula starts with = and function names can be written in French or in English, as you prefer: =SOMME(A1:A10) and =SUM(A1:A10) give the same total.",
    classeur_colonnes: ["Function (French · English)", "What it does"],
    classeur_fonctions: [
        "adds up the values of a range",
        "returns one value or another depending on a condition",
        "looks up a value in the first column of a range and returns the matching one from another column",
    ],
    classeur_note: "In a presentation, {{fields}} are written just as in a document.",
    exports_titre: "Exports",
    exports_noms: ["Text", "Template", "Project"],
    exports: [
        "native, no browser needed, in pages of the right size, with header and footer.",
        "a Word document, also readable by LibreOffice and Google Docs.",
        "a LibreOffice Writer document, with its styles, header and page numbers.",
        "for Pages and TextEdit.",
        "an e-book.",
        "a self-contained file, formulas typeset by KaTeX, ready to print.",
        "a complete .tex file, ready to compile as is.",
        "the final document, without formatting.",
        "the raw source, with its {{variables}}.",
        "a .varium.json file with the source, the values and the types.",
    ],
    api_titre: "Rendering API",
    api_intro_statique: "This site's server (cd site && cargo run) exposes the engine as JSON, so you can try it from a terminal:",
    api_intro_serveur: "This site exposes the engine as JSON, so you can try it from a terminal:",
    api_reponse: "The response contains the rendered text (texte), the complete HTML export (html) and the detected variables with their number of occurrences (variables).",

    // ---- Install
    installer_description: "Download Varium, the office suite with variables (Documents, Classeur, Présentation), for Windows, macOS and Linux.",
    installer_chapo: "Varium is a free office suite for Windows, macOS and Linux: Documents, Classeur and Présentation. No account, no online version: your files stay on your computer.",
    telecharger_pour: "Download for {}",
    systemes: [
        Systeme {
            compat: "Windows 10 and 11, 64-bit",
            etapes: [
                "Unzip the file (right-click → Extract All).",
                "Run varium.exe.",
                "If Windows shows “Windows protected your PC”: “More info” → “Run anyway”.",
            ],
        },
        Systeme {
            compat: "macOS 11 or later, Apple silicon and Intel",
            etapes: [
                "Unzip the file and drag Varium into the Applications folder.",
                "On first launch: right-click Varium → Open → Open.",
                "If macOS still refuses: System Settings → Privacy & Security → “Open Anyway”.",
            ],
        },
        Systeme {
            compat: "x86_64, glibc 2.35 or newer (Ubuntu 22.04, Debian 12, Fedora 36…)",
            etapes: [
                "Extract: tar xzf varium-linux-x86_64.tar.gz",
                "Run: ./varium/varium",
                "For the applications menu: copy varium to ~/.local/bin, varium.desktop to ~/.local/share/applications and varium.svg to ~/.local/share/icons.",
            ],
        },
    ],
    versions: "All releases and their notes: {}. The apps are not yet signed by Apple or Microsoft, hence the warning on first launch.",
    versions_lien: "releases page on GitHub",
    source_titre: "From source",
    source_texte: "With Rust (rustup.rs), the varium command installs on any system:",
    source_linux: "On Linux, Varium uses the usual graphics libraries (libxkbcommon, Wayland or X11, an OpenGL or Vulkan driver), already present on any desktop.",
    source_github: "Source code on GitHub",

    // ---- Try it
    essayer_titre: "Try Varium",
    essayer_description: [
        "Write a template with {{variables}}, give them values and watch the document update: Varium's online demo.",
        "A template with {{variables}}, their values and the document rendered by Varium. The full suite installs on your computer.",
    ],
    essayer_explication: [
        "Edit the template or the values, then update the document. It is rendered by Varium's real engine (varium-core), on the server. The example is in French, and so are its date and amount formats.",
        "Varium's sample letter, in French: the template and its values on the left, the document computed by the real engine (varium-core) on the right.",
    ],
    essayer_installer: Item {
        titre: "To write your own templates, install Varium.",
        texte: "The full suite is a desktop application for Linux, macOS and Windows: page layout, calculated fields, mail merge, spreadsheets, presentations, ODT, DOCX and PDF export. Your documents stay on your computer.",
    },
    modele: "Template",
    modele_aide: "write {{a_variable}} anywhere",
    lecture_seule: "read-only",
    variables: "Variables ({})",
    aucune_variable: "No variables yet: add {{first_name}} to the template, then update.",
    mettre_a_jour: "Update the document",
    reinitialiser: "Reset",
    variables_colonnes: ["Variable", "Type", "Value"],
    a_remplir: "to fill in",
    document: "Document",
    compteur: [
        "{} variable",
        "{} variables",
        "{} occurrence",
        "{} occurrences",
    ],
    a_remplir_n: "{} to fill in",
    texte_brut: "Plain text",
    sans_valeur: "“{}” has no value yet",
    saut_de_page: "page break",
    type_de: "Type of {}",
    format_date: "YYYY-MM-DD",
    erreurs: [
        "Date expected: YYYY-MM-DD",
        "Number expected",
        "Invalid email address",
    ],
    tronque: "The template was cut to {} characters.",

    // ---- Not found
    introuvable_titre: "Page not found",
    introuvable_description: "This page does not exist.",
    introuvable_h1: "{{page}} not found",
    introuvable_texte: "This variable has no value: the page you asked for does not exist.",
    retour_accueil: "Back to home",
};
