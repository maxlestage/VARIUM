//! Aperçu Unicode d'une formule LaTeX (pour l'affichage dans l'éditeur).
//! La composition exacte reste celle de LaTeX à l'export `.tex` / KaTeX en HTML.

const SYMBOLS: &[(&str, &str)] = &[
    ("alpha", "α"),
    ("beta", "β"),
    ("gamma", "γ"),
    ("delta", "δ"),
    ("epsilon", "ε"),
    ("varepsilon", "ε"),
    ("zeta", "ζ"),
    ("eta", "η"),
    ("theta", "θ"),
    ("iota", "ι"),
    ("kappa", "κ"),
    ("lambda", "λ"),
    ("mu", "μ"),
    ("nu", "ν"),
    ("xi", "ξ"),
    ("pi", "π"),
    ("rho", "ρ"),
    ("sigma", "σ"),
    ("tau", "τ"),
    ("phi", "φ"),
    ("varphi", "φ"),
    ("chi", "χ"),
    ("psi", "ψ"),
    ("omega", "ω"),
    ("Gamma", "Γ"),
    ("Delta", "Δ"),
    ("Theta", "Θ"),
    ("Lambda", "Λ"),
    ("Xi", "Ξ"),
    ("Pi", "Π"),
    ("Sigma", "Σ"),
    ("Phi", "Φ"),
    ("Psi", "Ψ"),
    ("Omega", "Ω"),
    ("sum", "∑"),
    ("prod", "∏"),
    ("int", "∫"),
    ("oint", "∮"),
    ("infty", "∞"),
    ("partial", "∂"),
    ("nabla", "∇"),
    ("pm", "±"),
    ("mp", "∓"),
    ("times", "×"),
    ("cdot", "·"),
    ("div", "÷"),
    ("leq", "≤"),
    ("le", "≤"),
    ("geq", "≥"),
    ("ge", "≥"),
    ("neq", "≠"),
    ("ne", "≠"),
    ("approx", "≈"),
    ("equiv", "≡"),
    ("sim", "∼"),
    ("propto", "∝"),
    ("in", "∈"),
    ("notin", "∉"),
    ("subset", "⊂"),
    ("subseteq", "⊆"),
    ("cup", "∪"),
    ("cap", "∩"),
    ("emptyset", "∅"),
    ("forall", "∀"),
    ("exists", "∃"),
    ("neg", "¬"),
    ("land", "∧"),
    ("lor", "∨"),
    ("to", "→"),
    ("rightarrow", "→"),
    ("leftarrow", "←"),
    ("Rightarrow", "⇒"),
    ("Leftarrow", "⇐"),
    ("iff", "⇔"),
    ("mapsto", "↦"),
    ("ldots", "…"),
    ("cdots", "⋯"),
    ("degree", "°"),
    ("hbar", "ħ"),
    ("ell", "ℓ"),
    ("Re", "ℜ"),
    ("Im", "ℑ"),
    ("mathbb{R}", "ℝ"),
    ("mathbb{N}", "ℕ"),
    ("mathbb{Z}", "ℤ"),
    ("mathbb{Q}", "ℚ"),
    ("mathbb{C}", "ℂ"),
    (",", " "),
    (";", " "),
    ("quad", "  "),
    ("qquad", "    "),
    ("left", ""),
    ("right", ""),
    ("{", "{"),
    ("}", "}"),
];

fn sup(c: char) -> Option<char> {
    Some(match c {
        '0' => '⁰',
        '1' => '¹',
        '2' => '²',
        '3' => '³',
        '4' => '⁴',
        '5' => '⁵',
        '6' => '⁶',
        '7' => '⁷',
        '8' => '⁸',
        '9' => '⁹',
        '+' => '⁺',
        '-' => '⁻',
        '=' => '⁼',
        '(' => '⁽',
        ')' => '⁾',
        'n' => 'ⁿ',
        'i' => 'ⁱ',
        'x' => 'ˣ',
        'k' => 'ᵏ',
        'a' => 'ᵃ',
        'b' => 'ᵇ',
        'T' => 'ᵀ',
        _ => return None,
    })
}

