/*!
 * SOURCE OF TRUTH KEYWORDS: LiveCaptions, interim_decode, live_captions, caption_interval,
 *   stale_interim, heard_vs_fixed
 * WHAT:  The bookkeeping for live captions: when to ask the engine for a snapshot
 *        of what has been said so far, whether an answer is still current, and how
 *        to turn it into the two texts the HUD shows — what was HEARD and what
 *        will be WRITTEN.
 * WHY:   The engine only ever decodes a closed chunk (8–15 seconds, cut at a
 *        pause), so without help a caption would arrive in lumps ten seconds
 *        apart. Live captions therefore decode a SNAPSHOT of the open chunk every
 *        INTERVAL — an `Interim` chunk, which takes the engine's shrunk-context
 *        path (the same one that keeps the tail decode fast) and is never added to
 *        the transcript. It is the committed transcript so far plus the
 *        snapshot's words; the real chunk, when it closes, replaces them.
 *
 *        THREE GUARDS, because an interim is speculative work competing with the
 *        real thing:
 *          - ONE IN FLIGHT. A new snapshot is not requested until the last one
 *            has come back, so a slow machine falls behind (captions lag) rather
 *            than queueing work in front of the audio that matters.
 *          - STALENESS. A snapshot is a copy of the buffer at one moment; if that
 *            buffer closes into a real chunk before the answer arrives, the answer
 *            describes audio that is now part of the transcript. Showing it would
 *            print that chunk's words twice. An epoch counter, bumped each time a
 *            chunk closes, is how an answer recognises it is too late and is
 *            dropped.
 *          - OFF BY DEFAULT. It transcribes a little extra while you talk, and a
 *            stop pressed mid-snapshot waits for it — so it is a setting.
 *
 *        THE LAST WORD IS HELD BACK until two snapshots in a row agree on it. A
 *        snapshot ends wherever the microphone happened to be, usually in the
 *        middle of a word, and the decoder dutifully turns that fragment into a
 *        different, real-looking word — "transcri" becomes "transcript" becomes
 *        "transcribes" over three captions. Showing it is what made captions look
 *        like they were mishearing; withholding it costs one interval of delay on
 *        the final word and nothing else, and the finished text is unaffected.
 *
 *        The "fixed" text is produced by the SAME enhancement the paste will use
 *        (`enhance_context`), so the caption never shows a rule
 *        set the paste will not apply.
 * WHERE: Owned by session/actor.rs, which asks `due` on every audio buffer and
 *        routes Interim decode results to `show`.
 */

use crate::ipc::context::SessionContext;
use crate::pipeline::assembler::Assembler;
use crate::session::enhance_context::enhance_context;
use crate::session::settings_view::SessionSettings;
use crate::types::{Caption, DictionaryEntry, LanguageCode, TranscriptSegment};

/// How much AUDIO passes between snapshot requests. Short enough that words
/// appear within a breath of being said, long enough that the engine is idle
/// most of the time.
///
/// Measured in audio time, not wall-clock time. In production the two are the
/// same, because the microphone delivers audio at the speed it is spoken — but
/// audio time is the quantity the cadence is ABOUT, it does not drift when the
/// machine is busy and a scheduler hiccup delays a buffer, and it lets a test
/// replay five seconds of speech in a few milliseconds and still get the
/// snapshots five seconds of speech would produce.
const INTERVAL_MS: u64 = 1_500;

#[derive(Default)]
pub struct LiveCaptions {
    enabled: bool,
    /// The user's dictionary, read once per session — it cannot change while one
    /// is recording, and a database read every interval would be waste.
    dictionary: Vec<DictionaryEntry>,
    /// Audio position (ms captured this session) of the last request.
    last_requested_ms: Option<u64>,
    /// The epoch of the snapshot currently being decoded, if any.
    in_flight: Option<u32>,
    /// Bumped each time a real chunk closes. See the module WHY.
    epoch: u32,
    /// The last pair shown, so an unchanged caption is not re-emitted.
    last_shown: Option<(String, String)>,
    /// The words of the previous snapshot's whole hypothesis, normalised, for
    /// deciding whether the final word has settled. See `withhold_unconfirmed`.
    previous_words: Vec<String>,
}

impl LiveCaptions {
    pub fn new() -> Self {
        Self::default()
    }

    /// A recording is starting.
    pub fn begin(&mut self, enabled: bool, dictionary: Vec<DictionaryEntry>) {
        *self = Self { enabled, dictionary, ..Self::default() };
    }

