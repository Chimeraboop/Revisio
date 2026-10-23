//! Petite calculatrice : évalue une expression comme `(-b+sqrt(b^2-4*a*c))/(2*a)`.
//!
//! Sert à VÉRIFIER les résultats des exercices de maths : l'IA donne la formule de calcul,
//! Révisio fait le calcul lui-même (hors ligne) au lieu de croire la valeur annoncée.
//! Analogie Python : c'est un mini `eval`, mais sûr (seuls nombres, + - * / ^ et quelques fonctions).

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
enum Jeton {
    Nombre(f64),
    Nom(String),
    Op(char),
}

fn decouper(texte: &str) -> Result<Vec<Jeton>, String> {
    let car: Vec<char> = texte.chars().collect();
    let mut jetons = Vec::new();
    let mut i = 0;
    while i < car.len() {
        let c = car[i];
        if c.is_whitespace() {
            i += 1;
        } else if c.is_ascii_digit() || (c == '.' && car.get(i + 1).is_some_and(|d| d.is_ascii_digit())) {
            let debut = i;
            while i < car.len() && (car[i].is_ascii_digit() || car[i] == '.') {
                i += 1;
            }
            // Notation scientifique : 1.5e3, 2E-4
            if i < car.len() && (car[i] == 'e' || car[i] == 'E') {
                let suite = if matches!(car.get(i + 1), Some('+' | '-')) { i + 2 } else { i + 1 };
                if car.get(suite).is_some_and(|d| d.is_ascii_digit()) {
                    i = suite;
                    while i < car.len() && car[i].is_ascii_digit() {
                        i += 1;
                    }
                }
            }
            let t: String = car[debut..i].iter().collect();
            jetons.push(Jeton::Nombre(t.parse().map_err(|_| format!("nombre illisible : {t}"))?));
        } else if c.is_alphabetic() || c == '_' {
            let debut = i;
            while i < car.len() && (car[i].is_alphanumeric() || car[i] == '_') {
                i += 1;
            }
            jetons.push(Jeton::Nom(car[debut..i].iter().collect()));
        } else if "+-*/^()".contains(c) {
            jetons.push(Jeton::Op(c));
            i += 1;
        } else {
            return Err(format!("caractère inattendu : {c}"));
        }
    }
    Ok(jetons)
}

/// Analyse « descente récursive » : une fonction par niveau de priorité
/// (somme < produit < puissance < signe < valeur).
struct Analyseur<'a> {
    jetons: Vec<Jeton>,
    pos: usize,
    variables: &'a HashMap<String, f64>,
}

impl Analyseur<'_> {
    fn suivant(&self) -> Option<&Jeton> {
        self.jetons.get(self.pos)
    }

    fn est_op(&self, op: char) -> bool {
        self.suivant() == Some(&Jeton::Op(op))
    }

    fn somme(&mut self) -> Result<f64, String> {
        let mut v = self.produit()?;
        loop {
            if self.est_op('+') {
                self.pos += 1;
                v += self.produit()?;
            } else if self.est_op('-') {
                self.pos += 1;
                v -= self.produit()?;
            } else {
                return Ok(v);
            }
        }
    }

    fn produit(&mut self) -> Result<f64, String> {
        let mut v = self.signe()?;
        loop {
            if self.est_op('*') {
                self.pos += 1;
                v *= self.signe()?;
            } else if self.est_op('/') {
                self.pos += 1;
                let d = self.signe()?;
                if d == 0.0 {
                    return Err("division par zéro".into());
                }
                v /= d;
            } else if matches!(self.suivant(), Some(Jeton::Nombre(_) | Jeton::Nom(_)) | Some(Jeton::Op('('))) {
                // Multiplication implicite : 2x, 2(x+1), 2 sqrt(3)
                v *= self.puissance()?;
            } else {
                return Ok(v);
            }
        }
    }

    fn signe(&mut self) -> Result<f64, String> {
        if self.est_op('-') {
            self.pos += 1;
            return Ok(-self.signe()?);
        }
        if self.est_op('+') {
            self.pos += 1;
            return self.signe();
        }
        self.puissance()
    }

    fn puissance(&mut self) -> Result<f64, String> {
        let base = self.valeur()?;
        if self.est_op('^') {
            self.pos += 1;
            // ^ est associatif à droite et plus prioritaire que le signe : -2^2 = -4, 2^-1 = 0,5.
            let exposant = self.signe()?;
            return Ok(base.powf(exposant));
        }
        Ok(base)
    }

    fn valeur(&mut self) -> Result<f64, String> {
        match self.suivant().cloned() {
            Some(Jeton::Nombre(n)) => {
                self.pos += 1;
                Ok(n)
            }
            Some(Jeton::Op('(')) => {
                self.pos += 1;
                let v = self.somme()?;
                if !self.est_op(')') {
                    return Err("parenthèse fermante manquante".into());
                }
                self.pos += 1;
                Ok(v)
            }
            Some(Jeton::Nom(nom)) => {
                self.pos += 1;
                if let Some(f) = fonction(&nom) {
                    // sqrt(…) ou sqrt 3
                    let argument = if self.est_op('(') { self.valeur()? } else { self.puissance()? };
                    let r = f(argument);
                    return if r.is_finite() { Ok(r) } else { Err(format!("{nom}({argument}) n'est pas défini")) };
                }
                match nom.as_str() {
                    "pi" | "π" => Ok(std::f64::consts::PI),
                    _ => self.variables.get(&nom).copied().ok_or_else(|| format!("variable inconnue : {nom}")),
                }
            }
            Some(Jeton::Op(c)) => Err(format!("« {c} » inattendu")),
            None => Err("expression incomplète".into()),
        }
    }
}

