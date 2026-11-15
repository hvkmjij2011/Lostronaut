//! Pronounceable procedural names: random but following simple phonotactic rules.
use crate::util::Rng;

const ONSETS: &[&str] = &[
    "b", "d", "f", "g", "h", "k", "l", "m", "n", "p", "r", "s", "t", "v", "z", "l", "m", "n", "r", "s", "t", "k",
    "br", "dr", "kr", "tr", "th", "sh", "st", "gl", "pr", "vr", "sk", "ch", "y", "kh", "",
];
const VOWELS: &[&str] = &["a", "e", "i", "o", "u", "a", "e", "o", "a", "i", "ia", "ae", "ei", "au", "io"];
const CODAS: &[&str] = &["", "", "", "", "n", "r", "l", "s", "m", "th", "x", "nd", "rn", "lt"];
const PLANET_END: &[&str] = &[
    "a", "ia", "on", "is", "us", "ar", "or", "en", "ara", "une", "eth", "os", "ea", "ir", "yn", "an", "ora", "el",
];
const STAR_END: &[&str] = &["is", "us", "ar", "on", "ax", "or", "eus", "ion", "ara", "el", "an"];
const SPECIES_END: &[&str] = &["ora", "ella", "anthus", "ium", "ica", "osa", "ia", "ax", "ula", "ensis"];

fn is_vowel(c: char) -> bool {
    matches!(c, 'a' | 'e' | 'i' | 'o' | 'u' | 'y')
}

/// Rejects ugly results: 3+ consonants or vowels in a row, repeated letters, etc.
fn acceptable(s: &str, min: usize, max: usize) -> bool {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() < min || chars.len() > max {
        return false;
    }
    let (mut cons, mut vow) = (0, 0);
    for (i, &c) in chars.iter().enumerate() {
        if is_vowel(c) {
            vow += 1;
            cons = 0;
        } else {
            cons += 1;
            vow = 0;
        }
        if cons > 2 || vow > 2 {
            return false;
        }
        if i > 0 && chars[i - 1] == c {
            return false;
        }
        if i > 3 && chars[i - 2] == c && chars[i - 3] == chars[i - 1] {
            return false; // "anan" style stutter
        }
    }
    !is_vowel(chars[0]) || chars.len() > 3
}

fn syllable(rng: &mut Rng, coda: bool) -> String {
    let mut s = String::new();
    s.push_str(rng.pick(ONSETS));
    s.push_str(rng.pick(VOWELS));
    if coda {
        s.push_str(rng.pick(CODAS));
    }
    s
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// Well-known names from other fiction, games and brands that the generator must never produce
/// by chance (compared without case and accents). Real astronomical names are fine.
const RESERVED: &[&str] = &[
    // planets and places from books, films and series
    "arrakis", "caladan", "giedi", "salusa", "tatooine", "naboo", "hoth", "endor", "dagobah", "coruscant",
    "kashyyyk", "mustafar", "alderaan", "jakku", "geonosis", "kamino", "bespin", "yavin", "vulcan", "romulus",
    "qonos", "kronos", "bajor", "cardassia", "krypton", "gallifrey", "skaro", "trantor", "terminus", "solaris",
    "pandora", "magrathea", "arda", "mordor", "gondor", "rohan", "narnia", "riverworld", "hyperion", "pern",
    "arrakeen", "tralfamadore", "vogsphere", "eternia", "thra", "lv", "acheron", "fiorina", "zion", "cybertron",
    "namek", "vegeta", "tralfaz", "ego", "xandar", "sakaar", "asgard", "titan", "knowhere", "abydos", "chulak",
    "dakara", "atlantis", "caprica", "kobol", "galactica", "miranda", "persephone", "reach", "harvest",
    "zebes", "tallon", "aether", "hylia", "hyrule", "corneria", "venom", "sauria", "mobius", "kerbin", "duna",
    "eve", "jool", "laythe", "eeloo", "sylva", "desolo", "calidor", "vesania", "novus", "glacio", "atrox",
    "rapture", "columbia", "aiur", "char", "korhal", "tarsonis", "mar sara", "azeroth", "draenor", "tamriel",
    "nirn", "thedas", "sera", "lodis", "tython", "korriban", "dromund", "ilum", "malachor", "tuchanka",
    "thessia", "palaven", "sur kesh", "rannoch", "omega", "citadel", "eden", "feros", "noveria", "virmire",
    "ilos", "aegis", "pyrrhus", "lumen", "terra nova", "arcadia", "elysium", "avalon",
    // brands and studios
    "nasa", "spacex", "sony", "sega", "atari", "nintendo", "valve", "bungie", "bioware",
];

/// True when `name` matches (or starts a word with) a reserved name.
fn reserved(name: &str) -> bool {
    let n: String = name.to_lowercase().chars().filter(|c| c.is_ascii_alphabetic()).collect();
    RESERVED.iter().any(|r| {
        let r: String = r.chars().filter(|c| c.is_ascii_alphabetic()).collect();
        // short reserved words only match exactly, longer ones also as a prefix
        n == r || (r.len() >= 5 && n.starts_with(&r))
    })
}

fn make(rng: &mut Rng, endings: &[&str], min: usize, max: usize) -> String {
    for _ in 0..200 {
        let n = 1 + rng.int(2);
        let mut s = String::new();
        for i in 0..n {
            let coda = i + 1 < n && rng.chance(0.35);
            s.push_str(&syllable(rng, coda));
        }
        // the ending starts with a vowel: make sure the stem ends in a consonant
        if s.chars().last().map(is_vowel).unwrap_or(false) {
            let c = *rng.pick(&["l", "r", "n", "s", "t", "m", "v", "d", "k", "th", "z"]);
            s.push_str(c);
        }
        s.push_str(rng.pick(endings));
        if acceptable(&s, min, max) && !reserved(&s) {
            return capitalize(&s);
        }
    }
    "Terra".to_string()
}

pub fn star_name(rng: &mut Rng) -> String {
    make(rng, STAR_END, 5, 9)
}

pub fn planet_name(rng: &mut Rng, taken: &[String]) -> String {
    loop {
        let n = make(rng, PLANET_END, 4, 9);
        if !taken.iter().any(|t| t == &n || t[..2] == n[..2]) {
            return n;
        }
    }
}

pub fn species_name(rng: &mut Rng) -> String {
    make(rng, SPECIES_END, 5, 11)
}

pub fn ship_name(rng: &mut Rng) -> String {
    make(rng, &["a", "is", "on", "ara"], 5, 8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn never_produces_reserved_names() {
        let mut r = Rng::new(7);
        let mut taken = Vec::new();
        for _ in 0..20000 {
            for n in [star_name(&mut r), planet_name(&mut r, &taken), species_name(&mut r), ship_name(&mut r)] {
                assert!(!reserved(&n), "generated a reserved name: {n}");
            }
            if taken.len() > 6 {
                taken.clear();
            }
            taken.push(planet_name(&mut r, &taken));
        }
        assert!(reserved("Tatooine") && reserved("Arrakisa") && reserved("Kerbin") && !reserved("Rinon"));
    }
    #[test]
    fn print_names() {
        let mut r = Rng::new(42);
        for _ in 0..12 {
            println!("{} / {} / {}", star_name(&mut r), planet_name(&mut r, &[]), species_name(&mut r));
        }
    }
}