    /// Capture ended (stopped, cancelled or failed): clear the HUD and stand down.
    /// Unconditional once something was shown — capture ending is exactly when
    /// the words stop being live, whatever state the machine lands in.
    pub fn end(&mut self, ctx: &SessionContext) {
        let was_showing = self.last_shown.take().is_some();
        self.in_flight = None;
        self.last_requested_ms = None;
        if was_showing {
            ctx.ports.events.caption_changed(None);
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// A real chunk closed: any snapshot in flight now describes audio that is
    /// part of the transcript.
    pub fn chunk_closed(&mut self) {
        self.epoch = self.epoch.wrapping_add(1);
    }

    /// Whether it is time to ask for another snapshot, `audio_ms` into the session.
    pub fn due(&self, audio_ms: u64) -> bool {
        self.enabled
            && self.in_flight.is_none()
            && self.last_requested_ms.is_none_or(|last| audio_ms.saturating_sub(last) >= INTERVAL_MS)
    }

    /// A snapshot was submitted (or deliberately skipped — nothing to decode yet,
    /// or the queue was full). Either way, wait a full interval before asking again.
    pub fn requested(&mut self, audio_ms: u64, submitted: bool) {
        self.last_requested_ms = Some(audio_ms);
        self.in_flight = submitted.then_some(self.epoch);
    }

    /// An interim result arrived. True when it is still current.
    pub fn accept(&mut self) -> bool {
        let current = self.in_flight == Some(self.epoch);
        self.in_flight = None;
        current && self.enabled
    }

    /**
     * WHAT:  Turns an accepted snapshot into a caption and pushes it to the HUD.
     * WHY:   The committed transcript is CLONED and the snapshot's segments pushed
     *        into the clone, so the live transcript is never touched by a
     *        speculative decode, and the assembler's own ordering and
     *        de-duplication do the stitching. An enhancement failure falls back
     *        to the heard text: a caption is a courtesy, never a reason to show
     *        nothing.
     */
    pub fn show(
        &mut self,
        ctx: &SessionContext,
        settings: &SessionSettings,
        committed: &Assembler,
        snapshot: &[TranscriptSegment],
        language: Option<LanguageCode>,
    ) {
        let mut preview = committed.clone();
        preview.push_segments(snapshot);
        let hypothesis = preview.finish();
        if hypothesis.trim().is_empty() {
            return;
        }

        // Remembered whether or not anything is shown below: agreement is between
        // CONSECUTIVE snapshots, so a skipped one must still count as a previous.
        let heard = withhold_unconfirmed(&hypothesis, &self.previous_words).to_string();
        self.previous_words = normalised_words(&hypothesis);
        if heard.trim().is_empty() {
            return;
        }

        // The language the words themselves carry wins, exactly as it does for
        // the delivered text: until the first real chunk has been decoded the
        // session has no pinned language, and English-only rules (fillers, say)
        // would otherwise be skipped for every caption of the first sentence.
        let language = preview.language().map(|code| LanguageCode(code.to_string())).or(language);
        let context = enhance_context(settings, language, self.dictionary.clone());
        let fixed = ctx.ports.enhancer.enhance(&heard, &context).unwrap_or_else(|_| heard.clone());

        let shown = (heard, fixed);
        if self.last_shown.as_ref() == Some(&shown) {
            return;
        }
        ctx.ports
            .events
            .caption_changed(Some(&Caption::Live { heard: shown.0.clone(), fixed: shown.1.clone() }));
        self.last_shown = Some(shown);
    }
}

/// Letters, digits and apostrophes only, lowercased: "Hello," and "hello" agree.
fn normalise_word(word: &str) -> String {
    word.chars()
        .filter(|c| c.is_alphanumeric() || *c == '\'')
        .collect::<String>()
        .to_lowercase()
}

fn normalised_words(text: &str) -> Vec<String> {
    text.split_whitespace().map(normalise_word).collect()
}

/**
 * SOURCE OF TRUTH KEYWORDS: withhold_unconfirmed, stable_caption, local_agreement
 * WHAT:  `text` without its final word, unless the previous snapshot had the same
 *        word in the same place.
 * WHY:   Two snapshots agreeing on a word is the cheapest evidence it is finished
 *        rather than cut off. Only the LAST word is ever held: everything before
 *        it has had a later word spoken after it, which is what makes it settled,
 *        so a caption never lags by more than one word. Positions are compared,
 *        not just words, so a word that has moved is not mistaken for a match.
 * WHERE: LiveCaptions::show.
 */
fn withhold_unconfirmed<'a>(text: &'a str, previous: &[String]) -> &'a str {
    let mut words: Vec<(usize, &str)> = Vec::new();
    let mut start = None;
    for (i, ch) in text.char_indices() {
        if ch.is_whitespace() {
            if let Some(s) = start.take() {
                words.push((s, &text[s..i]));
            }
        } else if start.is_none() {
            start = Some(i);
        }
    }
    if let Some(s) = start {
        words.push((s, &text[s..]));
    }

