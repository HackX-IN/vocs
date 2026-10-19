/*!
 * SOURCE OF TRUTH KEYWORDS: merge_safe_edits, Merged, edit_guard, accept_hunk,
 *   FAMILIES, FUNCTION_WORDS, MAX_CHANGED_SHARE, over_rewrite
 * WHAT:  Takes a sentence and the grammar model's rewrite of it and returns the
 *        sentence with ONLY the safe edits applied, word by word.
 * WHY:   The model is a small seq2seq network fine-tuned on essays, and it
 *        rewrites. Measured on speech-like sentences it fixed "she don't" and
 *        "we was" correctly and also turned "because their going to review it"
 *        into "because they will review it" — a different claim. Pasting its
 *        output wholesale would fix grammar and quietly change what the person
 *        said, and the second failure is the one nobody forgives. So its output
 *        is treated as a list of PROPOSED edits and each one is judged alone:
 *
 *          - A name, a number, a code-looking token, an acronym or a dictionary
 *            term is never touched, in either direction.
 *          - A one-word swap is kept only when it is a grammatical form of the
 *            same word (is/are, a/an, their/they're) or a near-spelling of it
 *            (go/goes, dont/don't). A swap to a different word is a rewrite.
 *          - Inserting or dropping a word is kept only for the small closed set
 *            of function words grammar fixes actually add or remove.
 *          - If the accepted edits would change more than a third of the
 *            sentence, none are kept: that is the model rewriting, not fixing.
 *
 *        Punctuation and capitalisation from the model are ignored — the rule
 *        enhancer already owns both, and the model's habit of re-punctuating is
 *        not a grammar fix. The one exception is a lowercase standalone "i",
 *        which the model reliably capitalises.
 *
 *        Whitespace of the original is preserved exactly, so a newline from a
 *        spoken "new line" survives.
 * WHERE: Called by pipeline/grammar/pass.rs for every sentence the model
 *        rewrote. Pure: no model, no I/O, table-tested.
 */

use std::collections::HashSet;

/// If accepted edits touch more than this share of a sentence's words (and
/// more than `MIN_EDIT_ALLOWANCE` words), the model rewrote instead of fixed.
const MAX_CHANGED_SHARE: f32 = 0.35;
const MIN_EDIT_ALLOWANCE: usize = 4;
const MAX_HUNK_WORDS: usize = 3;

/// Grammatical forms of one word. A swap inside a family is the fix the model
/// is for; a swap across families is a different word.
const FAMILIES: &[&[&str]] = &[
    &["am", "is", "are", "was", "were", "be", "been", "being"],
    &["have", "has", "had", "having"],
    &["do", "does", "did", "done"],
    &["a", "an"],
    &["this", "these"],
    &["that", "those"],
    &["their", "there", "they're", "theyre"],
    &["your", "you're", "youre"],
    &["its", "it's"],
    &["to", "too"],
    &["then", "than"],
    &["whose", "who's"],
    &["affect", "effect"],
    &["don't", "doesn't", "didn't"],
    &["isn't", "aren't", "wasn't", "weren't"],
    &["haven't", "hasn't", "hadn't"],
];

/// The only words a grammar fix adds or removes on its own.
const FUNCTION_WORDS: &[&str] = &[
    "a", "an", "the", "to", "of", "in", "on", "at", "for", "by", "with", "as", "is", "are", "was",
    "were", "be", "been", "am", "have", "has", "had", "do", "does", "did", "will", "would", "it",
    "that", "this", "and", "or", "but", "so",
];

/**
 * SOURCE OF TRUTH KEYWORDS: Merged
 * WHAT:  The guarded sentence and how the edits were judged.
 * WHY:   The counts exist so the pass can log how much of the model's output the
 *        guard is discarding — a rising reject rate is the signal that the
 *        guard is too strict or the model is drifting.
 * WHERE: Returned by merge_safe_edits; consumed by pipeline/grammar/pass.rs.
 */
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Merged {
    pub text: String,
    pub accepted: usize,
    pub rejected: usize,
}

/// One whitespace-separated word, split so punctuation can be kept apart from
/// the letters that are compared.
#[derive(Debug, Clone)]
struct Tok {
    lead: String,
    core: String,
    trail: String,
    /// Lowercased core with curly apostrophes straightened: what is compared.
    norm: String,
}