fn fonction(nom: &str) -> Option<fn(f64) -> f64> {
    Some(match nom {
        "sqrt" | "racine" => f64::sqrt,
        "abs" => f64::abs,
        "exp" => f64::exp,
        "ln" => f64::ln,
        "log" => f64::log10,
        "sin" => f64::sin,
        "cos" => f64::cos,
        "tan" => f64::tan,
        _ => return None,
    })
}

/// Évalue une expression avec des variables.
pub fn evaluer(expression: &str, variables: &HashMap<String, f64>) -> Result<f64, String> {
    let texte = expression.replace('√', " sqrt ").replace(['×', '·'], "*").replace('÷', "/").replace(['−', '–'], "-").replace("**", "^");
    let mut a = Analyseur { jetons: decouper(&texte)?, pos: 0, variables };
    if a.jetons.is_empty() {
        return Err("expression vide".into());
    }
    let v = a.somme()?;
    if a.pos < a.jetons.len() {
        return Err("expression mal formée".into());
    }
    if !v.is_finite() {
        return Err("résultat non défini".into());
    }
    Ok(v)
}

/// Lit le nombre écrit par l'élève : « 2,5 », « -3/4 », « x = 1,41 », « 2√3 », « 12 cm », « 1,5×10^3 ».
pub fn lire_nombre(reponse: &str) -> Option<f64> {
    let mut t = reponse.trim().to_string();
    if let Some(p) = t.rfind('=') {
        t = t[p + 1..].to_string();
    }
    let t = t.replace(['\u{a0}', '\u{202f}'], " ").replace("{,}", ".");
    // Virgule décimale française (on ne cherche pas de séparateur de milliers).
    let t = t.replace(',', ".");
    let t = t.trim().trim_end_matches('.');
    if let Ok(v) = evaluer(t, &HashMap::new()) {
        return Some(v);
    }
    // Avec une unité ou du texte autour : le premier nombre écrit.
    let re = regex::Regex::new(r"-?\d+(?:\.\d+)?").unwrap();
    re.find(t).and_then(|m| m.as_str().parse().ok())
}

/// La réponse de l'élève est-elle juste ? Écart relatif d'au plus 1 %, ou bon arrondi
/// au nombre de décimales écrit par l'élève (1,41 pour √2).
pub fn proche(eleve: f64, attendu: f64, decimales_ecrites: usize) -> bool {
    let ecart = (eleve - attendu).abs();
    if ecart <= 1e-9 || ecart <= 0.01 * attendu.abs() {
        return true;
    }
    if decimales_ecrites > 0 {
        let facteur = 10f64.powi(decimales_ecrites as i32);
        let arrondi = (attendu * facteur).round() / facteur;
        let tronque = (attendu * facteur).trunc() / facteur;
        return (eleve - arrondi).abs() < 1e-9 || (eleve - tronque).abs() < 1e-9;
    }
    false
}

