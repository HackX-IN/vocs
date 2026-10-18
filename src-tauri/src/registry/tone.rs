/*!
 * SOURCE OF TRUTH KEYWORDS: suggested_tone, tone_presets, app_tone_table, bundle_id_prefix,
 *   per_app_tone
 * WHAT:  Which writing style (`Tone`) is the obvious choice for an app, by bundle
 *        id: casual for chat apps, formal for mail and word processors, literal
 *        for terminals and code editors.
 * WHY:   Tone presets are not a separate feature — a preset is a per-app
 *        profile that overrides `enhance.tone` — so the only thing the app
 *        has to KNOW is which apps deserve which preset. That is knowledge about
 *        the world, not about the user, and it belongs in the registry, the
 *        single place that is allowed to say what the app has; a table in the UI
 *        would be a second opinion the next release forgets to update.
 *
 *        It is a SUGGESTION: it is offered when a profile is created and never
 *        applied behind the user's back, and an app the table does not know
 *        returns Natural, which means "no opinion". Matching is by exact bundle
 *        id or by a prefix for families that share one (every JetBrains IDE is
 *        `com.jetbrains.*`), and case-insensitive because bundle ids are written
 *        both ways in the wild.
 * WHERE: ipc/commands/profiles.rs::suggest_tone, called when a per-app profile
 *        is created.
 */

use crate::types::Tone;

/// Exact bundle ids or prefixes (ending in `.`) → the tone they suggest.
const TABLE: &[(&str, Tone)] = &[
    // Chat: short, quick, no full stops.
    ("com.tinyspeck.slackmacgap", Tone::Casual),
    ("com.apple.mobilesms", Tone::Casual),
    ("net.whatsapp.whatsapp", Tone::Casual),
    ("ru.keepcoder.telegram", Tone::Casual),
    ("com.hnc.discord", Tone::Casual),
    ("com.microsoft.teams", Tone::Casual),
    ("com.microsoft.teams2", Tone::Casual),
    ("com.facebook.archon", Tone::Casual),
    // Mail and documents: complete sentences, written out.
    ("com.readdle.smartemail-mac.", Tone::Formal),
    ("com.apple.mail", Tone::Formal),
    ("com.microsoft.outlook", Tone::Formal),
    ("com.microsoft.word", Tone::Formal),
    ("com.apple.iwork.pages", Tone::Formal),
    // Terminals and editors: what was heard, untouched.
    ("com.apple.terminal", Tone::Literal),
    ("com.googlecode.iterm2", Tone::Literal),
    ("dev.warp.warp-stable", Tone::Literal),
    ("com.mitchellh.ghostty", Tone::Literal),
    ("net.kovidgoyal.kitty", Tone::Literal),
    ("com.microsoft.vscode", Tone::Literal),
    ("com.apple.dt.xcode", Tone::Literal),
    ("dev.zed.zed", Tone::Literal),
    ("com.sublimetext.", Tone::Literal),
    ("com.jetbrains.", Tone::Literal),
];

/// The tone suggested for `bundle_id`, or `Natural` when the table has no
/// opinion about that app.
pub fn suggested_tone(bundle_id: &str) -> Tone {
    let id = bundle_id.trim().to_ascii_lowercase();
    TABLE
        .iter()
        .find(|(known, _)| if known.ends_with('.') { id.starts_with(known) } else { id == *known })
        .map_or(Tone::Natural, |(_, tone)| *tone)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_apps_suggest_casual_mail_suggests_formal_and_terminals_suggest_literal() {
        assert_eq!(suggested_tone("com.tinyspeck.slackmacgap"), Tone::Casual);
        assert_eq!(suggested_tone("com.apple.mail"), Tone::Formal);
        assert_eq!(suggested_tone("com.apple.Terminal"), Tone::Literal);
    }

    #[test]
    fn a_family_of_apps_is_matched_by_its_shared_prefix() {
        assert_eq!(suggested_tone("com.jetbrains.intellij"), Tone::Literal);
        assert_eq!(suggested_tone("com.jetbrains.WebStorm"), Tone::Literal);
        assert_eq!(suggested_tone("com.sublimetext.4"), Tone::Literal);
    }

    #[test]
    fn matching_ignores_case_because_bundle_ids_are_written_both_ways() {
        assert_eq!(suggested_tone("COM.APPLE.MAIL"), Tone::Formal);
        assert_eq!(suggested_tone("  com.apple.mail  "), Tone::Formal);
    }

    #[test]
    fn an_unknown_app_gets_no_opinion_rather_than_a_guess() {
        assert_eq!(suggested_tone("com.example.unknown"), Tone::Natural);
        // A prefix entry must not match an id that merely starts the same way
        // without the dot boundary.
        assert_eq!(suggested_tone("com.jetbrainsfake.app"), Tone::Natural);
        assert_eq!(suggested_tone(""), Tone::Natural);
    }
}
