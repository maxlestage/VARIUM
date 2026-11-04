//! Português (de Portugal, como a língua de documento da aplicação).

use super::{Appli, Item, Ligne, Systeme, Textes};

pub const TEXTES: Textes = Textes {
    // ---- Comum
    accroche: "Um documento. Todas as possibilidades.",
    nav: ["Início", "Sintaxe", "Instalar", "Experimentar"],
    nav_aria: "Navegação principal",
    accueil_aria: "Varium, página inicial",
    langue_choisir: "Escolher o idioma",
    installer_varium: "Instalar o Varium",
    code_source: "Código-fonte",
    pied: "Varium, uma suite de escritório com variáveis, escrita em Rust.",
    pied_paquets: "Este site foi feito com {}, o framework full-stack para Rust.",
    lire_syntaxe: "Ler a sintaxe",
    voir_exemple: "Ver um exemplo",

    // ---- Página inicial
    description_accueil: "O Varium é uma suite de escritório escrita em Rust: Documents, Classeur (folhas de cálculo) e Présentation (diapositivos). Escreva {{nome}} uma vez, mude o seu valor e tudo acompanha. Seis línguas de escrita, para Linux, macOS e Windows.",
    surtitre: "Suite de escritório com variáveis · Rust",
    chapo: "Documents, Classeur e Présentation: uma suite de escritório completa que funciona no seu computador. Escreva {{nome}}, {{apelido}} ou {{montante}} num texto ou num diapositivo, mude um valor uma única vez e todas as ocorrências são atualizadas.",
    sous_hero: "Aplicação para Linux, macOS e Windows · os seus documentos ficam consigo · 100% Rust",
    apercu: ["O modelo", "O documento"],
    apercu_note: "Um exemplo em francês, gerado pelo verdadeiro motor do Varium (varium-core) ao criar esta página.",
    applis_titre: "Três aplicações, um só Varium",
    applis_intro: "Um processador de texto, uma folha de cálculo e um programa de apresentações, reunidos na mesma aplicação.",
    applis: [
        Appli {
            genre: "Processador de texto",
            texte: "Cartas, orçamentos e contratos que se preenchem sozinhos.",
            points: [
                "Títulos, listas, tabelas e páginas reais em A4, A5 ou Letter",
                "Campos {{…}} globais, calculados e condicionais, impressão em série, deteção automática de campos",
                "Barra de ferramentas ao estilo do Writer, inspetor ao estilo do Pages",
                "Exportação para PDF, DOCX, ODT, RTF, EPUB, HTML e LaTeX, e um controlo «Pronto para enviar» antes do PDF",
                "Ortografia em seis línguas, com verdadeiros dicionários Hunspell",
            ],
        },
        Appli {
            genre: "Folha de cálculo",
            texte: "Folhas com células, fórmulas e gráficos.",
            points: [
                "Células, fórmulas e várias folhas por livro, e perguntas em linguagem simples",
                "Funções em francês ou em inglês: SOMME ou SUM, SI ou IF, RECHERCHEV ou VLOOKUP",
                "Formatação condicional, escalas de cores, barras de dados e painéis fixos",
                "Oito gráficos: barras, barras empilhadas, linhas, áreas, circular, anel, dispersão",
                "Abre e guarda ficheiros Excel (XLSX) e LibreOffice (ODS), além de CSV",
            ],
        },
        Appli {
            genre: "Apresentações",
            texte: "Diapositivos prontos a projetar, com os seus campos lá dentro.",
            points: [
                "Esquemas, temas e transições",
                "Imagens e formas colocadas com o rato, campos {{…}} nos diapositivos",
                "Modo de apresentador num segundo ecrã: notas, diapositivo seguinte, cronómetro",
                "Abre apresentações PowerPoint (PPTX) e Impress (ODP)",
                "Exportação para HTML, ODP e PDF",
            ],
        },
    ],
    langues_titre: "Escrever em seis línguas",
    langues_intro: "Cada documento tem a sua língua: francês, inglês, espanhol, alemão, italiano ou português. O Varium segue as respetivas regras de escrita.",
    langues: [
        Item {
            titre: "Tipografia automática",
            texte: "As aspas «\u{a0}» em francês, “\u{a0}” em inglês ou „\u{a0}“ em alemão, o apóstrofo tipográfico e, em francês, o espaço fino antes de ;\u{a0}:\u{a0}!\u{a0}? surgem sozinhos.",
        },
        Item {
            titre: "Revisão",
            texte: "O corretor Révision assinala espaços duplos, palavras repetidas, maiúsculas esquecidas, espaços antes da pontuação, um ¿ ou um ¡ em falta em espanhol, “a” ou “an” em inglês…",
        },
        Item {
            titre: "Datas, números e moedas",
            texte: "Cada língua tem os seus formatos: 12\u{202f}500,50 em francês, 12,500.50 em inglês, 12.500,50 em alemão; as datas, os meses e os montantes seguem a língua do documento.",
        },
        Item {
            titre: "Ortografia",
            texte: "Os erros são sublinhados enquanto escreve, com sugestões no clique direito. O Varium usa os dicionários instalados ou descarrega-os do LibreOffice, e guarda as suas palavras num dicionário pessoal.",
        },
    ],
    langues_note: "A interface do Varium existe nestas seis línguas; a língua do texto escolhe-se para cada documento.",
    fonctions_titre: "Tudo o que precisa para os seus documentos repetitivos",
    fonctions_intro: "Cartas, orçamentos, contratos, declarações: um só modelo, tantos documentos quantos forem precisos.",
    fonctions: [
        Item {
            titre: "Campos globais",
            texte: "Um valor escrito uma vez atualiza todas as ocorrências, num documento como numa apresentação. {{nome}} e {{apelido}} ficam sempre separados.",
        },
        Item {
            titre: "Campos calculados",
            texte: "{{= quantidade * preço | money}}: totais, IVA, prestações mensais e percentagens recalculam-se sozinhos.",
        },
        Item {
            titre: "Texto condicional",
            texte: "{{si desconto}}…{{sinon}}…{{fin}} (se, senão, fim): um parágrafo só aparece se o campo estiver preenchido.",
        },
        Item {
            titre: "Impressão em série",
            texte: "Basta uma tabela CSV do Excel ou do Calc: o Varium produz um documento por destinatário, num único ficheiro.",
        },
        Item {
            titre: "Modelos prontos a usar",
            texte: "Carta, fatura, orçamento, declaração, contrato, convocatória: um clique para começar.",
        },
        Item {
            titre: "Campos com tipo e filtros",
            texte: "Data com calendário, montante, número, e-mail, no formato da língua do documento; filtros upper, date:full, money:usd…",
        },
        Item {
            titre: "Páginas reais",
            texte: "Pré-visualização página a página em A4, A5 ou Letter, com margens, cabeçalho, rodapé, números de página e zoom.",
        },
        Item {
            titre: "Exportação para LibreOffice e Microsoft Office",
            texte: "Documentos em PDF nativo, DOCX, ODT, RTF e EPUB; livros em XLSX, ODS e CSV; apresentações em ODP, HTML e PDF.",
        },
        Item {
            titre: "Ficheiros como no Writer",
            texte: "Ctrl+S, “Guardar como”, documentos recentes, confirmação antes de fechar e cópia de segurança automática.",
        },
        Item {
            titre: "Procurar, substituir, anular",
            texte: "Pesquisa com ou sem distinção de maiúsculas, substituição uma a uma ou de uma só vez, anular e refazer sem limite prático.",
        },
        Item {
            titre: "Fórmulas LaTeX",
            texte: "$…$ no texto, $$…$$ centradas. Um $ vindo de um valor (10,00 US$) nunca abre uma fórmula.",
        },
        Item {
            titre: "Uma verdadeira app para Mac",
            texte: "Menus na barra de menus, atalhos ⌘, ícone próprio, ecrã inteiro e modo de concentração; também para Windows e Linux.",
        },
        Item {
            titre: "Paleta de comandos ⌘K",
            texte: "Ctrl+K ou ⌘K: escreva algumas letras e lance qualquer comando dos três módulos; F1 mostra a cábula dos atalhos.",
        },
        Item {
            titre: "Centro de início",
            texte: "Modelos, ficheiros recentes e dicas logo ao abrir; um ficheiro arrastado, ou aberto com duplo clique no Finder ou no Explorador, vai para o módulo certo.",
        },
        Item {
            titre: "Preenchimento rápido",
            texte: "Ctrl+E no Classeur: o Varium adivinha o resto da coluna a partir de um exemplo, com preenchimento automático, colagem inteligente e análise rápida.",
        },
        Item {
            titre: "Tabela dinâmica",
            texte: "Arraste as colunas para linhas, colunas e valores: totais, médias e contagens por grupo num instante.",
        },
        Item {
            titre: "Histórico de versões",
            texte: "Cada gravação guarda uma versão do documento; compare-as, com as diferenças realçadas, e volte atrás com um clique.",
        },
        Item {
            titre: "Códigos QR",
            texte: "Um código QR num documento ou num diapositivo, e um diapositivo «Partilhar» com o endereço da apresentação.",
        },
        Item {
            titre: "Embelezar e Harmonizar",
            texte: "Paginações propostas a partir do conteúdo do diapositivo e depois toda a apresentação coerente, com dez temas.",
        },
        Item {
            titre: "Modo apresentador",
            texte: "O público no projetor e, para si, as notas, o diapositivo seguinte, o cronómetro, o ponteiro laser, a caneta e o zoom.",
        },
        Item {
            titre: "Modo quiosque e álbum de fotografias",
            texte: "Uma apresentação em ciclo, sem comandos, que volta ao início após inatividade; um álbum criado a partir de uma pasta de imagens, ordenado por data.",
        },
        Item {
            titre: "Importação do Word, LibreOffice e PowerPoint",
            texte: "Abra ficheiros .docx e .odt no Documents, e apresentações .pptx com as cores do tema, gradientes, rotações e tabelas.",
        },
        Item {
            titre: "Comentários e controlo de alterações",
            texte: "Comente uma seleção, proponha inserções e eliminações e aceite ou recuse cada alteração.",
        },
        Item {
            titre: "Notas de rodapé",
            texte: "Verdadeiras notas no fundo da página, numeradas automaticamente, na pré-visualização e no PDF.",
        },
        Item {
            titre: "Assinatura manuscrita",
            texte: "Assine com o rato ou o trackpad e a assinatura é colocada no documento.",
        },
        Item {
            titre: "Leitura em voz alta",
            texte: "O documento lido pela síntese de voz do sistema, na sua língua, para rever de ouvido.",
        },
    ],
    etapes_titre: "Como funciona",
    etapes: [
        Item {
            titre: "Escreva",
            texte: "Redija o seu texto e coloque {{variáveis}} onde ele muda de um documento para outro.",
        },
        Item {
            titre: "Preencha",
            texte: "O Varium deteta cada variável e propõe-lhe um campo do tipo certo: data, montante, e-mail…",
        },
        Item {
            titre: "Exporte",
            texte: "O documento final, fórmulas incluídas, em PDF, DOCX, ODT, HTML, LaTeX ou texto.",
        },
    ],
    bureau: Item {
        titre: "No computador",
        texte: "Uma aplicação nativa e rápida, feita com egui, para Linux, macOS e Windows.",
    },
    prive: Item {
        titre: "Os seus documentos ficam consigo",
        texte: "Sem conta, sem servidor, sem versão online: o Varium guarda os seus projetos no seu disco, e nada sai do seu computador.",
    },
    capot_titre: "Por dentro",
    capot_intro: "Um workspace Cargo: motores puros, sem interface, e a aplicação que os reúne.",
    capot_colonnes: ["Crate", "Função"],
    capot_roles: [
        "O motor dos documentos: análise, renderização, filtros, fórmulas, línguas, exportações, formato de projeto. Testado, sem dependências gráficas.",
        "O motor da folha de cálculo: células, fórmulas, folhas e exportações.",
        "O motor das apresentações: diapositivos, esquemas, temas e exportações.",
        "A aplicação egui/eframe que reúne os três módulos, para Linux, macOS e Windows.",
    ],
    appel_titre: "Pronto para experimentar?",
    appel_texte: "Instale o Varium no seu computador: escreva os seus modelos, preencha os valores, calcule, apresente, exporte.",

    // ---- Sintaxe
    syntaxe_description: "A sintaxe do Varium: variáveis {{…}}, filtros, tipos, fórmulas LaTeX, funções da folha de cálculo e exportações.",
    syntaxe_chapo: "Tudo o que se pode escrever num documento Varium.",
    sommaire: [
        "Variáveis",
        "Filtros",
        "Tipos",
        "Fórmulas",
        "Folha de cálculo",
        "Exportações",
        "API",
    ],
    ecrire_titre: "Escrever variáveis",
    ecrire_texte: "Uma variável escreve-se entre chavetas duplas. Os nomes aceitam acentos, algarismos, _, . e -: {{nome}}, {{cliente.cidade}}, {{data-1}}.",
    syntaxe_colonnes: ["Escrita", "Efeito"],
    syntaxe: [
        Ligne {
            code: "{{nome}}",
            effet: "um campo: o mesmo valor em todo o lado",
        },
        Ligne {
            code: "{{ nome | upper | trim }}",
            effet: "um campo com filtros encadeados",
        },
        Ligne {
            code: "{{montante | money}}",
            effet: "um montante em euros (money:usd, money:gbp, money:chf…)",
        },
        Ligne {
            code: "{{data | date}}",
            effet: "uma data por extenso na língua do documento: 15 de março de 2026 (date:full, date:short)",
        },
        Ligne {
            code: "{{x | number:2}}",
            effet: "um número com duas casas decimais: 3,14",
        },
        Ligne {
            code: "{{t | default:\"texto\"}}",
            effet: "um valor por omissão se o campo estiver vazio",
        },
        Ligne {
            code: "{{= quantidade * preço | money}}",
            effet: "um campo calculado: + − * / ( ), 20%, round(x; 2), min, max, sum",
        },
        Ligne {
            code: "{{si desconto}}…{{sinon}}…{{fin}}",
            effet: "texto condicional (se, senão, fim)",
        },
        Ligne {
            code: "{{aujourdhui}}",
            effet: "a data de hoje, preenchida automaticamente",
        },
        Ligne {
            code: "# Título, ## Subtítulo, ### Título 3",
            effet: "títulos",
        },
        Ligne {
            code: "**negrito**, *itálico*",
            effet: "negrito e itálico",
        },
        Ligne {
            code: "__sublinhado__, ~~rasurado~~",
            effet: "sublinhado e rasurado",
        },
        Ligne {
            code: "- item, 1. item",
            effet: "listas com marcas e numeradas (2 espaços de avanço por nível)",
        },
        Ligne {
            code: "> citação",
            effet: "uma citação",
        },
        Ligne {
            code: "-> centrado <-, -> à direita ->",
            effet: "alinhamento do parágrafo",
        },
        Ligne {
            code: "| a | b |  depois  |---|---:|",
            effet: "uma tabela, o seu cabeçalho e o alinhamento das colunas",
        },
        Ligne {
            code: "---, \\newpage",
            effet: "linha horizontal, quebra de página",
        },
        Ligne {
            code: "\\sommaire",
            effet: "um sumário gerado a partir dos títulos",
        },
        Ligne {
            code: "\\note{texte}",
            effet: "uma nota numerada [1], reunida no fim do documento",
        },
        Ligne {
            code: "$E = mc^2$, $$ \\sum_i x_i $$",
            effet: "fórmulas LaTeX, no texto ou centradas",
        },
        Ligne {
            code: "\\{{nome}}",
            effet: "chavetas literais, sem campo",
        },
        Ligne {
            code: "\\$, \\*, \\|",
            effet: "um carácter literal",
        },
    ],
    filtres_titre: "Filtros",
    filtres_texte: "Os filtros transformam o valor de uma ocorrência, e só dessa. Encadeiam-se da esquerda para a direita: {{nome | trim | upper}}. Os resultados abaixo são calculados pelo varium-core para um documento em francês; noutra língua, os números, os montantes e as datas seguem as suas convenções.",
    filtres_colonnes: ["Filtro", "Valor", "Resultado"],
    filtre_defaut: "Exmo.(a) Senhor(a)",
    types_titre: "Tipos de variáveis",
    types_texte: "Cada variável tem um tipo, que escolhe o campo de introdução, verifica o valor e dá um formato por omissão quando a ocorrência não tem filtro: em português, uma data aparece como “1 de março de 2026” e um montante como “12.500,00\u{a0}€”.",
    types: [
        "Texto",
        "Texto longo",
        "Data",
        "Número",
        "Montante",
        "E-mail",
    ],
    formules_titre: "Fórmulas",
    formules_texte: "Entre $…$ (no texto) ou $$…$$ (centrada), o texto é LaTeX, e as variáveis também funcionam aí. Só os $ escritos na fonte abrem uma fórmula: um valor como “10,00 US$” nunca estraga o documento.",
    formules_exemple: "A prestação mensal é $m = \\frac{M}{n}$, com\n$$ C_n = C_0 \\times (1 + t)^n $$",
    classeur_titre: "Fórmulas do Classeur",
    classeur_texte: "No Classeur, uma fórmula começa por = e as funções escrevem-se em francês ou em inglês, à escolha: =SOMME(A1:A10) e =SUM(A1:A10) dão o mesmo total.",
    classeur_colonnes: ["Função (francês · inglês)", "Para que serve"],
    classeur_fonctions: [
        "soma os valores de um intervalo",
        "devolve um valor ou outro consoante uma condição",
        "procura um valor na primeira coluna de um intervalo e devolve o de outra coluna",
    ],
    classeur_note: "Numa apresentação, os {{campos}} escrevem-se como num documento.",
    exports_titre: "Exportações",
    exports_noms: ["Texto", "Modelo", "Projeto"],
    exports: [
        "nativo, sem navegador, com páginas no formato certo, cabeçalho e rodapé.",
        "um documento Word, que também abre no LibreOffice e no Google Docs.",
        "um documento LibreOffice Writer, com os seus estilos, cabeçalho e números de página.",
        "para o Pages e o TextEdit.",
        "um livro digital.",
        "um ficheiro autónomo, com as fórmulas compostas pelo KaTeX, pronto a imprimir.",
        "um .tex completo, para compilar tal como está.",
        "o documento final, sem formatação.",
        "a fonte em bruto, com as suas {{variáveis}}.",
        "um ficheiro .varium.json com a fonte, os valores e os tipos.",
    ],
    api_titre: "API de renderização",
    api_intro_statique: "O servidor deste site (cd site && cargo run) expõe o motor em JSON, para o experimentar num terminal:",
    api_intro_serveur: "Este site expõe o motor em JSON, para o experimentar num terminal:",
    api_reponse: "A resposta contém o texto gerado (texte), a exportação HTML completa (html) e as variáveis detetadas com o respetivo número de ocorrências (variables).",

    // ---- Instalar
    installer_description: "Descarregar o Varium, a suite de escritório com variáveis (Documents, Classeur, Présentation), para Windows, macOS e Linux.",
    installer_chapo: "O Varium é uma suite de escritório gratuita para Windows, macOS e Linux: Documents, Classeur e Présentation. Sem conta, sem versão online: os seus ficheiros ficam no seu computador.",
    telecharger_pour: "Descarregar para {}",
    systemes: [
        Systeme {
            compat: "Windows 10 e 11, 64 bits",
            etapes: [
                "Descomprima o ficheiro (botão direito → Extrair tudo).",
                "Abra o varium.exe.",
                "Se o Windows mostrar “O Windows protegeu o seu PC”: “Mais informações” → “Executar mesmo assim”.",
            ],
        },
        Systeme {
            compat: "macOS 11 ou mais recente, Apple Silicon e Intel",
            etapes: [
                "Descomprima o ficheiro e arraste o Varium para a pasta Aplicações.",
                "No primeiro arranque: botão direito no Varium → Abrir → Abrir.",
                "Se o macOS continuar a recusar: Definições do Sistema → Privacidade e segurança → “Abrir mesmo assim”.",
            ],
        },
        Systeme {
            compat: "x86_64, glibc 2.35 ou mais recente (Ubuntu 22.04, Debian 12, Fedora 36…)",
            etapes: [
                "Descomprima: tar xzf varium-linux-x86_64.tar.gz",
                "Execute: ./varium/varium",
                "Para o menu de aplicações: copie varium para ~/.local/bin, varium.desktop para ~/.local/share/applications e varium.svg para ~/.local/share/icons.",
            ],
        },
    ],
    versions: "Todas as versões e as respetivas notas: {}. As aplicações ainda não estão assinadas pela Apple nem pela Microsoft, daí o aviso no primeiro arranque.",
    versions_lien: "página de versões no GitHub",
    source_titre: "A partir do código-fonte",
    source_texte: "Com o Rust (rustup.rs), o comando varium instala-se em qualquer sistema:",
    source_linux: "No Linux, o Varium usa as bibliotecas gráficas habituais (libxkbcommon, Wayland ou X11, um controlador OpenGL ou Vulkan), já presentes em qualquer ambiente de trabalho.",
    source_github: "Código-fonte no GitHub",

    // ---- Experimentar
    essayer_titre: "Experimentar o Varium",
    essayer_description: [
        "Escreva um modelo com {{variáveis}}, dê-lhes valores e veja o documento atualizar-se: a demonstração online do Varium.",
        "Um modelo com {{variáveis}}, os seus valores e o documento gerado pelo Varium. A suite completa instala-se no seu computador.",
    ],
    essayer_explication: [
        "Altere o modelo ou os valores e atualize o documento. O resultado é calculado pelo verdadeiro motor do Varium (varium-core), no servidor. O exemplo está em francês, tal como os formatos de data e de montante.",
        "A carta de exemplo do Varium, em francês: o modelo e os seus valores à esquerda, o documento calculado pelo verdadeiro motor (varium-core) à direita.",
    ],
    essayer_installer: Item {
        titre: "Para escrever os seus próprios modelos, instale o Varium.",
        texte: "A suite completa é uma aplicação para Linux, macOS e Windows: paginação, campos calculados, impressão em série, folhas de cálculo, apresentações, exportação para ODT, DOCX e PDF. Os seus documentos ficam no seu computador.",
    },
    modele: "Modelo",
    modele_aide: "escreva {{uma_variavel}} onde quiser",
    lecture_seule: "só de leitura",
    variables: "Variáveis ({})",
    aucune_variable: "Ainda não há variáveis: acrescente {{nome}} ao modelo e atualize.",
    mettre_a_jour: "Atualizar o documento",
    reinitialiser: "Repor",
    variables_colonnes: ["Variável", "Tipo", "Valor"],
    a_remplir: "por preencher",
    document: "Documento",
    compteur: [
        "{} variável",
        "{} variáveis",
        "{} ocorrência",
        "{} ocorrências",
    ],
    a_remplir_n: "{} por preencher",
    texte_brut: "Texto simples",
    sans_valeur: "“{}” ainda não tem valor",
    saut_de_page: "quebra de página",
    type_de: "Tipo de {}",
    format_date: "AAAA-MM-DD",
    erreurs: [
        "Data esperada: AAAA-MM-DD",
        "Número esperado",
        "Endereço de e-mail inválido",
    ],
    tronque: "O modelo foi cortado a {} caracteres.",

    // ---- Não encontrada
    introuvable_titre: "Página não encontrada",
    introuvable_description: "Esta página não existe.",
    introuvable_h1: "{{página}} não encontrada",
    introuvable_texte: "Esta variável não tem valor: a página pedida não existe.",
    retour_accueil: "Voltar ao início",
};