/// Nombre de décimales écrites dans la réponse (« 1,414 » → 3).
pub fn decimales(reponse: &str) -> usize {
    let t = reponse.rsplit('=').next().unwrap_or("").replace(',', ".");
    let re = regex::Regex::new(r"\d+\.(\d+)").unwrap();
    re.captures(&t).map_or(0, |c| c[1].len())
}

/// Affiche un nombre à la française, sans zéros inutiles (2,5 ; 1,4142 ; -3).
pub fn afficher(v: f64) -> String {
    let arrondi = (v * 10_000.0).round() / 10_000.0;
    let mut t = format!("{arrondi:.4}");
    while t.contains('.') && (t.ends_with('0') || t.ends_with('.')) {
        t.pop();
    }
    if t == "-0" {
        t = "0".into();
    }
    t.replace('.', ",")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars(v: &[(&str, f64)]) -> HashMap<String, f64> {
        v.iter().map(|(n, x)| (n.to_string(), *x)).collect()
    }

    #[test]
    fn calculs_de_base() {
        let rien = HashMap::new();
        assert_eq!(evaluer("1 + 2 * 3", &rien), Ok(7.0));
        assert_eq!(evaluer("(1 + 2) * 3", &rien), Ok(9.0));
        assert_eq!(evaluer("2^3^2", &rien), Ok(512.0));
        assert_eq!(evaluer("-2^2", &rien), Ok(-4.0));
        assert_eq!(evaluer("2^-1", &rien), Ok(0.5));
        assert_eq!(evaluer("10 / 4", &rien), Ok(2.5));
        assert_eq!(evaluer("2(3+1)", &rien), Ok(8.0));
        assert_eq!(evaluer("1.5e3", &rien), Ok(1500.0));
        assert_eq!(evaluer("abs(-3) + sqrt(16)", &rien), Ok(7.0));
        assert!((evaluer("2√3", &rien).unwrap() - 2.0 * 3f64.sqrt()).abs() < 1e-12);
        assert!(evaluer("1/0", &rien).is_err());
        assert!(evaluer("sqrt(-1)", &rien).is_err());
        assert!(evaluer("(1+2", &rien).is_err());
        assert!(evaluer("2 +", &rien).is_err());
    }

    #[test]
    fn second_degre() {
        let v = vars(&[("a", 1.0), ("b", -3.0), ("c", 2.0)]);
        assert_eq!(evaluer("b^2-4*a*c", &v), Ok(1.0));
        assert_eq!(evaluer("(-b+sqrt(b^2-4*a*c))/(2*a)", &v), Ok(2.0));
        assert_eq!(evaluer("(-b-sqrt(b^2-4*a*c))/(2*a)", &v), Ok(1.0));
        assert_eq!(evaluer("-b/(2a)", &v), Ok(1.5));
        assert!(evaluer("d + 1", &v).unwrap_err().contains("variable inconnue"));
    }

    #[test]
    fn reponses_d_eleve() {
        assert_eq!(lire_nombre("2,5"), Some(2.5));
        assert_eq!(lire_nombre("x = -3/4"), Some(-0.75));
        assert_eq!(lire_nombre("Δ = 1"), Some(1.0));
        assert_eq!(lire_nombre("12 cm"), Some(12.0));
        assert_eq!(lire_nombre("1,5×10^3"), Some(1500.0));
        assert!((lire_nombre("√2").unwrap() - 2f64.sqrt()).abs() < 1e-12);
        assert_eq!(lire_nombre("je ne sais pas"), None);
    }

    #[test]
    fn tolerance() {
        let racine2 = 2f64.sqrt();
        assert!(proche(1.41, racine2, 2));
        assert!(proche(1.414, racine2, 3));
        assert!(!proche(1.5, racine2, 1));
        assert!(proche(0.33, 1.0 / 3.0, 2));
        assert!(!proche(2.0, 2.4, 0));
        assert!(proche(-3.0, -3.0, 0));
        assert_eq!(decimales("x = 1,414"), 3);
        assert_eq!(afficher(2.5), "2,5");
        assert_eq!(afficher(-3.0), "-3");
        assert_eq!(afficher(racine2), "1,4142");
    }
}