    let Some(&(last_start, last)) = words.last() else {
        return text;
    };
    let settled = previous.get(words.len() - 1).is_some_and(|word| *word == normalise_word(last));
    if settled {
        text
    } else {
        text[..last_start].trim_end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on() -> LiveCaptions {
        let mut captions = LiveCaptions::new();
        captions.begin(true, vec![]);
        captions
    }

    #[test]
    fn it_does_nothing_unless_it_was_asked_to() {
        let captions = LiveCaptions::new();
        assert!(!captions.due(0), "captions are opt-in");
        assert!(!captions.enabled());
    }

    #[test]
    fn a_snapshot_is_due_at_once_then_only_after_a_full_interval_of_audio() {
        let mut captions = on();
        assert!(captions.due(0), "the first one is not delayed");

        captions.requested(0, false);
        assert!(!captions.due(100), "too soon");
        assert!(captions.due(INTERVAL_MS), "a full interval of audio later");
    }

    #[test]
    fn only_one_snapshot_is_ever_in_flight() {
        let mut captions = on();
        captions.requested(0, true);
        assert!(!captions.due(INTERVAL_MS * 5), "the answer has not come back, however long it takes");

        assert!(captions.accept(), "the answer is current");
        assert!(captions.due(INTERVAL_MS * 5), "and now the next one can be asked for");
    }

    #[test]
    fn an_answer_for_a_chunk_that_has_since_closed_is_dropped() {
        let mut captions = on();
        captions.requested(0, true);

        // The chunk the snapshot was taken from closes into the transcript.
        captions.chunk_closed();

        assert!(!captions.accept(), "showing it would print that chunk's words twice");
    }

    #[test]
    fn an_answer_nobody_asked_for_is_not_current() {
        let mut captions = on();
        assert!(!captions.accept());
    }

    fn words(text: &str) -> Vec<String> {
        normalised_words(text)
    }

    #[test]
    fn a_final_word_nobody_has_confirmed_yet_is_held_back() {
        assert_eq!(withhold_unconfirmed("we should ship the transcri", &[]), "we should ship the");
        assert_eq!(withhold_unconfirmed("hello", &[]), "", "a single unconfirmed word shows nothing yet");
    }

    #[test]
    fn a_final_word_the_previous_snapshot_agreed_on_is_shown() {
        let previous = words("We should ship the transcript");
        assert_eq!(
            withhold_unconfirmed("We should ship the transcript today", &previous),
            "We should ship the transcript",
            "'today' is new; 'transcript' was confirmed by the later word it now has before it"
        );
        assert_eq!(
            withhold_unconfirmed("we should ship the transcript", &previous),
            "we should ship the transcript",
            "an identical hypothesis is settled, punctuation and case aside"
        );
    }

    #[test]
    fn a_word_that_changed_is_not_mistaken_for_a_match() {
        let previous = words("we should ship the transcript");
        assert_eq!(withhold_unconfirmed("we should ship the transcribes", &previous), "we should ship the");
    }

    #[test]
    fn a_word_in_a_different_place_is_not_a_match() {
        // "ship" is in the previous hypothesis, but at position 2, not 4.
        let previous = words("we should ship the thing");
        assert_eq!(withhold_unconfirmed("we should ship the ship", &previous), "we should ship the");
    }

    #[test]
    fn nothing_to_hold_back_in_nothing() {
        assert_eq!(withhold_unconfirmed("", &[]), "");
        assert_eq!(withhold_unconfirmed("   ", &[]), "   ");
    }

    #[test]
    fn beginning_a_new_recording_forgets_the_last_one() {
        let mut captions = on();
        captions.requested(0, true);
        captions.chunk_closed();
        captions.begin(true, vec![]);
        assert!(captions.due(0));
        assert!(!captions.accept(), "nothing is in flight in the new session");
    }
}