impl Tok {
    fn parse(raw: &str) -> Self {
        let chars: Vec<char> = raw.chars().collect();
        let start = chars.iter().position(|c| c.is_alphanumeric()).unwrap_or(chars.len());
        let end = chars.iter().rposition(|c| c.is_alphanumeric()).map_or(start, |i| i + 1);
        let lead: String = chars[..start].iter().collect();
        let core: String = chars[start..end].iter().collect();
        let trail: String = chars[end..].iter().collect();
        let norm = core.replace('\u{2019}', "'").to_lowercase();
        Self { lead, core, trail, norm }
    }

    fn text(&self) -> String {
        format!("{}{}{}", self.lead, self.core, self.trail)
    }
}

/// Splits into tokens and the whitespace around them: `gaps.len() == toks.len() + 1`.
fn tokenise(text: &str) -> (Vec<Tok>, Vec<String>) {
    let mut toks = Vec::new();
    let mut gaps = vec![String::new()];
    let mut word = String::new();
    for ch in text.chars() {
        if ch.is_whitespace() {
            if word.is_empty() {
                if let Some(gap) = gaps.last_mut() {
                    gap.push(ch);
                }
            } else {
                toks.push(Tok::parse(&word));
                word.clear();
                gaps.push(ch.to_string());
            }
        } else {
            word.push(ch);
        }
    }
    if !word.is_empty() {
        toks.push(Tok::parse(&word));
        gaps.push(String::new());
    }
    (toks, gaps)
}

/// One differing stretch: original tokens `a0..a1` became candidate tokens `b0..b1`.
#[derive(Debug, Clone, Copy)]
struct Hunk {
    a0: usize,
    a1: usize,
    b0: usize,
    b1: usize,
}

/// Longest-common-subsequence diff over normalised words. Sentences are capped
/// near 60 words, so the quadratic table is a few thousand cells.
fn diff(a: &[Tok], b: &[Tok]) -> Vec<Hunk> {
    let (n, m) = (a.len(), b.len());
    let mut lcs = vec![vec![0u16; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if a[i].norm == b[j].norm {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }

    let mut hunks = Vec::new();
    let (mut i, mut j) = (0, 0);
    let mut open: Option<(usize, usize)> = None;
    while i < n || j < m {
        if i < n && j < m && a[i].norm == b[j].norm {
            if let Some((a0, b0)) = open.take() {
                hunks.push(Hunk { a0, a1: i, b0, b1: j });
            }
            i += 1;
            j += 1;
            continue;
        }
        open.get_or_insert((i, j));
        if j < m && (i == n || lcs[i][j + 1] >= lcs[i + 1][j]) {
            j += 1;
        } else {
            i += 1;
        }
    }
    if let Some((a0, b0)) = open {
        hunks.push(Hunk { a0, a1: n, b0, b1: m });
    }
    hunks
}

fn levenshtein(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let above = row[j + 1];
            row[j + 1] = (diagonal + usize::from(ca != *cb)).min(row[j] + 1).min(above + 1);
            diagonal = above;
        }
    }
    row[b.len()]
}

fn same_family(a: &str, b: &str) -> bool {
    FAMILIES.iter().any(|family| family.contains(&a) && family.contains(&b))
}

/// A near-spelling of the same word: go/goes, buy/buyed, dont/don't, writed/wrote.
fn near_spelling(a: &str, b: &str) -> bool {
    if a.chars().next() != b.chars().next() {
        return false;
    }
    if a.starts_with(b) || b.starts_with(a) {
        return true;
    }
    a.chars().count() >= 4 && b.chars().count() >= 4 && levenshtein(a, b) <= 2
}

fn is_function_word(norm: &str) -> bool {
    FUNCTION_WORDS.contains(&norm)
}

/// Words the person said that no grammar fix may touch.
fn is_protected(tok: &Tok, at_sentence_start: bool, protected: &HashSet<String>) -> bool {
    let core = tok.core.as_str();
    if core.is_empty() {
        return false;
    }
    if protected.contains(&tok.norm) {
        return true;
    }
    if core.chars().any(|c| c.is_ascii_digit()) {
        return true;
    }
    // File names, handles, paths, assignments, identifiers.
    if core.chars().any(|c| matches!(c, '.' | '_' | '/' | '@' | '#' | '\\' | ':' | '=')) {
        return true;
    }
    let letters: Vec<char> = core.chars().filter(|c| c.is_alphabetic()).collect();
    // Acronyms ("API") and inner capitals ("iPhone", "GitHub").
    if letters.len() >= 2 && letters.iter().all(|c| c.is_uppercase()) {
        return true;
    }
    if letters.iter().skip(1).any(|c| c.is_uppercase()) {
        return true;
    }
    // A capitalised word that is not the sentence's first is a name, except the
    // pronoun "I" and its contractions.
    let is_pronoun_i = tok.norm == "i" || tok.norm.starts_with("i'");
    let capitalised = letters.first().is_some_and(|c| c.is_uppercase());
    capitalised && !at_sentence_start && !is_pronoun_i
}

