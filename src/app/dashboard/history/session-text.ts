/**
 * SOURCE OF TRUTH KEYWORDS: effectiveText, wasCorrected, session-text,
 *   SessionSummary, reverted
 * WHAT:  Which text a history session stands for, and whether the rules changed
 *        it at all.
 * WHY:   Mirrors SessionSummary::effective_text in Rust, which export uses, so
 *        the row, the copy button and an export can never disagree about a
 *        session the user reverted. It is a mirror and not a second source: the
 *        rule is three lines, and it exists here only because a list row must
 *        pick its preview synchronously without a round trip per row. If one
 *        changes, the other must — the Rust test
 *        `choosing_the_original_is_a_reversible_flag…` pins the contract.
 * WHERE: HistoryRow, HistoryView and HistoryDetail.
 */

import type { SessionSummary } from "@/lib/bindings";

export function effectiveText(session: SessionSummary): string | null {
  return session.reverted
    ? (session.raw_text ?? session.final_text)
    : (session.final_text ?? session.raw_text);
}

/** True when the fix differs from what was said in more than spacing. */
export function wasCorrected(session: SessionSummary): boolean {
  const { raw_text: raw, final_text: fixed } = session;
  if (raw === null || fixed === null) return false;
  return raw.split(/\s+/).join(" ").trim() !== fixed.split(/\s+/).join(" ").trim();
}
