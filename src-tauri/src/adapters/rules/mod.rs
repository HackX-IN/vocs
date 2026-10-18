/*!
 * SOURCE OF TRUTH KEYWORDS: RuleEnhancer, enhance, RULE_ORDER, text, fillers,
 *   repeats, sentences
 * WHAT:  The deterministic TextEnhancer: applies the rules in text.rs in a
 *        fixed order.
 * WHY:   The ORDER is the design, and it is not arbitrary:
 *          1. whitespace first, so every later rule can assume single spaces;
 *          2. spoken commands next, because they INSERT punctuation that later
 *             rules then need to tidy and capitalise after;
 *          3. fillers before the dictionary, so a filler cannot sit inside a
 *             phrase the dictionary is trying to match, and spoken corrections
 *             straight after them so a stray "um" cannot separate a slip from
 *             the correction that replaces it;
 *          4. the dictionary before stutter removal, so a corrected term is
 *             what gets compared for repetition, and grammar straight after the
 *             dictionary so a user's spelling is already final when the grammar
 *             rules decide what to leave alone;
 *          5. punctuation, then casing, then the terminal stop — casing depends
 *             on sentence boundaries existing, and the terminal stop must come
 *             last or it would be capitalised as a new sentence;
 *          6. the TONE rides on that: Literal leaves right after step 1 with
 *             only the dictionary applied, Formal writes contractions out after
 *             the stutters are gone (so an expansion cannot manufacture a
 *             "repeat"), and Casual drops a one-line message's full stop after
 *             the terminal stop has been added.
 *
 *        Every rule is individually toggleable through EnhanceContext, and the
 *        whole pass is measured in microseconds — it runs inside the finalize
 *        budget, so anything expensive here is spending the product's headline
 *        promise.
 * WHERE: Implements the TextEnhancer port; called by pipeline/deliver.rs.
 */

pub mod fillers;
pub mod grammar;
pub mod repeats;
pub mod sentences;
pub mod text;
pub mod tone;

#[cfg(test)]
mod corpus;

use crate::error::AppResult;
use crate::ports::enhancer::{EnhanceContext, TextEnhancer};
use crate::types::Tone;

pub struct RuleEnhancer;

impl RuleEnhancer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RuleEnhancer {
    fn default() -> Self {
        Self::new()
    }
}

