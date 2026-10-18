/**
 * SOURCE OF TRUTH KEYWORDS: BillingView, CommunityView, community-route, open-source, support
 * WHAT:  The Community & Support view. Vocs is 100% free and open source.
 * WHY:   There is no billing, no paywall, and no subscription. This page
 *        celebrates the open-source spirit, reassures users of their privacy,
 *        and gives direct links to support development via GitHub Sponsors,
 *        Buy Me a Coffee, or giving the repo a star.
 *        Built from the same numbered cards as every other page: the statement is
 *        the one hero card (it alone carries the dot lattice) with the three
 *        promises as chips under it, the ways to help are rows of ONE card — a
 *        list reads faster than four boxes that each say one sentence — and the
 *        version sits at the foot, because "which build am I on" is the first
 *        thing anyone filing an issue is asked.
 * WHERE: Routed from the capability registry's Community nav entry (route "billing").
 */

import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Code2, Coffee, ExternalLink, Heart, Lock, ShieldCheck, Star, UserX } from "lucide-react";
import { Card, Chip, ScrollArea } from "@/components/global";

interface SupportLink {
  title: string;
  description: string;
  icon: typeof Heart;
  url: string;
  badge: string;
}

const SUPPORT_LINKS: readonly SupportLink[] = [
  {
    title: "Sponsor on GitHub",
    description: "Support ongoing development directly through GitHub Sponsors.",
    icon: Heart,
    url: "https://github.com/HackX-IN",
    badge: "Sponsor",
  },
  {
    title: "Buy Me a Coffee",
    description: "Send a one-time tip to fuel late-night coding sessions.",
    icon: Coffee,
    url: "https://buymeacoffee.com",
    badge: "Donate",
  },
  {
    title: "Star the repository",
    description: "Help more people find Vocs by starring the project on GitHub.",
    icon: Star,
    url: "https://github.com/HackX-IN/vocs",
    badge: "Star",
  },
  {
    title: "Contribute and report issues",
    description: "Report bugs, propose features, or contribute code on GitHub.",
    icon: Code2,
    url: "https://github.com/HackX-IN/vocs/issues",
    badge: "Code",
  },
];

export function BillingView() {
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    void getVersion().then(setVersion, () => setVersion(null));
  }, []);

  return (
    <ScrollArea contentClassName="flex flex-col gap-4 px-[var(--page-padding-x)] pb-12">
      <Card index="01" title="Free. Local. Forever." tone="hero">
        <div className="flex flex-col gap-4">
          <p className="font-display text-title text-text-primary">Yours.</p>
          <p className="max-w-xl text-body text-text-secondary">
            Vocs is an open-source, local-first dictation tool for macOS. Dictation handles your most confidential
            thoughts, code and messages, and paid cloud alternatives process your voice on remote servers. Apple
            Silicon is fast enough to do it on your own Mac, so nothing has to leave it.
          </p>
          <div className="flex flex-wrap gap-2">
            <Chip icon={<Lock className="size-3.5" />}>On-device transcription</Chip>
            <Chip icon={<UserX className="size-3.5" />}>No account</Chip>
            <Chip icon={<ShieldCheck className="size-3.5" />}>No telemetry</Chip>
          </div>
        </div>
      </Card>

      <Card index="02" title="Ways to help" flush>
        <ul className="flex flex-col">
          {SUPPORT_LINKS.map((link) => {
            const Icon = link.icon;
            return (
              <li key={link.title} className="hairline-b last:border-b-0">
                <button
                  type="button"
                  onClick={() => void openUrl(link.url)}
                  aria-label={`${link.title} — opens in your browser`}
                  className="group flex w-full items-center gap-4 px-[var(--card-padding)] py-4 text-left transition-colors hover:bg-sunken"
                >
                  <span className="flex size-10 shrink-0 items-center justify-center rounded-input bg-accent-soft text-accent">
                    <Icon aria-hidden="true" className="size-5" />
                  </span>
                  <span className="flex min-w-0 flex-1 flex-col">
                    <span className="text-body font-medium text-text-primary">{link.title}</span>
                    <span className="text-caption text-text-secondary">{link.description}</span>
                  </span>
                  <span className="hidden font-mono text-eyebrow uppercase text-text-tertiary sm:inline">{link.badge}</span>
                  <ExternalLink
                    aria-hidden="true"
                    className="size-4 shrink-0 text-text-tertiary transition-colors group-hover:text-text-primary"
                  />
                </button>
              </li>
            );
          })}
        </ul>
      </Card>

      <p className="pt-2 text-center text-caption text-text-tertiary">
        Thank you for being part of the Vocs community.{version ? ` Version ${version}.` : ""}
      </p>
    </ScrollArea>
  );
}
