/*!
 * SOURCE OF TRUTH KEYWORDS: RuleOrganizer, organize, split_units, classify_unit,
 *   action_text, ACTION_LEAD_INS, IMPERATIVE_VERBS, RUN_ON_MARKERS
 * WHAT:  The deterministic TextOrganizer: cleans a ramble, splits it into
 *        thoughts, sorts the thoughts into tasks and notes, and lays them out as
 *        bullets, a checklist or a paragraph.
 * WHY:   Rules, not a model, for the same reason the rest of this directory is
 *        rules: it is instant, it runs offline, it cannot invent a task nobody
 *        said, and it is table-testable. What it cannot do is understand — it
 *        recognises that a sentence is a task from how people phrase tasks
 *        ("I need to", "remember to", a leading verb, "by Friday"), and a task
 *        phrased some other way is filed as a note rather than lost. The
 *        TextOrganizer port is where a language model replaces it.
 *
 *        Nothing is ever dropped except exact repeats and fillers. Every thought
 *        the person said ends up in the output under some heading, so organising
 *        can only ever rearrange their words, never edit away something they
 *        meant. English only for the task cues and fillers: another language's
 *        text is tidied and laid out, but not sorted.
 * WHERE: Built by adapters::build_organizer; called by ipc/commands/scratchpad.rs.
 */

use std::collections::HashSet;

use crate::error::{AppError, AppResult};
use crate::ports::{EnhanceContext, TextEnhancer, TextOrganizer};
use crate::types::{FillerStrength, LanguageCode, OrganizeMode, Tone};

use super::sentences::ends_abbreviation;
use super::text::bare;
use super::RuleEnhancer;

/// Phrases that turn a sentence into a task. Stripped when the task is written
/// out ("I need to email Sam" → "Email Sam"). Longest first, matched at the
/// start of the sentence after any leading discourse word.
const ACTION_LEAD_INS: &[&str] = &[
    "i also need to ", "we also need to ", "i need to ", "we need to ", "you need to ",
    "i have to ", "we have to ", "you have to ", "i've got to ", "i got to ",
    "i should ", "we should ", "you should ", "i must ", "we must ",
    "i'm going to ", "i am going to ", "we're going to ", "we are going to ",
    "i'll ", "i will ", "we'll ", "we will ", "let's ", "let us ",
    "don't forget to ", "do not forget to ", "dont forget to ", "remember to ",
    "make sure to ", "make sure we ", "make sure i ", "make sure you ", "make sure ",
    "i want to ", "need to ", "todo: ", "to do: ", "action item: ", "action item ",
];

/// Verbs that open an instruction. A sentence starting with one is a task even
/// without a lead-in ("Email Sam about the budget").
const IMPERATIVE_VERBS: &[&str] = &[
    "email", "call", "text", "message", "send", "schedule", "book", "buy", "order",
    "fix", "finish", "review", "update", "write", "draft", "check", "ask", "tell",
    "prepare", "ship", "deploy", "create", "add", "remove", "delete", "set", "pick",
    "reply", "respond", "submit", "pay", "cancel", "plan", "read", "test", "follow",
    "remind", "organise", "organize", "clean", "install", "upgrade", "merge", "push",
    "book", "arrange", "confirm", "share", "sign", "renew", "print", "file", "contact",
];

/// Mid-sentence cues that make a sentence a task for someone ("Sam needs to…",
/// "the invoice is due Friday").
const ACTION_CUES: &[&str] = &[
    " needs to ", " need to ", " has to ", " have to ", " should ", " must ",
    " is due ", " are due ", " due by ", " deadline ",
];

/// Words that open a sentence without belonging to the thought.
const LEADING_FILLER: &[&str] = &["so", "and", "also", "okay", "ok", "then", "oh", "well", "right", "now", "plus"];

/// Where a run-on ramble can be cut into separate thoughts. The marker itself is
/// dropped; each side must be a real clause (see `MIN_CLAUSE_WORDS`).
const RUN_ON_MARKERS: &[&str] = &[
    " and then ", ", and also ", " and also ", ", also ", " oh and ", ", plus ",
    " another thing ", ", then ", " and another thing ", " on top of that ",
];

/// A side of a split must have at least this many words, or "and then" inside a
/// short phrase would shred it.
const MIN_CLAUSE_WORDS: usize = 3;

/// Few enough thoughts that a bulleted list would be heavier than the text.
const PARAGRAPH_BELOW: usize = 3;

