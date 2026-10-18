/*!
 * SOURCE OF TRUTH KEYWORDS: enhance_context, EnhanceContext, SessionSettings,
 *   live_captions, paste_preview, one_rule_set
 * WHAT:  The EnhanceContext a SessionSettings implies, for a given language and
 *        dictionary.
 * WHY:   Built in one place because it is used in two: the delivery worker
 *        enhances the finished transcript with it, and the live captions enhance
 *        the transcript-so-far with it. Two hand-written copies would drift — a
 *        caption showing one rule set while the paste used another is a preview
 *        that lies about what is about to be written. Lives outside
 *        settings_view.rs, which only LOADS settings: reading them here is what
 *        the registry reachability test counts as consuming them.
 * WHERE: session/delivery.rs and session/captions.rs.
 */

use crate::ports::enhancer::EnhanceContext;
use crate::session::SessionSettings;
use crate::types::{DictionaryEntry, LanguageCode};

pub fn enhance_context(
    settings: &SessionSettings,
    language: Option<LanguageCode>,
    dictionary: Vec<DictionaryEntry>,
) -> EnhanceContext {
    EnhanceContext {
        language,
        dictionary,
        strip_fillers: settings.strip_fillers,
        filler_strength: settings.filler_strength,
        expand_spoken_commands: settings.spoken_commands,
        normalise_punctuation: settings.normalise_punctuation,
        capitalise_sentences: settings.capitalise_sentences,
        apply_corrections: settings.apply_corrections,
        fix_grammar: settings.fix_grammar,
        tone: settings.tone,
    }
}
