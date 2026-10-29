/*!
 * SOURCE OF TRUTH KEYWORDS: apply_voice_rollback, scratch_that, undo_last_sentence,
 *   RollbackScope, ROLLBACK_COMMANDS, voice_rollback, strip_last_clause
 * WHAT:  Applies the spoken commands that take words back — "scratch that",
 *        "undo last sentence", "delete everything" — by removing what was said
 *        before the command, and the command itself, from the transcript.
 * WHY:   Someone who rambles, misspeaks or changes their mind used to have to
 *        stop, reach for the keyboard, delete, and start again. The transcript is
 *        still text when this runs, so rolling it back costs nothing and nothing
 *        wrong ever reaches the document.
 *
 *        Every command must END A CLAUSE — be the last word, or carry a comma,
 *        stop or the like. That is what keeps "I need to scratch that itch" and
 *        "you can undo that change" intact: the command word is followed by more
 *        of the same sentence. Whisper punctuates around a deliberate pause, and
 *        a command is a deliberate pause. The commands that are ordinary phrases
 *        ("delete that", "undo that", "forget that") must also START a clause, so
 *        they only fire when they are the whole utterance; "scratch that" and the
 *        spelt-out "last sentence" forms are not things people say by accident,
 *        so they may follow words in the same clause ("ship Friday, scratch
 *        that").
 *
 *        What goes is the CLAUSE being spoken, back to the previous sentence stop;
 *        or, when the command stands alone after a finished sentence, that
 *        sentence. Initialisms ("p.m.") and titles ("Dr.") do not end a sentence,
 *        by the same rule the casing pass uses. English only, like every other
 *        spoken command: matching English phrases in another language would
 *        delete words that merely sound alike.
 * WHERE: Step 2a of adapters/rules/mod.rs, straight after spoken-command
 *        expansion (so "full stop" has become a real stop) and gated by the same
 *        setting. A transcript that is nothing but a rollback comes back empty,
 *        and session/delivery.rs treats an empty result as a dictation that
 *        pasted nothing.
 */

use crate::types::LanguageCode;

use super::sentences::ends_abbreviation;
use super::text::{bare, ends_clause, is_english, words_with_offsets, Word};

/// How far back a command reaches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RollbackScope {
    /// The clause being spoken, or the sentence just finished.
    Clause,
    /// Everything said so far.
    Everything,
}

/// A spoken command: its words, how far it reaches, and whether it may sit
/// inside a clause (`false`) or must be the whole utterance (`true`).
struct Command {
    words: &'static [&'static str],
    scope: RollbackScope,
    standalone_only: bool,
}

const fn command(words: &'static [&'static str], scope: RollbackScope, standalone_only: bool) -> Command {
    Command { words, scope, standalone_only }
}

/**
 * SOURCE OF TRUTH KEYWORDS: ROLLBACK_COMMANDS
 * WHAT:  Every phrase that rolls the transcript back.
 * WHY:   Longer phrases first within a family so "scratch the last sentence" is
 *        not read as "scratch" plus leftovers. The standalone-only ones are
 *        ordinary English; see the module WHY.
 */
const ROLLBACK_COMMANDS: &[Command] = &[
    command(&["scratch", "everything"], RollbackScope::Everything, false),
    command(&["delete", "everything"], RollbackScope::Everything, true),
    command(&["clear", "everything"], RollbackScope::Everything, true),
    command(&["scratch", "the", "last", "sentence"], RollbackScope::Clause, false),
    command(&["scratch", "last", "sentence"], RollbackScope::Clause, false),
    command(&["undo", "the", "last", "sentence"], RollbackScope::Clause, false),
    command(&["undo", "last", "sentence"], RollbackScope::Clause, false),
    command(&["delete", "the", "last", "sentence"], RollbackScope::Clause, false),
    command(&["delete", "last", "sentence"], RollbackScope::Clause, false),
    command(&["remove", "the", "last", "sentence"], RollbackScope::Clause, false),
    command(&["remove", "last", "sentence"], RollbackScope::Clause, false),
    command(&["scratch", "that"], RollbackScope::Clause, false),
    command(&["strike", "that"], RollbackScope::Clause, true),
    command(&["delete", "that"], RollbackScope::Clause, true),
    command(&["undo", "that"], RollbackScope::Clause, true),
    command(&["forget", "that"], RollbackScope::Clause, true),
];