/// Whether one proposed replacement of `removed` by `added` is a fix.
fn replacement_is_a_fix(removed: &[Tok], added: &[Tok]) -> bool {
    if removed.len() == added.len() {
        return removed
            .iter()
            .zip(added)
            .all(|(r, a)| same_family(&r.norm, &a.norm) || near_spelling(&r.norm, &a.norm));
    }
    // Different word counts: "alot" -> "a lot", "can not" -> "cannot",
    // "dont" -> "do not".
    let strip = |toks: &[Tok]| -> String {
        toks.iter().flat_map(|t| t.norm.chars().filter(|c| *c != '\'')).collect()
    };
    let (r, a) = (strip(removed), strip(added));
    let longest = r.chars().count().max(a.chars().count()).max(1);
    let similarity = 1.0 - levenshtein(&r, &a) as f32 / longest as f32;
    similarity >= 0.7
}

fn accept_hunk(a: &[Tok], b: &[Tok], hunk: Hunk, protected: &HashSet<String>) -> bool {
    let removed = &a[hunk.a0..hunk.a1];
    let added = &b[hunk.b0..hunk.b1];

    if removed.len() > MAX_HUNK_WORDS || added.len() > MAX_HUNK_WORDS {
        return false;
    }
    if removed.iter().enumerate().any(|(k, t)| is_protected(t, hunk.a0 + k == 0, protected)) {
        return false;
    }
    // The model introducing a name or number is as bad as it changing one.
    if added.iter().enumerate().any(|(k, t)| is_protected(t, hunk.a0 + k == 0, protected)) {
        return false;
    }

    match (removed.is_empty(), added.is_empty()) {
        // Insertion, never after the last word: a trailing addition is the
        // model finishing the thought for the speaker —
        // nor before the first, where it would also have to re-case two words.
        (true, false) => {
            hunk.a0 > 0
                && hunk.a0 < a.len()
                && added.len() <= 2
                && added.iter().all(|t| is_function_word(&t.norm))
        }
        // A deleted word must carry no punctuation, which would be lost with it.
        (false, true) => {
            removed.len() <= 2
                && removed
                    .iter()
                    .all(|t| is_function_word(&t.norm) && t.lead.is_empty() && t.trail.is_empty())
        }
        (false, false) => replacement_is_a_fix(removed, added),
        (true, true) => false,
    }
}

/// Gives the replacement the capitalisation of the word it replaces.
fn match_case(original: &str, replacement: &str) -> String {
    let starts_upper = original.chars().next().is_some_and(char::is_uppercase);
    let mut chars = replacement.chars();
    match chars.next() {
        Some(first) if starts_upper => first.to_uppercase().chain(chars).collect(),
        Some(first) => first.to_lowercase().chain(chars).collect(),
        None => String::new(),
    }
}

/**
 * WHAT:  `original` with the safe subset of the model's edits applied.
 * WHY:   See the module WHY. Returns the original unchanged when nothing is
 *        safe, so the caller never has to compare.
 * WHERE: pipeline/grammar/pass.rs.
 */
