/*!
 * SOURCE OF TRUTH KEYWORDS: Tok, tokenise, word_tokens, token_gaps, norm_word
 * WHAT:  Splits text into whitespace-separated words — each one divided into its
 *        leading punctuation, its letters and its trailing punctuation — and the
 *        whitespace between them.
 * WHY:   Two modules need exactly this view of a sentence and must agree on it:
 *        the edit guard compares the model's words with the original's, and the
 *        learned-correction applier matches a remembered phrase against the
 *        transcript. A second tokenizer would disagree about what a word is
 *        (is "don't," one word?) and a learned fix would stop matching the very
 *        text it was learned from. Whitespace is kept verbatim so rebuilding the
 *        text never flattens a newline.
 * WHERE: Used by pipeline/grammar/guard.rs and pipeline/grammar/learned.rs.
 */

/// One whitespace-separated word, split so punctuation can be kept apart from
/// the letters that are compared.
#[derive(Debug, Clone)]
pub(super) struct Tok {
    pub(super) lead: String,
    pub(super) core: String,
    pub(super) trail: String,
    /// Lowercased core with curly apostrophes straightened: what is compared.
    pub(super) norm: String,
}

impl Tok {
    pub(super) fn parse(raw: &str) -> Self {
        let chars: Vec<char> = raw.chars().collect();
        let start = chars.iter().position(|c| c.is_alphanumeric()).unwrap_or(chars.len());
        let end = chars.iter().rposition(|c| c.is_alphanumeric()).map_or(start, |i| i + 1);
        let lead: String = chars[..start].iter().collect();
        let core: String = chars[start..end].iter().collect();
        let trail: String = chars[end..].iter().collect();
        let norm = core.replace('\u{2019}', "'").to_lowercase();
        Self { lead, core, trail, norm }
    }

    pub(super) fn text(&self) -> String {
        format!("{}{}{}", self.lead, self.core, self.trail)
    }
}

/// Splits into tokens and the whitespace around them: `gaps.len() == toks.len() + 1`.
pub(super) fn tokenise(text: &str) -> (Vec<Tok>, Vec<String>) {
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

