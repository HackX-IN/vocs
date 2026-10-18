/*!
 * SOURCE OF TRUTH KEYWORDS: Caption, CaptionChanged, live_captions, preview_before_paste,
 *   caption_window, heard_vs_fixed
 * WHAT:  What the caption HUD is showing: the words appearing as you speak
 *        (Live), or the finished text held for a moment before it is pasted
 *        (Preview).
 * WHY:   One window and one type for two features, because they are the same
 *        object in two moments of a dictation: the HUD that shows what Vocs is
 *        hearing while you talk is the one that shows what it is about to write
 *        when you stop. Data-carrying variants, like SessionState, so the
 *        frontend cannot render a Preview without its hold time or a Live
 *        caption without both texts — and a new variant is a compile error in
 *        the view rather than a blank window.
 *
 *        `heard` is the model's raw words and `fixed` is what the rules make of
 *        them. They are sent together and the view decides whether to show both:
 *        when they are identical, repeating them would be noise.
 * WHERE: Emitted by session/captions.rs and session/preview.rs through the
 *        EventSink port; rendered by src/app/caption.
 */

use serde::{Deserialize, Serialize};
use specta::Type;

use super::numeric::TsNumber;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Caption {
    /// Words appearing as you speak.
    Live { heard: String, fixed: String },
    /// The corrected text, held for `hold_ms` before it is pasted. Escape
    /// withholds it.
    Preview {
        text: String,
        #[specta(type = TsNumber)]
        hold_ms: u64,
    },
}