pub fn merge_safe_edits(original: &str, candidate: &str, protected: &HashSet<String>) -> Merged {
    let (a, gaps) = tokenise(original);
    let (b, _) = tokenise(candidate);
    let unchanged = || Merged { text: original.to_string(), accepted: 0, rejected: 0 };
    if a.is_empty() || b.is_empty() {
        return unchanged();
    }

    let hunks = diff(&a, &b);
    let mut accepted = Vec::new();
    let mut rejected = 0;
    for hunk in hunks {
        if accept_hunk(&a, &b, hunk, protected) {
            accepted.push(hunk);
        } else {
            rejected += 1;
        }
    }

    let changed: usize = accepted.iter().map(|h| (h.a1 - h.a0).max(h.b1 - h.b0)).sum();
    let allowance = MIN_EDIT_ALLOWANCE.max((a.len() as f32 * MAX_CHANGED_SHARE) as usize);
    if changed > allowance {
        return Merged { text: original.to_string(), accepted: 0, rejected: rejected + accepted.len() };
    }

    // The standalone pronoun "i" is the one case-only change worth taking.
    let mut capitalise_i = vec![false; a.len()];
    let mut matches = diff_matches(&a, &b);
    for (ai, bj) in matches.drain(..) {
        let pronoun = a[ai].norm == "i" || a[ai].norm.starts_with("i'");
        let lowered = a[ai].core.starts_with('i');
        if pronoun && lowered && b[bj].core.starts_with('I') {
            capitalise_i[ai] = true;
        }
    }

    // Rebuild: (whitespace before, text) per original position.
    let mut out = String::with_capacity(original.len() + 8);
    let mut i = 0;
    let mut next = accepted.iter().peekable();
    while i < a.len() {
        match next.peek().copied() {
            // A dropped word takes its whitespace with it.
            Some(h) if h.a0 == i && h.a1 > h.a0 && h.b1 == h.b0 => {
                i = h.a1;
                next.next();
            }
            Some(h) if h.a0 == i && h.a1 > h.a0 => {
                out.push_str(&gaps[i]);
                out.push_str(&replacement_text(&a[h.a0..h.a1], &b[h.b0..h.b1]));
                i = h.a1;
                next.next();
            }
            Some(h) if h.a0 == i => {
                // Pure insertion before token i.
                out.push_str(&gaps[i]);
                let inserted: Vec<&str> = b[h.b0..h.b1].iter().map(|t| t.norm.as_str()).collect();
                out.push_str(&inserted.join(" "));
                out.push(' ');
                out.push_str(&a[i].text());
                i += 1;
                next.next();
            }
            _ => {
                out.push_str(&gaps[i]);
                if capitalise_i[i] {
                    out.push_str(&a[i].lead);
                    out.push('I');
                    out.push_str(&a[i].core[1..]);
                    out.push_str(&a[i].trail);
                } else {
                    out.push_str(&a[i].text());
                }
                i += 1;
            }
        }
    }
    out.push_str(&gaps[a.len()]);
    // A dropped first word leaves its neighbour's leading space behind.
    if gaps[0].is_empty() {
        out = out.trim_start().to_string();
    }

    Merged { text: out, accepted: accepted.len(), rejected }
}

/// The aligned (original, candidate) index pairs, for case-only adjustments.
fn diff_matches(a: &[Tok], b: &[Tok]) -> Vec<(usize, usize)> {
    let hunks = diff(a, b);
    let (mut i, mut j) = (0, 0);
    let mut pairs = Vec::new();
    for h in hunks {
        while i < h.a0 && j < h.b0 {
            pairs.push((i, j));
            i += 1;
            j += 1;
        }
        i = h.a1;
        j = h.b1;
    }
    while i < a.len() && j < b.len() {
        pairs.push((i, j));
        i += 1;
        j += 1;
    }
    pairs
}

