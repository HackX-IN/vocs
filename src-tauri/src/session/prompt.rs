/*!
 * SOURCE OF TRUTH KEYWORDS: build_prompt, recognition_prompt, context_prompt,
 *   PROMPT_JOINER
 * WHAT:  Joins the user's context line and their dictionary terms into the one
 *        comma-separated prompt handed to the engine.
 * WHY:   Order is a contract with adapters/whisper/prompt.rs::fit_prompt, which
 *        keeps the LEADING terms when the ~224-token budget overflows. The
 *        context is something the user typed on purpose, so it leads; the
 *        dictionary follows, already sorted most-recently-used first. Kept out
 *        of the actor because the actor is far past the file-size limit and
 *        this is pure string logic that wants a table test.
 * WHERE: Called by session/actor.rs once per transcribe request.
 */

const PROMPT_JOINER: &str = ", ";

/// None when there is neither context nor terms, so the engine is never handed
/// an empty prompt.
pub fn build_prompt(context: Option<&str>, terms: &[String]) -> Option<String> {
    let parts: Vec<&str> = context
        .into_iter()
        .chain(terms.iter().map(String::as_str))
        .collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(PROMPT_JOINER))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn terms(list: &[&str]) -> Vec<String> {
        list.iter().map(|term| term.to_string()).collect()
    }

    #[test]
    fn nothing_to_say_is_no_prompt() {
        assert_eq!(build_prompt(None, &[]), None);
    }

    #[test]
    fn the_context_leads_and_the_dictionary_follows() {
        assert_eq!(
            build_prompt(Some("Rust, Tauri"), &terms(&["Vocs", "specta"])).as_deref(),
            Some("Rust, Tauri, Vocs, specta")
        );
    }

    #[test]
    fn either_half_works_alone() {
        assert_eq!(build_prompt(Some("Rust"), &[]).as_deref(), Some("Rust"));
        assert_eq!(build_prompt(None, &terms(&["Vocs"])).as_deref(), Some("Vocs"));
    }
}