pub struct RuleOrganizer {
    enhancer: RuleEnhancer,
}

impl RuleOrganizer {
    pub fn new() -> Self {
        Self { enhancer: RuleEnhancer::new() }
    }
}

impl Default for RuleOrganizer {
    fn default() -> Self {
        Self::new()
    }
}

/// A thought, with what was decided about it.
struct Unit {
    text: String,
    /// The task as an instruction, when the thought is one.
    task: Option<String>,
}

impl TextOrganizer for RuleOrganizer {
    fn id(&self) -> &'static str {
        "rules"
    }

    fn organize(&self, text: &str, mode: OrganizeMode, language: Option<&LanguageCode>) -> AppResult<String> {
        if text.trim().is_empty() {
            return Err(AppError::invalid_input("There is nothing here to organise yet."));
        }

        // Unknown language is assumed English: someone on Auto who dictates in
        // English is the common case, and the cues are only used for sorting.
        let english = language.is_none_or(|l| l.as_str().starts_with("en"));
        let tidy = self.tidy(text, language, english)?;
        let units = classify(split_units(&tidy), english);
        if units.is_empty() {
            return Err(AppError::invalid_input("There is nothing here to organise yet."));
        }

        match mode {
            OrganizeMode::Paragraph => Ok(paragraph(&units)),
            OrganizeMode::Bullets => Ok(bullets(units.iter().map(|u| u.text.as_str()))),
            OrganizeMode::Actions => {
                let tasks = tasks(&units);
                if tasks.is_empty() {
                    return Err(AppError::invalid_input("No action items found in that text."));
                }
                Ok(checklist(&tasks))
            }
            OrganizeMode::Auto => Ok(auto(&units)),
        }
    }
}

impl RuleOrganizer {
    /// Fillers out, spacing and capitals repaired, via the same rules a dictation
    /// goes through — so typed text gets what spoken text already had.
    fn tidy(&self, text: &str, language: Option<&LanguageCode>, english: bool) -> AppResult<String> {
        let context = EnhanceContext {
            language: Some(language.cloned().unwrap_or_else(|| LanguageCode("en".into()))),
            dictionary: Vec::new(),
            strip_fillers: english,
            filler_strength: FillerStrength::Standard,
            expand_spoken_commands: false,
            normalise_punctuation: true,
            capitalise_sentences: true,
            apply_corrections: false,
            fix_grammar: false,
            tone: Tone::Natural,
        };
        self.enhancer.enhance(text, &context)
    }
}

/**
 * WHAT:  `text` cut into thoughts: sentences, and the pieces of a run-on.
 * WHY:   A ramble is mostly one long sentence joined by "and then", so splitting
 *        on stops alone would hand back one giant bullet.
 */
fn split_units(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut start = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((i, ch)) = chars.next() {
        let end = i + ch.len_utf8();
        let stops = match ch {
            '\n' => true,
            '.' | '!' | '?' => {
                chars.peek().is_none_or(|(_, next)| next.is_whitespace())
                    && !(ch == '.' && ends_abbreviation(&text[start..end]))
            }
            _ => false,
        };
        if stops {
            push_trimmed(&mut sentences, &text[start..end]);
            start = end;
        }
    }
    push_trimmed(&mut sentences, &text[start..]);

    sentences.into_iter().flat_map(|sentence| split_run_on(&sentence)).collect()
}

fn push_trimmed(out: &mut Vec<String>, piece: &str) {
    let piece = piece.trim();
    if !piece.is_empty() {
        out.push(piece.to_string());
    }
}

/// Cuts a sentence at the first run-on marker that leaves two real clauses, and
/// carries on with the rest.
fn split_run_on(sentence: &str) -> Vec<String> {
    // ASCII lowercasing keeps byte offsets identical to the original.
    let lowered = sentence.to_ascii_lowercase();
    let mut best: Option<(usize, usize)> = None;
    for marker in RUN_ON_MARKERS {
        let mut from = 0;
        while let Some(found) = lowered[from..].find(marker) {
            let at = from + found;
            let left_words = sentence[..at].split_whitespace().count();
            let right_words = sentence[at + marker.len()..].split_whitespace().count();
            if left_words >= MIN_CLAUSE_WORDS && right_words >= MIN_CLAUSE_WORDS {
                if best.is_none_or(|(earliest, _)| at < earliest) {
                    best = Some((at, marker.len()));
                }
                break;
            }
            from = at + marker.len();
        }
    }

    match best {
        Some((at, len)) => {
            let mut parts = vec![sentence[..at].trim().to_string()];
            parts.extend(split_run_on(&sentence[at + len..]));
            parts
        }
        None => vec![sentence.to_string()],
    }
}