/// The text a replaced stretch becomes: the model's words, wearing the original
/// stretch's capitalisation and its leading and trailing punctuation.
fn replacement_text(removed: &[Tok], added: &[Tok]) -> String {
    let lead = removed.first().map_or("", |t| t.lead.as_str());
    let trail = removed.last().map_or("", |t| t.trail.as_str());
    let first_original = removed.first().map_or("", |t| t.core.as_str());
    let words: Vec<String> = added
        .iter()
        .enumerate()
        .map(|(k, t)| if k == 0 { match_case(first_original, &t.core) } else { t.core.clone() })
        .collect();
    format!("{lead}{}{trail}", words.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn merge(original: &str, candidate: &str) -> String {
        merge_safe_edits(original, candidate, &HashSet::new()).text
    }

    #[test]
    fn agreement_and_form_fixes_are_taken() {
        assert_eq!(merge("I has a idea.", "I have an idea."), "I have an idea.");
        assert_eq!(
            merge(
                "She don't know what we was talking about yesterday.",
                "She doesn't know what we were talking about yesterday."
            ),
            "She doesn't know what we were talking about yesterday."
        );
        assert_eq!(merge("He go to school and it were fun.", "He went to school and it was fun."), "He go to school and it was fun.");
    }

    #[test]
    fn a_new_claim_is_refused() {
        // The model's real output for this input changed "their going to" into "they will".
        let original = "So I was thinking we should ship it today, because their going to review it tomorrow.";
        let candidate = "So I was thinking we should ship it today, because they will review it tomorrow.";
        assert_eq!(merge(original, candidate), original);
    }

    #[test]
    fn near_spellings_of_the_same_word_are_taken() {
        assert_eq!(merge("Me and him goes to the store.", "Me and him go to the store."), "Me and him go to the store.");
        assert_eq!(merge("I dont think so.", "I don't think so."), "I don't think so.");
        assert_eq!(merge("We buyed some milk.", "We buy some milk."), "We buy some milk.");
    }

    #[test]
    fn names_numbers_and_code_are_never_touched() {
        let protected = HashSet::new();
        for (original, candidate) in [
            ("Send it to Priya today.", "Send it to Priyanka today."),
            ("Pay invoice 4821 today.", "Pay invoice 4812 today."),
            ("Open main.rs and look.", "Open main.js and look."),
            ("We use Kubernetes now.", "We use Kubernetes Now."),
            ("The API is down.", "The Api is down."),
        ] {
            assert_eq!(merge_safe_edits(original, candidate, &protected).text, original, "{original}");
        }
    }

    #[test]
    fn the_users_dictionary_terms_are_protected_in_either_direction() {
        let protected: HashSet<String> = ["kubectl".to_string()].into();
        let merged = merge_safe_edits("Run kubectl get pods.", "Run kubectl gets pods.", &protected);
        // "get" -> "gets" is a legal near-spelling on its own; the point is that
        // the dictionary word itself cannot be the thing that changes.
        assert!(merged.text.contains("kubectl"));
        let swapped = merge_safe_edits("Run kubectl now.", "Run kubectl-ctl now.", &protected);
        assert_eq!(swapped.text, "Run kubectl now.");
    }

    #[test]
    fn small_function_word_insertions_and_removals_are_taken() {
        assert_eq!(merge("I want to to go home.", "I want to go home."), "I want to go home.");
        assert_eq!(merge("She is best player.", "She is the best player."), "She is the best player.");
    }

    #[test]
    fn a_content_word_the_model_invents_or_drops_is_refused() {
        let original = "We should ship it today.";
        assert_eq!(merge(original, "We should definitely ship it today."), original);
        assert_eq!(merge("We should not ship it.", "We should ship it."), "We should not ship it.");
        // Finishing the speaker's sentence for them.
        assert_eq!(merge("I think we should", "I think we should go"), "I think we should");
    }

    #[test]
    fn a_rewrite_is_not_a_fix() {
        let original = "me and him goes there and buyed some milks yesterday";
        let rewrite = "He and I went there and bought some milk yesterday";
        // The pronoun swap, "went" and "bought" are content changes and none may
        // survive; the plural "milks" -> "milk" is the one fix inside it.
        assert_eq!(merge(original, rewrite), "me and him goes there and buyed some milk yesterday");
    }

    #[test]
    fn too_many_accepted_edits_means_the_model_is_rewriting() {
        let original = "he go and she go and we goes and they goes and it go";
        let candidate = "he goes and she goes and we go and they go and it goes";
        assert_eq!(merge(original, candidate), original);
    }

    #[test]
    fn punctuation_and_case_from_the_model_are_ignored() {
        assert_eq!(merge("hello world", "Hello, world!"), "hello world");
        assert_eq!(merge("We ship today.", "We ship today"), "We ship today.");
    }

    #[test]
    fn a_standalone_lowercase_i_is_capitalised() {
        assert_eq!(merge("so i think i'm ready.", "So I think I'm ready."), "so I think I'm ready.");
    }

    #[test]
    fn the_original_whitespace_and_surrounding_punctuation_survive() {
        assert_eq!(
            merge("First, she  don't know.\nThen we leave.", "First, she doesn't know. Then we leave."),
            "First, she  doesn't know.\nThen we leave."
        );
        assert_eq!(merge("\"She don't,\" he said.", "\"She doesn't,\" he said."), "\"She doesn't,\" he said.");
    }

    #[test]
    fn a_replacement_at_the_start_keeps_the_capital() {
        assert_eq!(merge("Dont go there.", "Don't go there."), "Don't go there.");
    }

    #[test]
    fn splits_and_joins_of_the_same_letters_are_taken() {
        assert_eq!(merge("I like it alot.", "I like it a lot."), "I like it a lot.");
        assert_eq!(merge("We can not do it.", "We cannot do it."), "We cannot do it.");
    }

    #[test]
    fn identical_text_and_empty_candidates_change_nothing() {
        assert_eq!(merge("All good here.", "All good here."), "All good here.");
        assert_eq!(merge("All good here.", ""), "All good here.");
        assert_eq!(merge("", "Something."), "");
    }

    #[test]
    fn counts_report_what_was_judged() {
        let merged = merge_safe_edits(
            "She don't know Priya.",
            "She doesn't know Priyanka.",
            &HashSet::new(),
        );
        assert_eq!(merged.text, "She doesn't know Priya.");
        assert_eq!((merged.accepted, merged.rejected), (1, 1));
    }
}
