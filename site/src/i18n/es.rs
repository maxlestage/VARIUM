//! Español.

use super::{Appli, Item, Ligne, Systeme, Textes};

pub const TEXTES: Textes = Textes {
    // ---- Común
    accroche: "Un documento. Todas sus posibilidades.",
    nav: ["Presentación", "Sintaxis", "Instalar", "Probar"],
    nav_aria: "Navegación principal",
    accueil_aria: "Varium, inicio",
    langue_choisir: "Elige tu idioma",
    installer_varium: "Instalar Varium",
    code_source: "Código fuente",
    pied: "Varium, una suite ofimática con variables escrita en Rust.",
    pied_paquets: "Este sitio está hecho con {}, el framework full-stack de Rust.",
    lire_syntaxe: "Ver la sintaxis",
    voir_exemple: "Ver un ejemplo",

    // ---- Inicio
    description_accueil: "Varium es una suite ofimática escrita en Rust: Documents, Classeur (hojas de cálculo) y Présentation (diapositivas). Escribe {{nombre}} una vez, cambia su valor y todo se actualiza. Seis idiomas de escritura, para Linux, macOS y Windows.",
    surtitre: "Suite ofimática con variables · Rust",
    chapo: "Documents, Classeur y Présentation: una suite ofimática completa que funciona en tu ordenador. Escribe {{nombre}}, {{apellido}} o {{importe}} en un texto o en una diapositiva, cambia un valor una sola vez y todas sus apariciones se actualizan.",
    sous_hero: "Aplicación de escritorio para Linux, macOS y Windows · tus documentos se quedan contigo · 100 % Rust",
    apercu: ["La plantilla", "El documento"],
    apercu_note: "Un ejemplo en francés, generado por el motor real de Varium (varium-core) al crear esta página.",
    applis_titre: "Tres aplicaciones, un solo Varium",
    applis_intro: "Un procesador de textos, una hoja de cálculo y un programa de presentaciones, reunidos en la misma aplicación de escritorio.",
    applis: [
        Appli {
            genre: "Procesador de textos",
            texte: "Cartas, presupuestos y contratos que se rellenan solos.",
            points: [
                "Títulos, listas, tablas y páginas reales en A4, A5 o Carta",
                "Campos {{…}} globales, calculados y condicionales; combinación de correspondencia",
                "Barra de herramientas al estilo de Writer, inspector al estilo de Pages",
                "Exportación a PDF, DOCX, ODT, RTF, EPUB, HTML y LaTeX",
                "Ortografía en seis idiomas, con verdaderos diccionarios Hunspell",
            ],
        },
        Appli {
            genre: "Hoja de cálculo",
            texte: "Hojas con celdas, fórmulas y gráficos.",
            points: [
                "Celdas, fórmulas y varias hojas por libro",
                "Funciones en francés o en inglés: SOMME o SUM, SI o IF, RECHERCHEV o VLOOKUP",
                "Formato condicional, escalas de color, barras de datos e inmovilizar paneles",
                "Ocho gráficos: barras, barras apiladas, líneas, áreas, sectores, anillo, dispersión",
                "Abre y guarda archivos de Excel (XLSX) y LibreOffice (ODS), además de CSV",
            ],
        },
        Appli {
            genre: "Diapositivas",
            texte: "Presentaciones listas para proyectar, con tus campos dentro.",
            points: [
                "Diseños, temas y transiciones",
                "Imágenes y formas colocadas con el ratón, campos {{…}} en las diapositivas",
                "Modo presentador en una segunda pantalla: notas, diapositiva siguiente, cronómetro",
                "Abre presentaciones de PowerPoint (PPTX) e Impress (ODP)",
                "Exportación a HTML, ODP y PDF",
            ],
        },
    ],
    langues_titre: "Escribe en seis idiomas",
    langues_intro: "Cada documento tiene su idioma: francés, inglés, español, alemán, italiano o portugués. Varium sigue sus reglas de escritura.",
    langues: [
        Item {
            titre: "Tipografía automática",
            texte: "Las comillas «\u{a0}» en francés, “\u{a0}” en inglés o „\u{a0}“ en alemán, el apóstrofo tipográfico y, en francés, el espacio fino antes de ;\u{a0}:\u{a0}!\u{a0}? se colocan solos.",
        },
        Item {
            titre: "Revisión",
            texte: "El corrector Révision señala los espacios dobles, las palabras repetidas, una mayúscula olvidada, un espacio antes de la puntuación, el ¿ o el ¡ que faltan en español, «a» o «an» en inglés…",
        },
        Item {
            titre: "Fechas, números y monedas",
            texte: "Cada idioma tiene sus formatos: 12\u{202f}500,50 en francés, 12,500.50 en inglés, 12.500,50 en alemán; las fechas, los meses y los importes siguen el idioma del documento.",
        },
        Item {
            titre: "Ortografía",
            texte: "Los errores se subrayan mientras escribe, con sugerencias al hacer clic derecho. Varium usa los diccionarios instalados o los descarga de LibreOffice, y recuerda sus palabras en un diccionario personal.",
        },
    ],
    langues_note: "La interfaz de Varium está disponible en estos seis idiomas; el idioma del texto se elige para cada documento.",
    fonctions_titre: "Todo lo necesario para tus documentos repetitivos",
    fonctions_intro: "Cartas, presupuestos, contratos, certificados: una sola plantilla y tantos documentos como necesites.",
    fonctions: [
        Item {
            titre: "Campos globales",
            texte: "Un valor escrito una vez actualiza todas sus apariciones, tanto en un documento como en una presentación. {{nombre}} y {{apellido}} siempre se mantienen separados.",
        },
        Item {
            titre: "Campos calculados",
            texte: "{{= cantidad * precio | money}}: totales, IVA, cuotas mensuales y porcentajes se recalculan solos.",
        },
        Item {
            titre: "Texto condicional",
            texte: "{{si descuento}}…{{sinon}}…{{fin}} (si, si no, fin): un párrafo solo aparece si el campo tiene valor.",
        },
        Item {
            titre: "Combinación de correspondencia",
            texte: "Con una tabla CSV de Excel o de Calc, Varium genera un documento por destinatario, en un solo archivo.",
        },
        Item {
            titre: "Plantillas listas para usar",
            texte: "Carta, factura, presupuesto, certificado, contrato, convocatoria: un clic para empezar.",
        },
        Item {
            titre: "Campos tipados y filtros",
            texte: "Fecha con calendario, importe, número, correo electrónico, con el formato del idioma del documento; filtros upper, date:full, money:usd…",
        },
        Item {
            titre: "Páginas reales",
            texte: "Vista previa página a página en A4, A5 o Carta, con márgenes, encabezado, pie de página, números de página y zoom.",
        },
        Item {
            titre: "Exportación a LibreOffice y Microsoft Office",
            texte: "Documentos en PDF nativo, DOCX, ODT, RTF y EPUB; libros en XLSX, ODS y CSV; presentaciones en ODP, HTML y PDF.",
        },
        Item {
            titre: "Archivos como en Writer",
            texte: "Ctrl+S, «Guardar como», documentos recientes, confirmación antes de cerrar y copia de seguridad automática.",
        },
        Item {
            titre: "Buscar, reemplazar, deshacer",
            texte: "Búsqueda con o sin distinción de mayúsculas, reemplazo uno a uno o de una vez, deshacer y rehacer sin límite práctico.",
        },
        Item {
            titre: "Fórmulas LaTeX",
            texte: "$…$ en línea, $$…$$ centradas. Un $ que viene de un valor (10,00 US$) nunca abre una fórmula.",
        },
        Item {
            titre: "Una auténtica app para Mac",
            texte: "Menús en la barra superior, atajos con ⌘, icono propio, pantalla completa y modo concentración; también para Windows y Linux.",
        },
    ],
    etapes_titre: "Cómo funciona",
    etapes: [
        Item {
            titre: "Escribe",
            texte: "Redacta tu texto y coloca {{variables}} allí donde cambie de un documento a otro.",
        },
        Item {
            titre: "Rellena",
            texte: "Varium detecta cada variable y te ofrece un campo del tipo adecuado: fecha, importe, correo electrónico…",
        },
        Item {
            titre: "Exporta",
            texte: "El documento final, fórmulas incluidas, en PDF, DOCX, ODT, HTML, LaTeX o texto.",
        },
    ],
    bureau: Item {
        titre: "En el escritorio",
        texte: "Una aplicación nativa y rápida, hecha con egui, para Linux, macOS y Windows.",
    },
    prive: Item {
        titre: "Tus documentos se quedan contigo",
        texte: "Sin cuenta, sin servidor, sin versión en línea: Varium guarda tus proyectos en tu disco y nada sale de tu ordenador.",
    },
    capot_titre: "Bajo el capó",
    capot_intro: "Un workspace de Cargo: motores puros, sin interfaz, y la aplicación que los reúne.",
    capot_colonnes: ["Paquete", "Función"],
    capot_roles: [
        "El motor de documentos: análisis, renderizado, filtros, fórmulas, idiomas, exportación y formato de proyecto. Probado y sin dependencias gráficas.",
        "El motor de hojas de cálculo: celdas, fórmulas, hojas y exportación.",
        "El motor de presentaciones: diapositivas, diseños, temas y exportación.",
        "La aplicación de escritorio egui/eframe que reúne los tres módulos, para Linux, macOS y Windows.",
    ],
    appel_titre: "¿Listo para probarlo?",
    appel_texte: "Instala Varium en tu ordenador: escribe tus plantillas, rellena los valores, calcula, presenta y exporta.",

    // ---- Sintaxis
    syntaxe_description: "La sintaxis de Varium: variables {{…}}, filtros, tipos, fórmulas LaTeX, funciones de la hoja de cálculo y exportación.",
    syntaxe_chapo: "Todo lo que se puede escribir en un documento de Varium.",
    sommaire: [
        "Variables",
        "Filtros",
        "Tipos",
        "Fórmulas",
        "Hoja de cálculo",
        "Exportación",
        "API",
    ],
    ecrire_titre: "Escribir variables",
    ecrire_texte: "Una variable se escribe entre llaves dobles. Los nombres admiten tildes, cifras, _, . y -: {{nombre}}, {{cliente.ciudad}}, {{fecha-1}}.",
    syntaxe_colonnes: ["Escritura", "Efecto"],
    syntaxe: [
        Ligne {
            code: "{{nombre}}",
            effet: "un campo: el mismo valor en todas partes",
        },
        Ligne {
            code: "{{ nombre | upper | trim }}",
            effet: "un campo con filtros encadenados",
        },
        Ligne {
            code: "{{importe | money}}",
            effet: "un importe en euros (money:usd, money:gbp, money:chf…)",
        },
        Ligne {
            code: "{{fecha | date}}",
            effet: "una fecha larga en el idioma del documento: 15 de marzo de 2026 (date:full, date:short)",
        },
        Ligne {
            code: "{{x | number:2}}",
            effet: "un número con dos decimales: 3,14",
        },
        Ligne {
            code: "{{t | default:\"texto\"}}",
            effet: "un valor por defecto si el campo está vacío",
        },
        Ligne {
            code: "{{= cantidad * precio | money}}",
            effet: "un campo calculado: + − * / ( ), 20%, round(x; 2), min, max, sum",
        },
        Ligne {
            code: "{{si descuento}}…{{sinon}}…{{fin}}",
            effet: "texto condicional (si, si no, fin)",
        },
        Ligne {
            code: "{{aujourdhui}}",
            effet: "la fecha de hoy, rellenada automáticamente",
        },
        Ligne {
            code: "# Título, ## Subtítulo, ### Título 3",
            effet: "títulos",
        },
        Ligne {
            code: "**negrita**, *cursiva*",
            effet: "negrita y cursiva",
        },
        Ligne {
            code: "__subrayado__, ~~tachado~~",
            effet: "subrayado y tachado",
        },
        Ligne {
            code: "- elemento, 1. elemento",
            effet: "listas con viñetas y numeradas (2 espacios de sangría por nivel)",
        },
        Ligne {
            code: "> cita",
            effet: "una cita",
        },
        Ligne {
            code: "-> centrado <-, -> a la derecha ->",
            effet: "alineación del párrafo",
        },
        Ligne {
            code: "| a | b |  luego  |---|---:|",
            effet: "una tabla, su encabezado y la alineación de las columnas",
        },
        Ligne {
            code: "---, \\newpage",
            effet: "línea horizontal, salto de página",
        },
        Ligne {
            code: "\\sommaire",
            effet: "un índice generado a partir de los títulos",
        },
        Ligne {
            code: "\\note{texte}",
            effet: "una nota numerada [1], reunida al final del documento",
        },
        Ligne {
            code: "$E = mc^2$, $$ \\sum_i x_i $$",
            effet: "fórmulas LaTeX, en línea o centradas",
        },
        Ligne {
            code: "\\{{nombre}}",
            effet: "llaves literales, sin campo",
        },
        Ligne {
            code: "\\$, \\*, \\|",
            effet: "un carácter literal",
        },
    ],
    filtres_titre: "Filtros",
    filtres_texte: "Los filtros transforman el valor de una aparición, y solo de esa. Se encadenan de izquierda a derecha: {{nombre | trim | upper}}. Los resultados de abajo los calcula varium-core para un documento en francés; en otro idioma, los números, los importes y las fechas siguen sus convenciones.",
    filtres_colonnes: ["Filtro", "Valor", "Resultado"],
    filtre_defaut: "Estimado/a cliente",
    types_titre: "Tipos de variables",
    types_texte: "Cada variable tiene un tipo, que elige su campo de entrada, comprueba el valor y da un formato por defecto cuando la aparición no tiene filtro: en español, una fecha se muestra «1 de marzo de 2026» y un importe «12.500,00\u{a0}€».",
    types: [
        "Texto",
        "Texto largo",
        "Fecha",
        "Número",
        "Importe",
        "Correo electrónico",
    ],
    formules_titre: "Fórmulas",
    formules_texte: "Entre $…$ (en línea) o $$…$$ (centrada), el texto es LaTeX, y las variables también funcionan ahí. Solo los $ escritos en la fuente abren una fórmula: un valor como «10,00 US$» nunca rompe el documento.",
    formules_exemple: "La cuota mensual es $m = \\frac{M}{n}$, con\n$$ C_n = C_0 \\times (1 + t)^n $$",
    classeur_titre: "Fórmulas de la hoja de cálculo (Classeur)",
    classeur_texte: "En Classeur, una fórmula empieza por = y las funciones se escriben en francés o en inglés, como prefieras: =SOMME(A1:A10) y =SUM(A1:A10) dan el mismo total.",
    classeur_colonnes: ["Función (francés · inglés)", "Qué hace"],
    classeur_fonctions: [
        "suma los valores de un rango",
        "devuelve un valor u otro según una condición",
        "busca un valor en la primera columna de un rango y devuelve el de otra columna",
    ],
    classeur_note: "En una presentación, los {{campos}} se escriben igual que en un documento.",
    exports_titre: "Exportación",
    exports_noms: ["Texto", "Plantilla", "Proyecto"],
    exports: [
        "nativo, sin navegador, con páginas del tamaño correcto, encabezado y pie de página.",
        "un documento de Word, que también se abre en LibreOffice y Google Docs.",
        "un documento de LibreOffice Writer, con sus estilos, su encabezado y sus números de página.",
        "para Pages y TextEdit.",
        "un libro electrónico.",
        "un archivo autónomo, con las fórmulas compuestas por KaTeX, listo para imprimir.",
        "un .tex completo, para compilar tal cual.",
        "el documento final, sin formato.",
        "la fuente sin procesar, con sus {{variables}}.",
        "un archivo .varium.json con la fuente, los valores y los tipos.",
    ],
    api_titre: "API de renderizado",
    api_intro_statique: "El servidor de este sitio (cd site && cargo run) expone el motor en JSON, para probarlo desde un terminal:",
    api_intro_serveur: "Este sitio expone el motor en JSON, para probarlo desde un terminal:",
    api_reponse: "La respuesta contiene el texto generado (texte), la exportación HTML completa (html) y las variables detectadas con su número de apariciones (variables).",

    // ---- Instalar
    installer_description: "Descarga Varium, la suite ofimática con variables (Documents, Classeur, Présentation), para Windows, macOS y Linux.",
    installer_chapo: "Varium es una suite ofimática gratuita para Windows, macOS y Linux: Documents, Classeur y Présentation. Sin cuenta ni versión en línea: tus archivos se quedan en tu ordenador.",
    telecharger_pour: "Descargar para {}",
    systemes: [
        Systeme {
            compat: "Windows 10 y 11, 64 bits",
            etapes: [
                "Descomprime el archivo (clic derecho → Extraer todo).",
                "Abre varium.exe.",
                "Si Windows muestra «Windows protegió su PC»: «Más información» → «Ejecutar de todas formas».",
            ],
        },
        Systeme {
            compat: "macOS 11 o posterior, Apple Silicon e Intel",
            etapes: [
                "Descomprime el archivo y arrastra Varium a la carpeta Aplicaciones.",
                "La primera vez: clic derecho en Varium → Abrir → Abrir.",
                "Si macOS sigue sin abrirlo: Ajustes del Sistema → Privacidad y seguridad → «Abrir igualmente».",
            ],
        },
        Systeme {
            compat: "x86_64, glibc 2.35 o posterior (Ubuntu 22.04, Debian 12, Fedora 36…)",
            etapes: [
                "Descomprime: tar xzf varium-linux-x86_64.tar.gz",
                "Ejecuta: ./varium/varium",
                "Para el menú de aplicaciones: copia varium en ~/.local/bin, varium.desktop en ~/.local/share/applications y varium.svg en ~/.local/share/icons.",
            ],
        },
    ],
    versions: "Todas las versiones y sus notas: {}. Las aplicaciones aún no están firmadas por Apple ni por Microsoft; de ahí el aviso al abrirlas por primera vez.",
    versions_lien: "página de versiones en GitHub",
    source_titre: "Desde el código fuente",
    source_texte: "Con Rust (rustup.rs), el comando varium se instala en cualquier sistema:",
    source_linux: "En Linux, Varium usa las bibliotecas gráficas habituales (libxkbcommon, Wayland o X11, un controlador OpenGL o Vulkan), ya presentes en cualquier escritorio.",
    source_github: "Código fuente en GitHub",

    // ---- Probar
    essayer_titre: "Prueba Varium",
    essayer_description: [
        "Escribe una plantilla con {{variables}}, dales un valor y mira cómo se actualiza el documento: la demo en línea de Varium.",
        "Una plantilla con {{variables}}, sus valores y el documento generado por Varium. La suite completa se instala en tu ordenador.",
    ],
    essayer_explication: [
        "Modifica la plantilla o los valores y actualiza el documento. Lo genera el motor real de Varium (varium-core), en el servidor. El ejemplo está en francés, igual que sus formatos de fecha e importe.",
        "La carta de ejemplo de Varium, en francés: la plantilla y sus valores a la izquierda, el documento calculado por el motor real (varium-core) a la derecha.",
    ],
    essayer_installer: Item {
        titre: "Para escribir tus propias plantillas, instala Varium.",
        texte: "La suite completa es una aplicación de escritorio para Linux, macOS y Windows: maquetación, campos calculados, combinación de correspondencia, hojas de cálculo, presentaciones, exportación a ODT, DOCX y PDF. Tus documentos se quedan en tu ordenador.",
    },
    modele: "Plantilla",
    modele_aide: "escribe {{una_variable}} donde quieras",
    lecture_seule: "solo lectura",
    variables: "Variables ({})",
    aucune_variable: "Todavía no hay variables: añade {{nombre}} a la plantilla y actualiza.",
    mettre_a_jour: "Actualizar el documento",
    reinitialiser: "Restablecer",
    variables_colonnes: ["Variable", "Tipo", "Valor"],
    a_remplir: "por rellenar",
    document: "Documento",
    compteur: [
        "{} variable",
        "{} variables",
        "{} aparición",
        "{} apariciones",
    ],
    a_remplir_n: "{} por rellenar",
    texte_brut: "Texto sin formato",
    sans_valeur: "«{}» todavía no tiene valor",
    saut_de_page: "salto de página",
    type_de: "Tipo de {}",
    format_date: "AAAA-MM-DD",
    erreurs: [
        "Se espera una fecha: AAAA-MM-DD",
        "Se espera un número",
        "Correo electrónico no válido",
    ],
    tronque: "La plantilla se ha recortado a {} caracteres.",

    // ---- No encontrada
    introuvable_titre: "Página no encontrada",
    introuvable_description: "Esta página no existe.",
    introuvable_h1: "{{página}} no encontrada",
    introuvable_texte: "Esta variable no tiene valor: la página que buscas no existe.",
    retour_accueil: "Volver al inicio",
};