/// Sorts each thought into task or note, dropping exact repeats.
fn classify(units: Vec<String>, english: bool) -> Vec<Unit> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut out = Vec::new();
    for raw in units {
        let text = finish(&raw);
        let key: String = text.split_whitespace().map(bare).collect::<Vec<_>>().join(" ");
        if key.is_empty() || !seen.insert(key) {
            continue;
        }
        let task = english.then(|| action_text(&text)).flatten();
        out.push(Unit { text, task });
    }
    out
}

/// A thought as a tidy line: a capital, no dangling comma, no closing stop (a
/// list item is not a sentence).
fn finish(raw: &str) -> String {
    let trimmed = raw
        .trim()
        .trim_start_matches(['-', '•', '*'])
        .trim()
        .trim_end_matches(['.', ',', ';', ':'])
        .trim();
    capitalise(&strip_leading_filler(trimmed))
}

fn strip_leading_filler(text: &str) -> String {
    let mut rest = text;
    while let Some((first, tail)) = rest.split_once(' ') {
        let word = first.trim_end_matches(',').to_ascii_lowercase();
        if !LEADING_FILLER.contains(&word.as_str()) || tail.trim().is_empty() {
            break;
        }
        rest = tail;
    }
    rest.trim().to_string()
}

fn capitalise(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/**
 * SOURCE OF TRUTH KEYWORDS: action_text
 * WHAT:  The task as an instruction when `thought` is one, else None.
 * WHY:   Three shapes, strongest first: a lead-in ("I need to email Sam" →
 *        "Email Sam"), a leading imperative verb ("Email Sam" as it stands), and
 *        a mid-sentence cue ("Sam needs to send the invoice", kept whole because
 *        it names who). A question is never a task on its own.
 */
fn action_text(thought: &str) -> Option<String> {
    let lowered = thought.to_ascii_lowercase();

    for lead in ACTION_LEAD_INS {
        if let Some(rest) = lowered.strip_prefix(lead) {
            let offset = thought.len() - rest.len();
            let task = thought[offset..].trim();
            if !task.is_empty() {
                return Some(capitalise(task));
            }
        }
    }

    if thought.ends_with('?') {
        return None;
    }

    let first = lowered.split_whitespace().next().unwrap_or("");
    if IMPERATIVE_VERBS.contains(&first) {
        return Some(thought.to_string());
    }

    let padded = format!(" {lowered} ");
    ACTION_CUES.iter().any(|cue| padded.contains(cue)).then(|| thought.to_string())
}

fn tasks(units: &[Unit]) -> Vec<&str> {
    units.iter().filter_map(|u| u.task.as_deref()).collect()
}

fn bullets<'a>(items: impl Iterator<Item = &'a str>) -> String {
    items.map(|item| format!("- {item}")).collect::<Vec<_>>().join("\n")
}

fn checklist(items: &[&str]) -> String {
    items.iter().map(|item| format!("- [ ] {item}")).collect::<Vec<_>>().join("\n")
}