/// The first word of every command. A transcript containing none of them cannot
/// hold a command, which lets the common case skip tokenising altogether.
const LEAD_WORDS: &[&str] = &["scratch", "undo", "delete", "remove", "clear", "strike", "forget"];

/// Chained rollbacks in one transcript ("scratch that, scratch that"). A cap so a
/// pathological input cannot loop; far above anything a person says.
const MAX_ROLLBACKS: usize = 16;

/**
 * WHAT:  `text` with every rollback command applied, left to right.
 * WHY:   Byte-identical when no command is found, for the reason given on
 *        apply_spoken_corrections: a rule that rewrites text it has no business
 *        touching is worse than no rule.
 */
pub fn apply_voice_rollback(text: &str, language: Option<&LanguageCode>) -> String {
    if !is_english(language) {
        return text.to_string();
    }

    // Every dictation passes through here and almost none contains a command, so
    // the cheap test comes first: one lowercase copy and seven substring scans
    // beat a String per word. ASCII-only lowercasing keeps it length-preserving.
    let lowered = text.to_ascii_lowercase();
    if !LEAD_WORDS.iter().any(|lead| lowered.contains(lead)) {
        return text.to_string();
    }

    let mut current = text.to_string();
    for _ in 0..MAX_ROLLBACKS {
        match roll_back_once(&current) {
            Some(next) => current = next,
            None => break,
        }
    }
    current
}