fn sub(c: char) -> Option<char> {
    Some(match c {
        '0' => '₀',
        '1' => '₁',
        '2' => '₂',
        '3' => '₃',
        '4' => '₄',
        '5' => '₅',
        '6' => '₆',
        '7' => '₇',
        '8' => '₈',
        '9' => '₉',
        '+' => '₊',
        '-' => '₋',
        '=' => '₌',
        '(' => '₍',
        ')' => '₎',
        'i' => 'ᵢ',
        'j' => 'ⱼ',
        'k' => 'ₖ',
        'n' => 'ₙ',
        'x' => 'ₓ',
        'a' => 'ₐ',
        'e' => 'ₑ',
        'o' => 'ₒ',
        'm' => 'ₘ',
        't' => 'ₜ',
        _ => return None,
    })
}

/// Lit un argument : `{…}` (équilibré) ou un seul caractère.
fn take_arg(s: &str) -> (&str, &str) {
    let s = s.trim_start();
    if let Some(body) = s.strip_prefix('{') {
        let mut depth = 1;
        for (i, c) in body.char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return (&body[..i], &body[i + 1..]);
                    }
                }
                _ => {}
            }
        }
        (body, "")
    } else {
        let n = s.chars().next().map_or(0, char::len_utf8);
        (&s[..n], &s[n..])
    }
}

fn script(arg: &str, map: fn(char) -> Option<char>, fallback: char) -> String {
    let conv = to_unicode(arg);
    match conv.chars().map(map).collect::<Option<String>>() {
        Some(s) => s,
        None => format!("{fallback}({conv})"),
    }
}

fn wrap(s: String) -> String {
    if s.chars().count() <= 1 {
        s
    } else {
        format!("({s})")
    }
}

/// Convertit une formule LaTeX en texte Unicode approché.
pub fn to_unicode(tex: &str) -> String {
    let mut out = String::new();
    let mut rest = tex;
    while let Some(c) = rest.chars().next() {
        match c {
            '\\' => {
                let body = &rest[1..];
                let len = body
                    .char_indices()
                    .find(|(_, c)| !c.is_ascii_alphabetic())
                    .map_or(body.len(), |(i, _)| i);
                let (cmd, after) = if len == 0 {
                    let n = body.chars().next().map_or(0, char::len_utf8);
                    body.split_at(n)
                } else {
                    body.split_at(len)
                };
                rest = after;
                match cmd {
                    "frac" | "dfrac" | "tfrac" => {
                        let (a, r) = take_arg(rest);
                        let (b, r) = take_arg(r);
                        out.push_str(&format!("{}⁄{}", wrap(to_unicode(a)), wrap(to_unicode(b))));
                        rest = r;
                    }
                    "sqrt" => {
                        let (a, r) = take_arg(rest);
                        out.push_str(&format!("√{}", wrap(to_unicode(a))));
                        rest = r;
                    }
                    "text" | "mathrm" | "mathbf" | "mathit" | "operatorname" => {
                        let (a, r) = take_arg(rest);
                        out.push_str(&to_unicode(a));
                        rest = r;
                    }
                    "mathbb" => {
                        let (a, r) = take_arg(rest);
                        let key = format!("mathbb{{{a}}}");
                        match SYMBOLS.iter().find(|(k, _)| *k == key) {
                            Some((_, v)) => out.push_str(v),
                            None => out.push_str(a),
                        }
                        rest = r;
                    }
                    _ => match SYMBOLS.iter().find(|(k, _)| *k == cmd) {
                        Some((_, v)) => out.push_str(v),
                        None => out.push_str(cmd), // \sin, \log, \lim…
                    },
                }
            }
            '^' | '_' => {
                let (a, r) = take_arg(&rest[1..]);
                let s = if c == '^' {
                    script(a, sup, '^')
                } else {
                    script(a, sub, '_')
                };
                out.push_str(&s);
                rest = r;
            }
            '{' | '}' => rest = &rest[1..],
            '~' => {
                out.push(' ');
                rest = &rest[1..];
            }
            _ => {
                out.push(c);
                rest = &rest[c.len_utf8()..];
            }
        }
    }
    out
}
