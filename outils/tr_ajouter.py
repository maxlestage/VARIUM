#!/usr/bin/env python3
"""Ajoute des traductions d'interface dans les cinq tables `crates/varium/src/tr/*.rs`.

Usage : python3 outils/tr_ajouter.py fichier.json
où le JSON est {"Texte français": {"en": "...", "es": "...", "de": "...", "it": "...", "pt": "..."}, ...}.

Les tables sont relues, complétées (une clé existante est remplacée), triées dans l'ordre
des octets UTF-8 (celui de `str::cmp` en Rust) et réécrites ; un verrou évite que deux
personnes écrivent en même temps. Lancer `cargo fmt -p varium` ensuite.
"""
import fcntl, json, os, re, sys

RACINE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DOSSIER = os.path.join(RACINE, "crates", "varium", "src", "tr")
LANGUES = ["en", "es", "de", "it", "pt"]
LIT = r'"(?:[^"\\]|\\.)*"'
PAIRE = re.compile(r"\(\s*(" + LIT + r")\s*,\s*(" + LIT + r")\s*,?\s*\)", re.S)


def unescape(lit):
    s = lit[1:-1]
    out, i = [], 0
    while i < len(s):
        c = s[i]
        if c == "\\":
            n = s[i + 1]
            if n == "n": out.append("\n"); i += 2
            elif n == "t": out.append("\t"); i += 2
            elif n == "u":
                j = s.index("}", i); out.append(chr(int(s[i + 3:j], 16))); i = j + 1
            elif n == "\n":
                i += 2
                while i < len(s) and s[i] in " \t\n": i += 1
            else: out.append(n); i += 2
        else:
            out.append(c); i += 1
    return "".join(out)


def escape(s):
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"').replace("\n", "\\n").replace("\t", "\\t") + '"'


def main():
    nouvelles = json.load(open(sys.argv[1], encoding="utf-8"))
    for fr, t in nouvelles.items():
        manque = [l for l in LANGUES if not t.get(l)]
        if manque:
            sys.exit(f"« {fr} » : traduction manquante pour {manque}")
    with open(os.path.join(DOSSIER, ".verrou"), "w") as v:
        fcntl.flock(v, fcntl.LOCK_EX)
        for l in LANGUES:
            p = os.path.join(DOSSIER, l + ".rs")
            src = open(p, encoding="utf-8").read()
            debut = src.index("= &[") + 4
            fin = src.rindex("];")
            tete, corps, queue = src[:debut], src[debut:fin], src[fin:]
            table = {unescape(a): unescape(b) for a, b in PAIRE.findall(corps)}
            for fr, t in nouvelles.items():
                table[fr] = t[l]
            lignes = "".join(
                f"\n    ({escape(k)}, {escape(table[k])}),"
                for k in sorted(table, key=lambda k: k.encode("utf-8"))
            )
            open(p, "w", encoding="utf-8").write(tete + lignes + "\n" + queue)
    print(f"{len(nouvelles)} traduction(s) ajoutée(s) dans {', '.join(LANGUES)}")


if __name__ == "__main__":
    main()
