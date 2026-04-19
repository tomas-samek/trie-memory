use super::node::fnv1a_hash;

/// Split an input string into lowercase word tokens.
///
/// Boundaries (in order):
///   1. Whitespace, `.`, `_`, `-`, `/`, `:`
///   2. CamelCase: insert boundary before an uppercase letter followed by a lowercase letter,
///      OR between a lowercase/digit letter and an uppercase letter.
///   3. Lowercase all tokens.
///   4. Drop empty tokens.
pub fn split_words(input: &str) -> Vec<String> {
    let mut raw_tokens: Vec<String> = Vec::new();
    let mut current = String::new();

    for ch in input.chars() {
        if ch.is_whitespace() || matches!(ch, '.' | '_' | '-' | '/' | ':') {
            if !current.is_empty() {
                raw_tokens.push(std::mem::take(&mut current));
            }
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        raw_tokens.push(current);
    }

    let mut out: Vec<String> = Vec::new();
    for tok in raw_tokens {
        for sub in split_camel_case(&tok) {
            let lower = sub.to_lowercase();
            if !lower.is_empty() {
                out.push(lower);
            }
        }
    }
    out
}

fn split_camel_case(tok: &str) -> Vec<String> {
    let chars: Vec<char> = tok.chars().collect();
    if chars.is_empty() {
        return vec![];
    }

    let mut boundaries: Vec<usize> = vec![0];
    for i in 1..chars.len() {
        let prev = chars[i - 1];
        let curr = chars[i];

        // lowercase/digit → uppercase: "getName" → get|Name
        let lower_to_upper = (prev.is_lowercase() || prev.is_ascii_digit()) && curr.is_uppercase();

        // upper → upper followed by lower: "HTMLParser" → HTML|Parser
        let upper_run_to_word = prev.is_uppercase()
            && curr.is_uppercase()
            && i + 1 < chars.len()
            && chars[i + 1].is_lowercase();

        if lower_to_upper || upper_run_to_word {
            boundaries.push(i);
        }
    }
    boundaries.push(chars.len());

    let mut parts = Vec::new();
    for w in boundaries.windows(2) {
        let s: String = chars[w[0]..w[1]].iter().collect();
        if !s.is_empty() {
            parts.push(s);
        }
    }
    parts
}

/// Encode input as visual-weight bytes. This is how eyes see text:
/// - Letters are dense (high values, small differences between them)
/// - Uppercase is heavier than lowercase
/// - Punctuation is light (low values)
/// - Space is nothing (zero)
///
/// The trie's delta encoding then amplifies boundaries:
/// letter->space = huge delta, letter->dot = big delta,
/// lower->Upper = noticeable, letter->letter = tiny (absorbed by root).
pub fn weight_encode(input: &str) -> Vec<u8> {
    input.chars().map(|ch| char_weight(ch)).collect()
}

fn char_weight(ch: char) -> u8 {
    match ch {
        'a'..='z' => 200 + (ch as u8 - b'a'),         // 200-225: dense letter band
        'A'..='Z' => 230 + (ch as u8 - b'A'),         // 230-255: heavier uppercase
        '0'..='9' => 170 + (ch as u8 - b'0'),         // 170-179: medium digits
        ' ' | '\n' | '\r' | '\t' => 0,                 // nothing
        '_' => 16,                                      // almost nothing
        '-' => 24,                                      // light
        '.' => 32,                                      // light structure
        '/' | '\\' => 40,                               // light path separator
        ':' | ';' => 48,                                // light punctuation
        '(' | ')' | '[' | ']' | '{' | '}' => 56,       // brackets
        '<' | '>' => 60,                                // angle brackets
        ',' => 64,                                      // separator
        '=' | '+' | '*' | '&' | '|' => 72,             // operators
        '"' | '\'' | '`' => 80,                         // quotes
        _ if "~@#%^!?".contains(ch) => 88,             // symbols
        _ => 128,                                       // unknown: neutral
    }
}

/// Hash each word to a u32 (FNV-1a, low 32 bits) and emit little-endian bytes.
/// DEPRECATED: kept only for legacy tests; new code should use `word_token`.
pub fn tokenize_to_bytes(input: &str) -> Vec<u8> {
    let words = split_words(input);
    let mut bytes = Vec::with_capacity(words.len() * 4);
    for word in &words {
        let hash = fnv1a_hash(word.as_bytes()) as u32;
        bytes.extend_from_slice(&hash.to_le_bytes());
    }
    bytes
}

/// Hash a single word for diagnostics (u32).
pub fn word_hash(word: &str) -> u32 {
    fnv1a_hash(word.as_bytes()) as u32
}

/// Convert a word to its atomic 2-byte word-trie token (u16 FNV-1a, LE).
pub fn word_token(word: &str) -> [u8; 2] {
    let hash = fnv1a_hash(word.to_lowercase().as_bytes()) as u16;
    hash.to_le_bytes()
}

/// Return (word, token) pairs for each word in `input`.
pub fn tokenize_words(input: &str) -> Vec<(String, [u8; 2])> {
    split_words(input)
        .into_iter()
        .map(|w| {
            let tok = word_token(&w);
            (w, tok)
        })
        .collect()
}

/// Tick gap encoded by a separator character. Silence duration ≈ structural depth.
pub fn silence_ticks(separator: char) -> u64 {
    match separator {
        ' ' | '\t' => 1,
        '-' | '_' => 1,
        ',' => 3,
        ';' | ':' => 5,
        '.' | '!' | '?' => 10,
        '\n' | '\r' => 20,
        _ => 1,
    }
}

/// A single tokenized element: either a concrete word, or a silence gap in ticks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WordOrSilence {
    Word(String),
    Silence(u64),
}

/// Split input into words and silence gaps.
///
/// Rules:
/// - Consecutive letters/digits accumulate into one word.
/// - A separator char emits the current word (if any), then a Silence whose
///   duration is `silence_ticks(char)`. Runs of separators collapse to the
///   MAXIMUM gap seen in the run (paragraph > sentence > word).
/// - A CamelCase boundary (e.g. `CatalogQuery`) emits the prior sub-word then
///   a Silence(1) before the next sub-word.
/// - Empty/whitespace-only inputs yield an empty vec.
pub fn tokenize_with_silence(input: &str) -> Vec<WordOrSilence> {
    let mut out: Vec<WordOrSilence> = Vec::new();
    let mut run = String::new();
    let mut pending_silence: u64 = 0;

    let flush_run = |run: &mut String,
                     out: &mut Vec<WordOrSilence>,
                     pending: &mut u64| {
        if run.is_empty() {
            return;
        }
        // Delegate CamelCase splitting to the shared helper.
        let sub_words = split_camel_case(run);
        for (i, sw) in sub_words.iter().enumerate() {
            let lower = sw.to_lowercase();
            if lower.is_empty() {
                continue;
            }
            let gap = if i == 0 { *pending } else { 1.max(*pending) };
            if gap > 0 {
                out.push(WordOrSilence::Silence(gap));
            }
            out.push(WordOrSilence::Word(lower));
            *pending = 0;
        }
        run.clear();
    };

    for ch in input.chars() {
        let is_sep = ch.is_whitespace()
            || matches!(
                ch,
                '.' | '_' | '-' | '/' | ':' | ';' | ',' | '!' | '?' | '\\'
            );

        if is_sep {
            flush_run(&mut run, &mut out, &mut pending_silence);
            let g = silence_ticks(ch);
            if g > pending_silence {
                pending_silence = g;
            }
        } else {
            run.push(ch);
        }
    }
    flush_run(&mut run, &mut out, &mut pending_silence);

    out
}