impl TextEnhancer for RuleEnhancer {
    fn id(&self) -> &'static str {
        "rules"
    }

    fn enhance(&self, raw: &str, context: &EnhanceContext) -> AppResult<String> {
        let language = context.language.as_ref();

        // 1. Normalise first — see the module WHY for why the order is fixed.
        let mut out = text::normalise_whitespace(raw);

        if out.is_empty() {
            return Ok(out);
        }

        /*
         * Literal is "type exactly what was heard", and it leaves BEFORE every
         * rule that rewrites words, punctuation or case — in a terminal,
         * capitalising `kubectl` or ending a command with a full stop is a bug.
         * The user's own dictionary still applies: it is their spelling, and it
         * is never a downgrade.
         */
        if context.tone == Tone::Literal {
            if !context.dictionary.is_empty() {
                out = text::apply_dictionary(&out, &context.dictionary);
            }
            return Ok(text::normalise_whitespace(&out));
        }

        // 2. Spoken formatting: inserts punctuation and newlines.
        if context.expand_spoken_commands {
            out = text::expand_spoken_commands(&out, language);
        }

        // 3. Fillers, before the dictionary can try to match across one.
        if context.strip_fillers {
            out = fillers::strip_fillers(&out, language, &context.dictionary, context.filler_strength);
        }

        /*
         * 3b. Spoken self-corrections, AFTER fillers and BEFORE the dictionary.
         * After fillers, so an "um" between the slip and the correction cannot
         * push the replaced span out of reach. Before the dictionary, so the
         * text the dictionary fixes is the text that survives the correction
         * rather than a phrase that is about to be replaced.
         */
        if context.apply_corrections {
            out = text::apply_spoken_corrections(&out, language);
        }

        // 4. The user's vocabulary. Always on — it is never a downgrade.
        if !context.dictionary.is_empty() {
            out = text::apply_dictionary(&out, &context.dictionary);
        }

        // 4b. A handful of unambiguous grammar slips. Opt-in; see grammar.rs.
        if context.fix_grammar {
            out = grammar::fix_grammar(&out, language, &context.dictionary);
        }

        // 5. Chunk seams and genuine stutters. Always on: a doubled word at a
        //    seam is an artefact of our own chunking, not something the user said.
        out = repeats::dedupe_stutters(&out);

        // 5b. Formal register: contractions written out. AFTER the stutters, not
        //     before: "it, it's gonna slip" becomes "it, it is going to slip",
        //     and a pass that ran first would hand the stutter rule a doubled
        //     "it" that the user never said twice.
        if context.tone == Tone::Formal {
            out = tone::formalise(&out, language, &context.dictionary);
        }

        // 6. Spacing and quote characters.
        if context.normalise_punctuation {
            out = text::normalise_punctuation(&out);
        }

        // 7. Casing, which needs sentence boundaries to already exist.
        if context.capitalise_sentences {
            out = sentences::capitalise_sentences(&out);
        }

        // 8. The terminal stop, last, so nothing capitalises after it.
        if context.normalise_punctuation {
            out = sentences::ensure_terminal_punctuation(&out);
        }

        // 9. Casual register, after the stop exists to be removed: a one-line
        //    chat message ends without a full stop. See tone.rs.
        if context.tone == Tone::Casual {
            out = tone::relax_ending(&out);
        }

        Ok(text::normalise_whitespace(&out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{DictionaryEntry, DictionaryId, LanguageCode, MatchKind};

    fn context() -> EnhanceContext {
        EnhanceContext {
            language: Some(LanguageCode("en".into())),
            dictionary: vec![],
            strip_fillers: true,
            filler_strength: Default::default(),
            expand_spoken_commands: true,
            normalise_punctuation: true,
            capitalise_sentences: true,
            apply_corrections: true,
            fix_grammar: false,
            tone: Tone::Natural,
        }
    }

    #[test]
    fn the_full_pass_turns_speech_into_writing() {
        let enhancer = RuleEnhancer::new();
        let raw = "um  so i was thinking we should ship it comma today";

        let out = enhancer.enhance(raw, &context()).expect("enhance");

        assert_eq!(out, "So i was thinking we should ship it, today.");
    }

    #[test]
    fn the_dictionary_runs_before_stutter_removal() {
        // Ordering guard: if these swapped, the corrected term would not be
        // what gets compared for repetition.
        let enhancer = RuleEnhancer::new();
        let mut ctx = context();
        ctx.dictionary = vec![DictionaryEntry {
            id: DictionaryId(1),
            pattern: "clod".into(),
            replacement: "Claude".into(),
            match_kind: MatchKind::Word,
            enabled: true,
            used_at: None,
        }];

        let out = enhancer.enhance("clod clod is great", &ctx).expect("enhance");
        assert_eq!(out, "Claude is great.");
    }

    #[test]
    fn every_rule_can_be_turned_off_independently() {
        let enhancer = RuleEnhancer::new();
        let ctx = EnhanceContext {
            language: Some(LanguageCode("en".into())),
            dictionary: vec![],
            strip_fillers: false,
            filler_strength: Default::default(),
            expand_spoken_commands: false,
            normalise_punctuation: false,
            capitalise_sentences: false,
            apply_corrections: false,
            fix_grammar: false,
            tone: Tone::Natural,
        };

        let raw = "um hello comma world";
        let out = enhancer.enhance(raw, &ctx).expect("enhance");

        // Only whitespace normalisation and seam de-duplication remain, and
        // neither should have changed anything here.
        assert_eq!(out, raw);
    }

    #[test]
    fn a_non_english_transcript_passes_through_the_language_rules_untouched() {
        let enhancer = RuleEnhancer::new();
        let mut ctx = context();
        ctx.language = Some(LanguageCode("hi".into()));

        // English filler and command words must not be treated as such here.
        let out = enhancer.enhance("um like comma", &ctx).expect("enhance");
        assert!(
            out.to_lowercase().contains("um") && out.to_lowercase().contains("comma"),
            "language-specific rules must not fire on another language: got {out:?}"
        );
    }

    #[test]
    fn empty_and_whitespace_only_input_produce_nothing_rather_than_a_lone_full_stop() {
        let enhancer = RuleEnhancer::new();
        assert_eq!(enhancer.enhance("", &context()).expect("enhance"), "");
        assert_eq!(enhancer.enhance("   ", &context()).expect("enhance"), "");
    }

    fn with_tone(tone: Tone) -> EnhanceContext {
        EnhanceContext { tone, ..context() }
    }

    #[test]
    fn natural_is_what_it_was_before_tones_existed() {
        let enhancer = RuleEnhancer::new();
        let raw = "i don't think we should ship it today";
        assert_eq!(
            enhancer.enhance(raw, &with_tone(Tone::Natural)).expect("enhance"),
            "I don't think we should ship it today."
        );
    }

    #[test]
    fn casual_keeps_contractions_and_ends_a_one_line_message_without_a_stop() {
        let enhancer = RuleEnhancer::new();
        let out = enhancer
            .enhance("um i don't think we should ship it today", &with_tone(Tone::Casual))
            .expect("enhance");
        assert_eq!(out, "I don't think we should ship it today");

        // Two sentences: nothing is dropped, so the message is not half-edited.
        let two = enhancer
            .enhance("sounds good. see you at eight", &with_tone(Tone::Casual))
            .expect("enhance");
        assert_eq!(two, "Sounds good. See you at eight.");
    }

    #[test]
    fn formal_writes_contractions_out_and_still_punctuates() {
        let enhancer = RuleEnhancer::new();
        let out = enhancer
            .enhance("i don't think we can't ship it, it's gonna slip", &with_tone(Tone::Formal))
            .expect("enhance");
        assert_eq!(out, "I do not think we cannot ship it, it is going to slip.");
    }

    #[test]
    fn literal_is_exactly_what_was_heard_apart_from_the_dictionary() {
        let enhancer = RuleEnhancer::new();
        // No capital, no stop, no filler removal, no spoken command, no grammar.
        let raw = "um kubectl get pods comma new line";
        assert_eq!(
            enhancer.enhance(raw, &with_tone(Tone::Literal)).expect("enhance"),
            "um kubectl get pods comma new line"
        );

        // The user's dictionary is their own spelling and still applies.
        let mut ctx = with_tone(Tone::Literal);
        ctx.dictionary = vec![DictionaryEntry {
            id: DictionaryId(1),
            pattern: "cube control".into(),
            replacement: "kubectl".into(),
            match_kind: MatchKind::Word,
            enabled: true,
            used_at: None,
        }];
        assert_eq!(enhancer.enhance("run cube control now", &ctx).expect("enhance"), "run kubectl now");
    }

    #[test]
    fn literal_still_tidies_whitespace_and_an_empty_transcript_stays_empty() {
        let enhancer = RuleEnhancer::new();
        assert_eq!(enhancer.enhance("  ls   -la  ", &with_tone(Tone::Literal)).expect("enhance"), "ls -la");
        assert_eq!(enhancer.enhance("   ", &with_tone(Tone::Literal)).expect("enhance"), "");
    }

    #[test]
    fn enhancement_is_deterministic() {
        // The property the table tests depend on.
        let enhancer = RuleEnhancer::new();
        let raw = "um so like we should probably ship this comma today";
        let first = enhancer.enhance(raw, &context()).expect("enhance");
        let second = enhancer.enhance(raw, &context()).expect("enhance");
        assert_eq!(first, second);
    }

    #[test]
    fn the_pass_stays_well_inside_its_millisecond_budget() {
        // It runs inside the finalize budget; a slow rule here spends the
        // product's headline promise.
        let enhancer = RuleEnhancer::new();
        let raw = "um so like ".repeat(400);

        let started = std::time::Instant::now();
        let _ = enhancer.enhance(&raw, &context()).expect("enhance");
        let elapsed = started.elapsed();

        assert!(
            elapsed < std::time::Duration::from_millis(20),
            "enhancement took {elapsed:?} on a long transcript"
        );
    }
}
