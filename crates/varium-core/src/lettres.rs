//! Les lettres types fournies avec Varium, en six langues : résiliation, réclamation,
//! relance de facture, attestation, demande de congés et lettre de motivation. Ce
//! sont des modèles intelligents (voir `modele`) : Varium pose une question par champ,
//! puis écrit la lettre.
//!
//! Le texte des lettres désigne ses champs par une clé (`{{@nom}}`) ; chaque langue
//! leur donne un nom (`{{nom}}`, `{{name}}`, `{{nombre}}`…), un type et une question.

use crate::lang::Lang;
use crate::modele::{Modele, Question};
use crate::page::PageSetup;
use crate::project::Project;
use crate::render::VarType;

/// Un champ des lettres : clé, type, puis (nom, question) dans chaque langue, dans
/// l'ordre de `Lang::ALL` (fr, en, es, de, it, pt).
type Champ = (&'static str, VarType, [(&'static str, &'static str); 6]);

const CHAMPS: &[Champ] = &[
    (
        "nom",
        VarType::Text,
        [
            ("nom", "Quel est votre nom complet ?"),
            ("name", "What is your full name?"),
            ("nombre", "¿Cuál es su nombre completo?"),
            ("name", "Wie lautet Ihr vollständiger Name?"),
            ("nome", "Qual è il suo nome completo?"),
            ("nome", "Qual é o seu nome completo?"),
        ],
    ),
    (
        "adresse",
        VarType::Long,
        [
            ("adresse", "Quelle est votre adresse postale ?"),
            ("address", "What is your postal address?"),
            ("dirección", "¿Cuál es su dirección postal?"),
            ("adresse", "Wie lautet Ihre Postanschrift?"),
            ("indirizzo", "Qual è il suo indirizzo postale?"),
            ("morada", "Qual é a sua morada?"),
        ],
    ),
    (
        "email",
        VarType::Email,
        [
            ("email", "Quelle est votre adresse e-mail ?"),
            ("email", "What is your email address?"),
            ("email", "¿Cuál es su correo electrónico?"),
            ("email", "Wie lautet Ihre E-Mail-Adresse?"),
            ("email", "Qual è il suo indirizzo e-mail?"),
            ("email", "Qual é o seu endereço de e-mail?"),
        ],
    ),
    (
        "destinataire",
        VarType::Text,
        [
            ("destinataire", "À qui écrivez-vous (société ou personne) ?"),
            ("recipient", "Who are you writing to (company or person)?"),
            ("destinatario", "¿A quién escribe (empresa o persona)?"),
            ("empfänger", "An wen schreiben Sie (Firma oder Person)?"),
            ("destinatario", "A chi scrive (azienda o persona)?"),
            ("destinatario", "A quem escreve (empresa ou pessoa)?"),
        ],
    ),
    (
        "adresse_dest",
        VarType::Long,
        [
            (
                "adresse_destinataire",
                "Quelle est l'adresse du destinataire ?",
            ),
            ("recipient_address", "What is the recipient's address?"),
            (
                "dirección_destinatario",
                "¿Cuál es la dirección del destinatario?",
            ),
            (
                "empfänger_adresse",
                "Wie lautet die Anschrift des Empfängers?",
            ),
            (
                "indirizzo_destinatario",
                "Qual è l'indirizzo del destinatario?",
            ),
            ("morada_destinatario", "Qual é a morada do destinatário?"),
        ],
    ),
    (
        "ville",
        VarType::Text,
        [
            ("ville", "De quelle ville écrivez-vous ?"),
            ("city", "Which city are you writing from?"),
            ("ciudad", "¿Desde qué ciudad escribe?"),
            ("ort", "Aus welchem Ort schreiben Sie?"),
            ("città", "Da quale città scrive?"),
            ("cidade", "De que cidade escreve?"),
        ],
    ),
    (
        "date",
        VarType::Date,
        [
            ("date", "Quelle date porte la lettre ?"),
            ("date", "What date is the letter?"),
            ("fecha", "¿Qué fecha lleva la carta?"),
            ("datum", "Welches Datum trägt der Brief?"),
            ("data", "Che data porta la lettera?"),
            ("data", "Que data tem a carta?"),
        ],
    ),
    (
        "numero",
        VarType::Text,
        [
            (
                "numéro_contrat",
                "Quel est le numéro du contrat (ou de client) ?",
            ),
            (
                "contract_number",
                "What is the contract (or customer) number?",
            ),
            (
                "número_contrato",
                "¿Cuál es el número de contrato (o de cliente)?",
            ),
            (
                "vertragsnummer",
                "Wie lautet die Vertrags- oder Kundennummer?",
            ),
            (
                "numero_contratto",
                "Qual è il numero del contratto (o cliente)?",
            ),
            (
                "número_contrato",
                "Qual é o número do contrato (ou de cliente)?",
            ),
        ],
    ),
    (
        "date_effet",
        VarType::Date,
        [
            (
                "date_effet",
                "À partir de quelle date la résiliation prend-elle effet ?",
            ),
            (
                "effective_date",
                "From which date should the cancellation take effect?",
            ),
            (
                "fecha_efecto",
                "¿A partir de qué fecha surte efecto la baja?",
            ),
            (
                "kündigungsdatum",
                "Zu welchem Datum soll die Kündigung wirksam werden?",
            ),
            ("data_effetto", "Da quale data ha effetto la disdetta?"),
            (
                "data_efeito",
                "A partir de que data produz efeito a rescisão?",
            ),
        ],
    ),
    (
        "commande",
        VarType::Text,
        [
            ("numéro_commande", "Quel est le numéro de la commande ?"),
            ("order_number", "What is the order number?"),
            ("número_pedido", "¿Cuál es el número de pedido?"),
            ("bestellnummer", "Wie lautet die Bestellnummer?"),
            ("numero_ordine", "Qual è il numero dell'ordine?"),
            ("número_encomenda", "Qual é o número da encomenda?"),
        ],
    ),
    (
        "date_commande",
        VarType::Date,
        [
            ("date_commande", "Quand avez-vous passé la commande ?"),
            ("order_date", "When did you place the order?"),
            ("fecha_pedido", "¿Cuándo hizo el pedido?"),
            ("bestelldatum", "Wann haben Sie bestellt?"),
            ("data_ordine", "Quando ha effettuato l'ordine?"),
            ("data_encomenda", "Quando fez a encomenda?"),
        ],
    ),
    (
        "probleme",
        VarType::Long,
        [
            ("problème", "Quel est le problème rencontré ?"),
            ("issue", "What went wrong?"),
            ("problema", "¿Cuál es el problema?"),
            ("problem", "Welches Problem ist aufgetreten?"),
            ("problema", "Qual è il problema riscontrato?"),
            ("problema", "Qual é o problema?"),
        ],
    ),
    (
        "demande",
        VarType::Text,
        [
            (
                "demande",
                "Que demandez-vous (remboursement, échange, réparation…) ?",
            ),
            (
                "request",
                "What do you ask for (refund, replacement, repair…)?",
            ),
            (
                "solicitud",
                "¿Qué solicita (reembolso, cambio, reparación…)?",
            ),
            (
                "anliegen",
                "Was fordern Sie (Erstattung, Umtausch, Reparatur…)?",
            ),
            (
                "richiesta",
                "Cosa chiede (rimborso, sostituzione, riparazione…)?",
            ),
            ("pedido", "O que pede (reembolso, troca, reparação…)?"),
        ],
    ),
    (
        "facture",
        VarType::Text,
        [
            ("numéro_facture", "Quel est le numéro de la facture ?"),
            ("invoice_number", "What is the invoice number?"),
            ("número_factura", "¿Cuál es el número de factura?"),
            ("rechnungsnummer", "Wie lautet die Rechnungsnummer?"),
            ("numero_fattura", "Qual è il numero della fattura?"),
            ("número_fatura", "Qual é o número da fatura?"),
        ],
    ),
    (
        "date_facture",
        VarType::Date,
        [
            ("date_facture", "De quand date la facture ?"),
            ("invoice_date", "What is the invoice date?"),
            ("fecha_factura", "¿Qué fecha tiene la factura?"),
            ("rechnungsdatum", "Von wann ist die Rechnung?"),
            ("data_fattura", "Di quando è la fattura?"),
            ("data_fatura", "De quando é a fatura?"),
        ],
    ),
    (
        "montant",
        VarType::Money,
        [
            ("montant", "Quel est le montant dû ?"),
            ("amount", "What amount is due?"),
            ("importe", "¿Cuál es el importe adeudado?"),
            ("betrag", "Welcher Betrag ist offen?"),
            ("importo", "Qual è l'importo dovuto?"),
            ("montante", "Qual é o montante em dívida?"),
        ],
    ),
    (
        "echeance",
        VarType::Date,
        [
            (
                "échéance",
                "Avant quelle date le paiement doit-il arriver ?",
            ),
            ("due_date", "By which date must the payment arrive?"),
            ("vencimiento", "¿Antes de qué fecha debe llegar el pago?"),
            ("frist", "Bis wann muss die Zahlung eingehen?"),
            ("scadenza", "Entro quale data deve arrivare il pagamento?"),
            ("prazo", "Até que data deve chegar o pagamento?"),
        ],
    ),
    (
        "entreprise",
        VarType::Text,
        [
            ("entreprise", "Quel est le nom de l'entreprise ?"),
            ("company", "What is the company's name?"),
            ("empresa", "¿Cuál es el nombre de la empresa?"),
            ("firma", "Wie heißt das Unternehmen?"),
            ("azienda", "Qual è il nome dell'azienda?"),
            ("empresa", "Qual é o nome da empresa?"),
        ],
    ),
    (
        "signataire",
        VarType::Text,
        [
            ("signataire", "Qui signe l'attestation ?"),
            ("signatory", "Who signs the certificate?"),
            ("firmante", "¿Quién firma el certificado?"),
            ("unterzeichner", "Wer unterschreibt die Bescheinigung?"),
            ("firmatario", "Chi firma l'attestazione?"),
            ("signatário", "Quem assina a declaração?"),
        ],
    ),
    (
        "fonction",
        VarType::Text,
        [
            ("fonction", "Quelle est la fonction du signataire ?"),
            ("role", "What is the signatory's role?"),
            ("cargo", "¿Qué cargo tiene el firmante?"),
            ("funktion", "Welche Funktion hat der Unterzeichner?"),
            ("ruolo", "Qual è il ruolo del firmatario?"),
            ("função", "Qual é a função do signatário?"),
        ],
    ),
    (
        "salarie",
        VarType::Text,
        [
            ("salarié", "Qui est la personne concernée ?"),
            ("employee", "Who is the employee concerned?"),
            ("empleado", "¿Quién es la persona interesada?"),
            ("mitarbeiter", "Um welche Person geht es?"),
            ("dipendente", "Chi è la persona interessata?"),
            ("colaborador", "Quem é a pessoa em causa?"),
        ],
    ),
    (
        "poste",
        VarType::Text,
        [
            ("poste", "Quel est le poste ?"),
            ("job_title", "What is the job title?"),
            ("puesto", "¿Cuál es el puesto?"),
            ("stelle", "Um welche Stelle geht es?"),
            ("mansione", "Qual è la mansione?"),
            ("cargo", "Qual é o cargo?"),
        ],
    ),
    (
        "date_debut",
        VarType::Date,
        [
            ("date_début", "À partir de quelle date ?"),
            ("start_date", "From which date?"),
            ("fecha_inicio", "¿Desde qué fecha?"),
            ("beginn", "Ab welchem Datum?"),
            ("data_inizio", "Da quale data?"),
            ("data_início", "A partir de que data?"),
        ],
    ),
    (
        "date_fin",
        VarType::Date,
        [
            ("date_fin", "Jusqu'à quelle date (incluse) ?"),
            ("end_date", "Until which date (inclusive)?"),
            ("fecha_fin", "¿Hasta qué fecha (incluida)?"),
            ("ende", "Bis zu welchem Datum (einschließlich)?"),
            ("data_fine", "Fino a quale data (inclusa)?"),
            ("data_fim", "Até que data (inclusive)?"),
        ],
    ),
    (
        "jours",
        VarType::Number,
        [
            (
                "nombre_jours",
                "Combien de jours ouvrés cela représente-t-il ?",
            ),
            ("days", "How many working days is that?"),
            ("días", "¿Cuántos días laborables son?"),
            ("tage", "Wie viele Arbeitstage sind das?"),
            ("giorni", "Quanti giorni lavorativi sono?"),
            ("dias", "Quantos dias úteis são?"),
        ],
    ),
    (
        "responsable",
        VarType::Text,
        [
            ("responsable", "Quel est le nom de votre responsable ?"),
            ("manager", "What is your manager's name?"),
            ("responsable", "¿Cómo se llama su responsable?"),
            ("vorgesetzter", "Wie heißt Ihre vorgesetzte Person?"),
            ("responsabile", "Come si chiama il suo responsabile?"),
            ("responsável", "Como se chama o seu responsável?"),
        ],
    ),
    (
        "motivation",
        VarType::Long,
        [
            (
                "motivation",
                "En quelques phrases : pourquoi ce poste, et qu'apportez-vous ?",
            ),
            (
                "motivation",
                "In a few sentences: why this job, and what do you bring?",
            ),
            (
                "motivación",
                "En pocas frases: ¿por qué este puesto y qué aporta?",
            ),
            (
                "motivation",
                "In wenigen Sätzen: Warum diese Stelle, und was bringen Sie mit?",
            ),
            (
                "motivazione",
                "In poche frasi: perché questo ruolo e cosa può offrire?",
            ),
            (
                "motivação",
                "Em poucas frases: porquê este cargo e o que traz?",
            ),
        ],
    ),
];

/// Une lettre : icône, puis (nom, description, texte) dans chaque langue.
type Lettre = (
    &'static str,
    [(&'static str, &'static str, &'static str); 6],
);

const RESILIATION: Lettre = ("✂", [
    ("Résiliation", "Mettre fin à un contrat ou un abonnement", "**{{@nom}}**
{{@adresse}}

{{@destinataire}}
{{@adresse_dest}}

-> {{@ville}}, le {{@date | date}} ->

**Objet : résiliation du contrat n° {{@numero}}**
Lettre recommandée avec accusé de réception

Madame, Monsieur,

Par la présente, je vous informe de ma décision de résilier le contrat n° {{@numero}}, à compter du {{@date_effet | date}}.

Je vous remercie de bien vouloir m'adresser une confirmation écrite de cette résiliation ainsi que, le cas échéant, le solde de tout compte.

Je vous prie d'agréer, Madame, Monsieur, l'expression de mes salutations distinguées.

-> {{@nom}} ->"),
    ("Cancellation", "End a contract or a subscription", "**{{@nom}}**
{{@adresse}}

{{@destinataire}}
{{@adresse_dest}}

-> {{@ville}}, {{@date | date}} ->

**Subject: cancellation of contract no. {{@numero}}**

Dear Sir or Madam,

I hereby give notice that I wish to cancel contract no. {{@numero}}, effective {{@date_effet | date}}.

Please send me written confirmation of this cancellation and, where applicable, the final statement of my account.

Yours faithfully,

-> {{@nom}} ->"),
    ("Baja de contrato", "Poner fin a un contrato o una suscripción", "**{{@nom}}**
{{@adresse}}

{{@destinataire}}
{{@adresse_dest}}

-> {{@ville}}, {{@date | date}} ->

**Asunto: baja del contrato n.º {{@numero}}**

Estimados señores:

Por medio de la presente les comunico mi decisión de dar de baja el contrato n.º {{@numero}} con efecto a partir del {{@date_effet | date}}.

Les ruego que me envíen una confirmación por escrito de esta baja y, en su caso, la liquidación final.

Atentamente,

-> {{@nom}} ->"),
    ("Kündigung", "Einen Vertrag oder ein Abonnement beenden", "**{{@nom}}**
{{@adresse}}

{{@destinataire}}
{{@adresse_dest}}

-> {{@ville}}, {{@date | date}} ->

**Kündigung des Vertrags Nr. {{@numero}}**

Sehr geehrte Damen und Herren,

hiermit kündige ich den Vertrag Nr. {{@numero}} fristgerecht zum {{@date_effet | date}}.

Bitte senden Sie mir eine schriftliche Bestätigung der Kündigung und gegebenenfalls die Schlussabrechnung.

Mit freundlichen Grüßen

-> {{@nom}} ->"),
    ("Disdetta", "Chiudere un contratto o un abbonamento", "**{{@nom}}**
{{@adresse}}

{{@destinataire}}
{{@adresse_dest}}

-> {{@ville}}, {{@date | date}} ->

**Oggetto: disdetta del contratto n. {{@numero}}**

Gentili Signori,

con la presente comunico la mia decisione di disdire il contratto n. {{@numero}} con decorrenza dal {{@date_effet | date}}.

Vi prego di inviarmi una conferma scritta della disdetta e, se previsto, il saldo finale.

Distinti saluti,

-> {{@nom}} ->"),
    ("Rescisão", "Pôr fim a um contrato ou a uma assinatura", "**{{@nom}}**
{{@adresse}}

{{@destinataire}}
{{@adresse_dest}}

-> {{@ville}}, {{@date | date}} ->

**Assunto: rescisão do contrato n.º {{@numero}}**

Exmos. Senhores,

Venho por este meio comunicar a minha decisão de rescindir o contrato n.º {{@numero}}, com efeitos a partir de {{@date_effet | date}}.

Agradeço o envio de uma confirmação escrita desta rescisão e, se for o caso, do acerto final de contas.

Com os melhores cumprimentos,

-> {{@nom}} ->"),
]);

const RECLAMATION: Lettre = ("⚠", [
    ("Réclamation", "Signaler un produit ou un service défectueux", "**{{@nom}}**
{{@adresse}}

{{@destinataire}}
{{@adresse_dest}}

-> {{@ville}}, le {{@date | date}} ->

**Objet : réclamation concernant la commande n° {{@commande}}**

Madame, Monsieur,

Le {{@date_commande | date}}, j'ai passé la commande n° {{@commande}}. Je rencontre malheureusement le problème suivant :

{{@probleme}}

Je vous demande donc : {{@demande}}, dans un délai de quinze jours à compter de la réception de ce courrier.

Dans l'attente de votre réponse, je vous prie d'agréer, Madame, Monsieur, mes salutations distinguées.

-> {{@nom}} ->"),
    ("Complaint", "Report a faulty product or service", "**{{@nom}}**
{{@adresse}}

{{@destinataire}}
{{@adresse_dest}}

-> {{@ville}}, {{@date | date}} ->

**Subject: complaint about order no. {{@commande}}**

Dear Sir or Madam,

On {{@date_commande | date}} I placed order no. {{@commande}}. Unfortunately I have encountered the following problem:

{{@probleme}}

I therefore ask for the following: {{@demande}}, within fifteen days of receiving this letter.

I look forward to your reply.

Yours faithfully,

-> {{@nom}} ->"),
    ("Reclamación", "Señalar un producto o servicio defectuoso", "**{{@nom}}**
{{@adresse}}

{{@destinataire}}
{{@adresse_dest}}

-> {{@ville}}, {{@date | date}} ->

**Asunto: reclamación sobre el pedido n.º {{@commande}}**

Estimados señores:

El {{@date_commande | date}} realicé el pedido n.º {{@commande}}. Lamentablemente, me encuentro con el siguiente problema:

{{@probleme}}

Por ello les solicito: {{@demande}}, en un plazo de quince días a partir de la recepción de esta carta.

A la espera de su respuesta, les saluda atentamente,

-> {{@nom}} ->"),
    ("Reklamation", "Ein mangelhaftes Produkt oder eine Leistung beanstanden", "**{{@nom}}**
{{@adresse}}

{{@destinataire}}
{{@adresse_dest}}

-> {{@ville}}, {{@date | date}} ->

**Reklamation zur Bestellung Nr. {{@commande}}**

Sehr geehrte Damen und Herren,

am {{@date_commande | date}} habe ich die Bestellung Nr. {{@commande}} aufgegeben. Leider ist folgendes Problem aufgetreten:

{{@probleme}}

Ich bitte Sie daher um Folgendes: {{@demande}}, innerhalb von vierzehn Tagen nach Erhalt dieses Schreibens.

Mit freundlichen Grüßen

-> {{@nom}} ->"),
    ("Reclamo", "Segnalare un prodotto o un servizio difettoso", "**{{@nom}}**
{{@adresse}}

{{@destinataire}}
{{@adresse_dest}}

-> {{@ville}}, {{@date | date}} ->

**Oggetto: reclamo relativo all'ordine n. {{@commande}}**

Gentili Signori,

il {{@date_commande | date}} ho effettuato l'ordine n. {{@commande}}. Purtroppo riscontro il seguente problema:

{{@probleme}}

Vi chiedo pertanto: {{@demande}}, entro quindici giorni dal ricevimento della presente.

In attesa di un vostro riscontro, porgo distinti saluti,

-> {{@nom}} ->"),
    ("Reclamação", "Assinalar um produto ou serviço com defeito", "**{{@nom}}**
{{@adresse}}

{{@destinataire}}
{{@adresse_dest}}

-> {{@ville}}, {{@date | date}} ->

**Assunto: reclamação relativa à encomenda n.º {{@commande}}**

Exmos. Senhores,

Em {{@date_commande | date}} fiz a encomenda n.º {{@commande}}. Infelizmente, deparo-me com o seguinte problema:

{{@probleme}}

Venho por isso pedir: {{@demande}}, no prazo de quinze dias a contar da receção desta carta.

Aguardando a vossa resposta, apresento os melhores cumprimentos,

-> {{@nom}} ->"),
]);

const RELANCE: Lettre = ("🔔", [
    ("Relance de facture", "Rappeler poliment une facture impayée", "**{{@entreprise}}**
{{@adresse}}

{{@destinataire}}
{{@adresse_dest}}

-> {{@ville}}, le {{@date | date}} ->

**Objet : facture n° {{@facture}} en attente de règlement**

Madame, Monsieur,

Sauf erreur de notre part, la facture n° {{@facture}} du {{@date_facture | date}}, d'un montant de {{@montant | money}}, reste à ce jour impayée.

Il s'agit peut-être d'un simple oubli : nous vous remercions de bien vouloir procéder à son règlement avant le {{@echeance | date}}. Si votre paiement a été effectué entre-temps, merci de ne pas tenir compte de ce courrier.

Nous vous prions d'agréer, Madame, Monsieur, nos salutations distinguées.

-> {{@nom}} ->
-> {{@entreprise}} ->"),
    ("Invoice reminder", "Politely chase an unpaid invoice", "**{{@entreprise}}**
{{@adresse}}

{{@destinataire}}
{{@adresse_dest}}

-> {{@ville}}, {{@date | date}} ->

**Subject: invoice no. {{@facture}} awaiting payment**

Dear Sir or Madam,

According to our records, invoice no. {{@facture}} dated {{@date_facture | date}}, for {{@montant | money}}, remains unpaid.

This may simply be an oversight: we would be grateful if you could settle it by {{@echeance | date}}. If you have already paid, please disregard this letter.

Yours faithfully,

-> {{@nom}} ->
-> {{@entreprise}} ->"),
    ("Recordatorio de factura", "Recordar con cortesía una factura impagada", "**{{@entreprise}}**
{{@adresse}}

{{@destinataire}}
{{@adresse_dest}}

-> {{@ville}}, {{@date | date}} ->

**Asunto: factura n.º {{@facture}} pendiente de pago**

Estimados señores:

Salvo error por nuestra parte, la factura n.º {{@facture}} del {{@date_facture | date}}, por un importe de {{@montant | money}}, sigue pendiente de pago.

Quizás se trate de un simple olvido: les agradeceríamos que procedieran a su pago antes del {{@echeance | date}}. Si ya han realizado el pago, no tengan en cuenta esta carta.

Atentamente,

-> {{@nom}} ->
-> {{@entreprise}} ->"),
    ("Zahlungserinnerung", "Freundlich an eine offene Rechnung erinnern", "**{{@entreprise}}**
{{@adresse}}

{{@destinataire}}
{{@adresse_dest}}

-> {{@ville}}, {{@date | date}} ->

**Zahlungserinnerung zur Rechnung Nr. {{@facture}}**

Sehr geehrte Damen und Herren,

sicher haben Sie es nur übersehen: Die Rechnung Nr. {{@facture}} vom {{@date_facture | date}} über {{@montant | money}} ist noch offen.

Bitte überweisen Sie den Betrag bis zum {{@echeance | date}}. Sollten Sie inzwischen bezahlt haben, betrachten Sie dieses Schreiben bitte als gegenstandslos.

Mit freundlichen Grüßen

-> {{@nom}} ->
-> {{@entreprise}} ->"),
    ("Sollecito di pagamento", "Ricordare con garbo una fattura non pagata", "**{{@entreprise}}**
{{@adresse}}

{{@destinataire}}
{{@adresse_dest}}

-> {{@ville}}, {{@date | date}} ->

**Oggetto: fattura n. {{@facture}} in attesa di pagamento**

Gentili Signori,

salvo errori da parte nostra, la fattura n. {{@facture}} del {{@date_facture | date}}, per un importo di {{@montant | money}}, risulta ancora non pagata.

Potrebbe trattarsi di una semplice dimenticanza: vi preghiamo di provvedere al pagamento entro il {{@echeance | date}}. Se il pagamento è già stato effettuato, non tenete conto della presente.

Distinti saluti,

-> {{@nom}} ->
-> {{@entreprise}} ->"),
    ("Lembrete de fatura", "Lembrar com cortesia uma fatura por pagar", "**{{@entreprise}}**
{{@adresse}}

{{@destinataire}}
{{@adresse_dest}}

-> {{@ville}}, {{@date | date}} ->

**Assunto: fatura n.º {{@facture}} por liquidar**

Exmos. Senhores,

Salvo erro da nossa parte, a fatura n.º {{@facture}} de {{@date_facture | date}}, no montante de {{@montant | money}}, continua por pagar.

Talvez se trate de um simples esquecimento: agradecemos que procedam ao pagamento até {{@echeance | date}}. Caso o pagamento já tenha sido efetuado, queiram ignorar esta carta.

Com os melhores cumprimentos,

-> {{@nom}} ->
-> {{@entreprise}} ->"),
]);

const ATTESTATION: Lettre = ("🎓", [
    ("Attestation d'emploi", "Certifier qu'une personne travaille dans l'entreprise", "**{{@entreprise}}**
{{@adresse}}

-> **Attestation d'emploi** <-

Je soussigné(e), {{@signataire}}, {{@fonction}} de la société {{@entreprise}}, atteste que {{@salarie}} est employé(e) au sein de notre entreprise depuis le {{@date_debut | date}}, en qualité de {{@poste}}.

Cette attestation est délivrée à la demande de l'intéressé(e), pour servir et valoir ce que de droit.

-> Fait à {{@ville}}, le {{@date | date}} ->

-> {{@signataire}} ->
-> {{@fonction}} ->"),
    ("Employment certificate", "Certify that someone works for the company", "**{{@entreprise}}**
{{@adresse}}

-> **Employment certificate** <-

I, the undersigned, {{@signataire}}, {{@fonction}} of {{@entreprise}}, certify that {{@salarie}} has been employed by our company since {{@date_debut | date}} as {{@poste}}.

This certificate is issued at the request of the person concerned, for whatever purpose it may serve.

-> {{@ville}}, {{@date | date}} ->

-> {{@signataire}} ->
-> {{@fonction}} ->"),
    ("Certificado de empleo", "Certificar que una persona trabaja en la empresa", "**{{@entreprise}}**
{{@adresse}}

-> **Certificado de empleo** <-

D./D.ª {{@signataire}}, {{@fonction}} de {{@entreprise}}, certifica que {{@salarie}} trabaja en nuestra empresa desde el {{@date_debut | date}}, con el puesto de {{@poste}}.

Se expide el presente certificado a petición de la persona interesada, a los efectos oportunos.

-> {{@ville}}, {{@date | date}} ->

-> {{@signataire}} ->
-> {{@fonction}} ->"),
    ("Arbeitsbescheinigung", "Bestätigen, dass eine Person im Unternehmen arbeitet", "**{{@entreprise}}**
{{@adresse}}

-> **Arbeitsbescheinigung** <-

Hiermit bestätige ich, {{@signataire}}, {{@fonction}} der {{@entreprise}}, dass {{@salarie}} seit dem {{@date_debut | date}} als {{@poste}} in unserem Unternehmen beschäftigt ist.

Diese Bescheinigung wird auf Wunsch der betreffenden Person ausgestellt.

-> {{@ville}}, {{@date | date}} ->

-> {{@signataire}} ->
-> {{@fonction}} ->"),
    ("Attestazione di servizio", "Attestare che una persona lavora in azienda", "**{{@entreprise}}**
{{@adresse}}

-> **Attestazione di servizio** <-

Il/La sottoscritto/a {{@signataire}}, {{@fonction}} di {{@entreprise}}, attesta che {{@salarie}} lavora presso la nostra azienda dal {{@date_debut | date}} con la mansione di {{@poste}}.

La presente attestazione viene rilasciata su richiesta dell'interessato/a, per gli usi consentiti dalla legge.

-> {{@ville}}, {{@date | date}} ->

-> {{@signataire}} ->
-> {{@fonction}} ->"),
    ("Declaração de emprego", "Declarar que uma pessoa trabalha na empresa", "**{{@entreprise}}**
{{@adresse}}

-> **Declaração de emprego** <-

Eu, abaixo assinado(a), {{@signataire}}, {{@fonction}} da empresa {{@entreprise}}, declaro que {{@salarie}} trabalha na nossa empresa desde {{@date_debut | date}}, com o cargo de {{@poste}}.

A presente declaração é emitida a pedido do(a) interessado(a), para os devidos efeitos.

-> {{@ville}}, {{@date | date}} ->

-> {{@signataire}} ->
-> {{@fonction}} ->"),
]);

const CONGES: Lettre = ("🌴", [
    ("Demande de congés", "Demander des jours de congé à son responsable", "**{{@nom}}**

-> {{@ville}}, le {{@date | date}} ->

À l'attention de {{@responsable}}

**Objet : demande de congés payés**

Bonjour {{@responsable}},

Je souhaiterais prendre des congés payés du {{@date_debut | date}} au {{@date_fin | date}} inclus, soit {{@jours}} jours ouvrés.

J'organiserai d'ici là la passation de mes dossiers en cours afin que mon absence ne perturbe pas le service.

Je vous remercie par avance de votre réponse.

-> {{@nom}} ->"),
    ("Leave request", "Ask your manager for days off", "**{{@nom}}**

-> {{@ville}}, {{@date | date}} ->

For the attention of {{@responsable}}

**Subject: annual leave request**

Dear {{@responsable}},

I would like to take annual leave from {{@date_debut | date}} to {{@date_fin | date}} inclusive, which is {{@jours}} working days.

I will make sure my ongoing work is handed over beforehand so that my absence does not disrupt the team.

Thank you in advance for your reply.

-> {{@nom}} ->"),
    ("Solicitud de vacaciones", "Pedir días de vacaciones a su responsable", "**{{@nom}}**

-> {{@ville}}, {{@date | date}} ->

A la atención de {{@responsable}}

**Asunto: solicitud de vacaciones**

Hola, {{@responsable}}:

Me gustaría disfrutar de vacaciones del {{@date_debut | date}} al {{@date_fin | date}}, ambos incluidos, es decir, {{@jours}} días laborables.

Antes de irme dejaré organizado el traspaso de mis tareas para que mi ausencia no afecte al equipo.

Gracias de antemano por su respuesta.

-> {{@nom}} ->"),
    ("Urlaubsantrag", "Bei der vorgesetzten Person Urlaub beantragen", "**{{@nom}}**

-> {{@ville}}, {{@date | date}} ->

z. Hd. {{@responsable}}

**Urlaubsantrag**

Guten Tag {{@responsable}},

hiermit beantrage ich Urlaub vom {{@date_debut | date}} bis einschließlich {{@date_fin | date}}, also {{@jours}} Arbeitstage.

Meine laufenden Aufgaben übergebe ich vorher, damit das Team gut ohne mich auskommt.

Vielen Dank im Voraus für Ihre Rückmeldung.

-> {{@nom}} ->"),
    ("Richiesta di ferie", "Chiedere giorni di ferie al proprio responsabile", "**{{@nom}}**

-> {{@ville}}, {{@date | date}} ->

Alla cortese attenzione di {{@responsable}}

**Oggetto: richiesta di ferie**

Gentile {{@responsable}},

vorrei usufruire di un periodo di ferie dal {{@date_debut | date}} al {{@date_fin | date}} compresi, per un totale di {{@jours}} giorni lavorativi.

Prima della partenza organizzerò il passaggio delle attività in corso, così che la mia assenza non crei disagi.

La ringrazio in anticipo per la risposta.

-> {{@nom}} ->"),
    ("Pedido de férias", "Pedir dias de férias ao seu responsável", "**{{@nom}}**

-> {{@ville}}, {{@date | date}} ->

À atenção de {{@responsable}}

**Assunto: pedido de férias**

Olá, {{@responsable}},

Gostaria de gozar férias de {{@date_debut | date}} a {{@date_fin | date}}, inclusive, ou seja, {{@jours}} dias úteis.

Antes disso, organizarei a passagem do trabalho em curso para que a minha ausência não prejudique a equipa.

Agradeço desde já a sua resposta.

-> {{@nom}} ->"),
]);

const MOTIVATION: Lettre = ("💼", [
    ("Lettre de motivation", "Présenter sa candidature à un poste", "**{{@nom}}**
{{@adresse}}
{{@email}}

{{@entreprise}}
{{@adresse_dest}}

-> {{@ville}}, le {{@date | date}} ->

**Objet : candidature au poste de {{@poste}}**

Madame, Monsieur,

C'est avec un vif intérêt que je vous adresse ma candidature au poste de {{@poste}} au sein de {{@entreprise}}.

{{@motivation}}

Je serais heureux(se) de vous exposer plus en détail mes motivations lors d'un entretien. Je vous prie d'agréer, Madame, Monsieur, l'expression de mes salutations distinguées.

-> {{@nom}} ->"),
    ("Cover letter", "Apply for a job", "**{{@nom}}**
{{@adresse}}
{{@email}}

{{@entreprise}}
{{@adresse_dest}}

-> {{@ville}}, {{@date | date}} ->

**Application for the position of {{@poste}}**

Dear Hiring Manager,

I am delighted to apply for the position of {{@poste}} at {{@entreprise}}.

{{@motivation}}

I would welcome the opportunity to discuss my application with you in an interview. Thank you for your time and consideration.

Yours sincerely,

-> {{@nom}} ->"),
    ("Carta de presentación", "Presentar su candidatura a un puesto", "**{{@nom}}**
{{@adresse}}
{{@email}}

{{@entreprise}}
{{@adresse_dest}}

-> {{@ville}}, {{@date | date}} ->

**Asunto: candidatura al puesto de {{@poste}}**

Estimados señores:

Con gran interés les presento mi candidatura al puesto de {{@poste}} en {{@entreprise}}.

{{@motivation}}

Me encantaría exponerles con más detalle mis motivaciones en una entrevista. Agradeciendo de antemano su atención, les saluda atentamente,

-> {{@nom}} ->"),
    ("Bewerbungsschreiben", "Sich auf eine Stelle bewerben", "**{{@nom}}**
{{@adresse}}
{{@email}}

{{@entreprise}}
{{@adresse_dest}}

-> {{@ville}}, {{@date | date}} ->

**Bewerbung als {{@poste}}**

Sehr geehrte Damen und Herren,

mit großem Interesse bewerbe ich mich bei der {{@entreprise}} um die Stelle als {{@poste}}.

{{@motivation}}

Über die Einladung zu einem persönlichen Gespräch freue ich mich sehr.

Mit freundlichen Grüßen

-> {{@nom}} ->"),
    ("Lettera di presentazione", "Candidarsi per un posto di lavoro", "**{{@nom}}**
{{@adresse}}
{{@email}}

{{@entreprise}}
{{@adresse_dest}}

-> {{@ville}}, {{@date | date}} ->

**Oggetto: candidatura per la posizione di {{@poste}}**

Gentili Signori,

con vivo interesse vi invio la mia candidatura per la posizione di {{@poste}} presso {{@entreprise}}.

{{@motivation}}

Sarei lieto/a di illustrarvi meglio le mie motivazioni in un colloquio. Ringraziando per l'attenzione, porgo distinti saluti,

-> {{@nom}} ->"),
    ("Carta de motivação", "Apresentar a candidatura a um cargo", "**{{@nom}}**
{{@adresse}}
{{@email}}

{{@entreprise}}
{{@adresse_dest}}

-> {{@ville}}, {{@date | date}} ->

**Assunto: candidatura ao cargo de {{@poste}}**

Exmos. Senhores,

É com grande interesse que apresento a minha candidatura ao cargo de {{@poste}} na {{@entreprise}}.

{{@motivation}}

Teria todo o gosto em expor com mais pormenor as minhas motivações numa entrevista. Com os melhores cumprimentos,

-> {{@nom}} ->"),
]);

const LETTRES: [Lettre; 6] = [
    RESILIATION,
    RECLAMATION,
    RELANCE,
    ATTESTATION,
    CONGES,
    MOTIVATION,
];

/// Nombre de lettres types (dans chaque langue).
pub const NOMBRE: usize = LETTRES.len();

fn indice(lang: Lang) -> usize {
    Lang::ALL.iter().position(|l| *l == lang).unwrap_or(0)
}

/// Remplace chaque `{{@clé` par `{{nom` dans la langue `i` ; renvoie les champs vus,
/// dans l'ordre.
fn localiser(texte: &str, i: usize) -> (String, Vec<&'static Champ>) {
    let mut out = String::with_capacity(texte.len());
    let mut vus: Vec<&'static Champ> = Vec::new();
    let mut reste = texte;
    while let Some(k) = reste.find("{{@") {
        out.push_str(&reste[..k + 2]);
        let apres = &reste[k + 3..];
        let fin = apres
            .find(|c: char| !(c.is_alphanumeric() || c == '_'))
            .unwrap_or(apres.len());
        let cle = &apres[..fin];
        match CHAMPS.iter().find(|c| c.0 == cle) {
            Some(c) => {
                out.push_str(c.2[i].0);
                if !vus.iter().any(|v| v.0 == c.0) {
                    vus.push(c);
                }
            }
            None => out.push_str(cle),
        }
        reste = &apres[fin..];
    }
    out.push_str(reste);
    (out, vus)
}

/// La lettre type `n` (0 à `NOMBRE - 1`) dans la langue `lang`.
pub fn lettre(n: usize, lang: Lang) -> Option<Modele> {
    let (icone, versions) = LETTRES.get(n)?;
    let i = indice(lang);
    let (nom, description, texte) = versions[i];
    let (source, champs) = localiser(texte, i);
    let mut p = Project::new(nom, source, &Default::default());
    p.page = PageSetup {
        langue: lang,
        ..PageSetup::default()
    };
    for c in &champs {
        if c.1 != VarType::Text {
            p.types.insert(c.2[i].0.to_string(), c.1);
        }
    }
    Some(Modele {
        format: crate::modele::FORMAT.into(),
        version: crate::modele::VERSION,
        nom: nom.into(),
        description: description.into(),
        icone: (*icone).into(),
        langue: lang,
        projet: p,
        questions: champs
            .iter()
            .map(|c| Question {
                champ: c.2[i].0.to_string(),
                question: c.2[i].1.to_string(),
                aide: String::new(),
            })
            .collect(),
    })
}

/// Les lettres types d'une langue.
pub fn lettres(lang: Lang) -> Vec<Modele> {
    (0..NOMBRE).filter_map(|n| lettre(n, lang)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn six_lettres_en_six_langues() {
        for l in Lang::ALL {
            let ls = lettres(l);
            assert_eq!(ls.len(), 6);
            for m in &ls {
                assert!(!m.projet.source.contains("{{@"), "{}", m.nom);
                assert!(m.projet.values.is_empty());
                assert_eq!(m.projet.page.langue, l);
                // Chaque champ du texte a sa question, et rien de plus.
                let champs: Vec<String> = crate::parse::variables(&m.projet.source)
                    .into_iter()
                    .map(|v| v.name)
                    .collect();
                let qs: Vec<&str> = m.questions.iter().map(|q| q.champ.as_str()).collect();
                assert_eq!(champs.len(), qs.len(), "{} ({l:?})", m.nom);
                for c in &champs {
                    assert!(qs.contains(&c.as_str()), "{c} sans question ({})", m.nom);
                    assert!(crate::parse::is_valid_name(c), "{c}");
                }
                assert_eq!(m.questions_saisies().len(), qs.len());
                // Remplie, la lettre s'exporte sans champ manquant.
                let mut r = std::collections::BTreeMap::new();
                for q in &m.questions {
                    let v = match m.projet.types.get(&q.champ) {
                        Some(VarType::Date) => "2026-11-04",
                        Some(VarType::Money | VarType::Number) => "120",
                        Some(VarType::Email) => "a@b.fr",
                        _ => "Texte",
                    };
                    r.insert(q.champ.clone(), v.to_string());
                }
                let p = m.remplir(&r);
                let rendu = crate::export::prepare(&p.source, &p.values(), &p.page);
                let t = crate::export::text(&[rendu]);
                assert!(!t.contains("{{"), "{} ({l:?}) :\n{t}", m.nom);
            }
        }
        assert!(lettre(NOMBRE, Lang::Fr).is_none());
    }
}