/// The thoughts back as running prose: each gets its full stop back.
fn paragraph(units: &[Unit]) -> String {
    units
        .iter()
        .map(|u| {
            if u.text.ends_with(['?', '!']) {
                u.text.clone()
            } else {
                format!("{}.", u.text)
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Tasks and notes when there are tasks, bullets for a longer ramble, a
/// paragraph for a short one.
fn auto(units: &[Unit]) -> String {
    let tasks = tasks(units);
    if !tasks.is_empty() {
        let notes: Vec<&str> = units.iter().filter(|u| u.task.is_none()).map(|u| u.text.as_str()).collect();
        let mut out = format!("Action items:\n{}", checklist(&tasks));
        if !notes.is_empty() {
            out.push_str(&format!("\n\nNotes:\n{}", bullets(notes.into_iter())));
        }
        return out;
    }
    if units.len() >= PARAGRAPH_BELOW {
        return bullets(units.iter().map(|u| u.text.as_str()));
    }
    paragraph(units)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn en() -> Option<LanguageCode> {
        Some(LanguageCode("en".into()))
    }

    fn organise(text: &str, mode: OrganizeMode) -> String {
        RuleOrganizer::new().organize(text, mode, en().as_ref()).expect("organises")
    }

    const RAMBLE: &str = "um so I need to email Sam about the budget and then the launch is next Tuesday \
        and also remember to book the flights. the venue looked great though";

    #[test]
    fn a_ramble_is_split_into_thoughts_at_its_joins() {
        let units = split_units(
            "I need to email Sam about the budget and then the launch is next Tuesday and also remember to book the flights.",
        );
        assert_eq!(
            units,
            [
                "I need to email Sam about the budget",
                "the launch is next Tuesday",
                "remember to book the flights."
            ]
        );
    }

    #[test]
    fn a_short_join_is_not_cut_in_two() {
        assert_eq!(split_units("Salt and then pepper."), ["Salt and then pepper."]);
    }

    #[test]
    fn auto_separates_tasks_from_notes() {
        assert_eq!(
            organise(RAMBLE, OrganizeMode::Auto),
            "Action items:\n\
             - [ ] Email Sam about the budget\n\
             - [ ] Book the flights\n\n\
             Notes:\n\
             - The launch is next Tuesday\n\
             - The venue looked great though"
        );
    }

    #[test]
    fn actions_lists_only_the_tasks() {
        assert_eq!(
            organise(RAMBLE, OrganizeMode::Actions),
            "- [ ] Email Sam about the budget\n- [ ] Book the flights"
        );
    }

    #[test]
    fn actions_says_so_when_there_is_nothing_to_do() {
        let err = RuleOrganizer::new()
            .organize("The weather was lovely. The hotel was fine.", OrganizeMode::Actions, en().as_ref())
            .expect_err("no tasks");
        assert!(err.message.contains("No action items"), "{}", err.message);
    }

    #[test]
    fn bullets_keeps_every_thought_in_order() {
        assert_eq!(
            organise("The venue was great. The food was late. The staff were kind.", OrganizeMode::Bullets),
            "- The venue was great\n- The food was late\n- The staff were kind"
        );
    }

    #[test]
    fn a_paragraph_puts_the_stops_back() {
        assert_eq!(
            organise("the venue was great and also the food was late", OrganizeMode::Paragraph),
            "The venue was great. The food was late."
        );
    }

    #[test]
    fn auto_makes_a_long_ramble_of_notes_into_bullets_and_a_short_one_into_prose() {
        assert_eq!(
            organise("Rain today. Sun tomorrow. Snow on Friday.", OrganizeMode::Auto),
            "- Rain today\n- Sun tomorrow\n- Snow on Friday"
        );
        assert_eq!(organise("Rain today. Sun tomorrow.", OrganizeMode::Auto), "Rain today. Sun tomorrow.");
    }

    #[test]
    fn repeats_are_dropped_but_nothing_else_is() {
        assert_eq!(
            organise("Call Priya. Call Priya! The deck is ready.", OrganizeMode::Bullets),
            "- Call Priya\n- The deck is ready"
        );
    }

    #[test]
    fn lead_ins_become_instructions() {
        for (said, task) in [
            ("I need to renew the passport", "Renew the passport"),
            ("Don't forget to water the plants", "Water the plants"),
            ("Make sure we lock the door", "Lock the door"),
            ("Email the landlord about the lease", "Email the landlord about the lease"),
            ("Sam needs to send the invoice", "Sam needs to send the invoice"),
        ] {
            assert_eq!(action_text(said).as_deref(), Some(task), "{said}");
        }
    }

    #[test]
    fn a_question_or_a_plain_statement_is_not_a_task() {
        for said in ["Should we book flights?", "The meeting went well", "It rained all day"] {
            assert_eq!(action_text(said), None, "{said}");
        }
    }

    #[test]
    fn nothing_to_organise_is_said_plainly() {
        let err = RuleOrganizer::new().organize("   \n ", OrganizeMode::Auto, None).expect_err("empty");
        assert!(err.message.contains("nothing here"), "{}", err.message);
    }

    #[test]
    fn another_language_is_laid_out_but_never_sorted() {
        let hindi = Some(LanguageCode("hi".into()));
        let out = RuleOrganizer::new()
            .organize("I need to email Sam. The venue looked great. Rain today.", OrganizeMode::Auto, hindi.as_ref())
            .expect("organises");
        assert!(!out.contains("Action items"), "English cues must not sort another language: {out}");
    }
}