/// Resolves the first command in `text`, or None when there is none.
fn roll_back_once(text: &str) -> Option<String> {
    let words = words_with_offsets(text);
    let bares: Vec<String> = words.iter().map(|w| bare(w.text)).collect();

    for index in 0..bares.len() {
        let Some(found) = ROLLBACK_COMMANDS.iter().find(|cmd| matches_at(cmd, &bares, index)) else {
            continue;
        };
        let mut end = index + found.words.len();
        // "full stop" expands to a detached "." (spacing is tidied much later), so
        // a stop standing alone after the command is the command's own punctuation.
        while end < words.len() && words[end].text.chars().all(|c| !c.is_alphanumeric()) {
            end += 1;
        }

        // Every command has to END its clause; see the module WHY.
        let ends_its_clause = end == words.len() || ends_clause(words[end - 1].text);
        let starts_its_clause = index == 0 || ends_clause(words[index - 1].text);
        if !ends_its_clause || (found.standalone_only && !starts_its_clause) {
            continue;
        }

        let before = &text[..words[index].start];
        let after = words.get(end).map_or("", |word: &Word<'_>| &text[word.start..]);
        let kept = match found.scope {
            RollbackScope::Everything => "",
            RollbackScope::Clause => strip_last_clause(before),
        };
        return Some(join(kept, after));
    }
    None
}

fn matches_at(cmd: &Command, bares: &[String], index: usize) -> bool {
    index + cmd.words.len() <= bares.len()
        && cmd.words.iter().zip(&bares[index..]).all(|(want, got)| *want == got.as_str())
}

/**
 * WHAT:  `before` without its last clause — or, when it ends in a finished
 *        sentence, without that sentence.
 * WHY:   "Meet at noon. Actually at one," + scratch that → "Meet at noon." (the
 *        unfinished clause goes), while "Meet at noon." + scratch that → "" (the
 *        command stands alone, so it takes the sentence it follows).
 */
fn strip_last_clause(before: &str) -> &str {
    let trimmed = before.trim_end();
    let finished = trimmed.ends_with(['.', '!', '?']) && !ends_abbreviation(trimmed);

    // A finished sentence's own stop must not count as the boundary before it.
    let search = if finished { &trimmed[..trimmed.len() - 1] } else { trimmed };
    match last_boundary(search) {
        // Spaces only: a newline boundary keeps its line break, or the line
        // before it would run into whatever comes after.
        Some(end) => trimmed[..end].trim_end_matches(' '),
        None => "",
    }
}

/// Byte offset just after the last sentence boundary in `text`: a stop followed
/// by whitespace (not an abbreviation's), or a newline.
fn last_boundary(text: &str) -> Option<usize> {
    let mut found = None;
    let mut chars = text.char_indices().peekable();
    while let Some((i, ch)) = chars.next() {
        let end = i + ch.len_utf8();
        let boundary = match ch {
            '\n' => true,
            '.' | '!' | '?' => {
                chars.peek().is_some_and(|(_, next)| next.is_whitespace())
                    && !(ch == '.' && ends_abbreviation(&text[..end]))
            }
            _ => false,
        };
        if boundary {
            found = Some(end);
        }
    }
    found
}

/// Puts what was kept and what followed back together with one space between,
/// unless the kept text already ends on a line break.
fn join(kept: &str, after: &str) -> String {
    let after = after.trim_start();
    match (kept.is_empty(), after.is_empty()) {
        (true, _) => after.to_string(),
        (_, true) => kept.to_string(),
        _ if kept.ends_with('\n') => format!("{kept}{after}"),
        _ => format!("{kept} {after}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn en() -> Option<LanguageCode> {
        Some(LanguageCode("en".into()))
    }

    fn run(text: &str) -> String {
        apply_voice_rollback(text, en().as_ref())
    }

    #[test]
    fn scratch_that_after_a_finished_sentence_takes_that_sentence() {
        assert_eq!(run("Ship it on Friday. Scratch that. Ship it Monday."), "Ship it Monday.");
    }

    #[test]
    fn scratch_that_inside_a_sentence_takes_the_clause_being_spoken() {
        assert_eq!(
            run("We meet at noon. Actually at one, scratch that. Let's do dinner."),
            "We meet at noon. Let's do dinner."
        );
    }

    #[test]
    fn a_stop_left_detached_by_spoken_command_expansion_still_ends_the_clause() {
        // "full stop" becomes " . " before spacing is tidied.
        assert_eq!(run("ship friday . scratch that . ship monday"), "ship monday");
    }

    #[test]
    fn it_works_at_the_very_start_and_the_very_end() {
        assert_eq!(run("Scratch that. Hello there."), "Hello there.");
        assert_eq!(run("Hello there. Scratch that."), "");
        assert_eq!(run("scratch that"), "");
    }

    #[test]
    fn the_last_sentence_forms_are_commands_too() {
        assert_eq!(run("One. Two. Undo last sentence. Three."), "One. Three.");
        assert_eq!(run("One. Two. Undo the last sentence."), "One.");
        assert_eq!(run("One. Two. Delete the last sentence, three."), "One. three.");
    }

    #[test]
    fn everything_clears_everything_said_so_far() {
        assert_eq!(run("One. Two. Scratch everything. Fresh start."), "Fresh start.");
    }

    #[test]
    fn commands_chain_left_to_right() {
        assert_eq!(run("A. B. C. Scratch that. Scratch that. D."), "A. D.");
    }

    #[test]
    fn a_command_that_continues_its_sentence_is_ordinary_speech() {
        for kept in [
            "I need to scratch that itch.",
            "You can undo that change later.",
            "Please delete that file for me.",
            "We should forget that ever happened.",
            "Do not scratch everything off the list yet.",
        ] {
            assert_eq!(run(kept), kept, "must not be treated as a command");
        }
    }

    #[test]
    fn the_everyday_phrases_only_fire_when_they_are_the_whole_utterance() {
        // Mid-clause they are speech, not commands...
        assert_eq!(run("I will delete that."), "I will delete that.");
        // ...on their own they are commands.
        assert_eq!(run("Ship Friday. Delete that. Ship Monday."), "Ship Monday.");
    }

    #[test]
    fn an_abbreviation_is_not_the_boundary_the_rollback_stops_at() {
        assert_eq!(run("Meet Dr. Rao at 3 p.m. today, scratch that. Fine."), "Fine.");
    }

    #[test]
    fn a_line_break_is_a_boundary() {
        assert_eq!(run("First item\nSecond item, scratch that. Third item"), "First item\nThird item");
    }

    #[test]
    fn other_languages_are_left_alone() {
        let hindi = Some(LanguageCode("hi".into()));
        assert_eq!(apply_voice_rollback("scratch that", hindi.as_ref()), "scratch that");
        assert_eq!(apply_voice_rollback("scratch that", None), "scratch that");
    }

    #[test]
    fn text_without_a_command_is_returned_byte_for_byte() {
        let text = "Hello...  world 3 p.m. e.g. fine";
        assert_eq!(run(text), text);
    }
}
