/**
 * SOURCE OF TRUTH KEYWORDS: BillingView, CommunityView, community-route, open-source, support
 * WHAT:  The Community & Support view. Vocs is 100% free and open source.
 * WHY:   There is no billing, no paywall, and no subscription. This page
 *        celebrates the open-source spirit, reassures users of their privacy,
 *        and gives direct links to support development via GitHub Sponsors,
 *        Buy Me a Coffee, or giving the repo a star.
 *        Built from the same numbered cards as every other page: the statement is
 *        the one hero card (it alone carries the dot lattice), the ways to help
 *        are four plain cards that are links, and the thank-you is a line of
 *        text rather than a card of its own.
 * WHERE: Routed from the capability registry's Community nav entry (route "billing").
 */

import { Card, ScrollArea } from "@/components/global";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  ExternalLink,
  Heart,
  Star,
  Coffee,
  Code2,
} from "lucide-react";

interface SupportLink {
  title: string;
  description: string;
  icon: typeof Heart;
  url: string;
  badge?: string;
}

const SUPPORT_LINKS: SupportLink[] = [
  {
    title: "Sponsor on GitHub",
    description:
      "Support ongoing development directly through GitHub Sponsors.",
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
    title: "Star the Repository",
    description:
      "Help more people find Vocs by starring the project on GitHub.",
    icon: Star,
    url: "https://github.com/HackX-IN/Vocs",
    badge: "Star",
  },
  {
    title: "Contribute & Issues",
    description: "Report bugs, propose features, or contribute code on GitHub.",
    icon: Code2,
    url: "https://github.com/HackX-IN/Vocs/issues",
    badge: "Code",
  },
];

export function BillingView() {
  const handleOpenLink = (url: string) => {
    void openUrl(url);
  };

  return (
    <ScrollArea contentClassName="flex flex-col gap-4 px-[var(--page-padding-x)] pb-12">
      <Card index="01" title="Free. Local. Forever." tone="hero">
        <div className="flex flex-col gap-3">
          <p className="font-display text-title text-text-primary">Yours.</p>
          <p className="max-w-xl text-body text-text-secondary">
            Vocs is an open-source, local-first dictation tool for macOS. No accounts, no subscriptions, no telemetry,
            and no audio ever leaves your Mac.
          </p>
        </div>
      </Card>

      <Card index="02" title="Built for the community">
        <p className="max-w-2xl text-body leading-relaxed text-text-secondary">
          Dictation handles your most confidential thoughts, code, and messages. Paid cloud alternatives charge monthly
          fees and process your voice on remote servers. Vocs proves that local Apple Silicon hardware is more than fast
          enough to deliver state-of-the-art accuracy with zero compromises on privacy.
        </p>
      </Card>

      <div className="grid grid-cols-2 gap-4">
        {SUPPORT_LINKS.map((link, position) => {
          const Icon = link.icon;
          return (
            <button
              key={link.title}
              type="button"
              onClick={() => handleOpenLink(link.url)}
              aria-label={`${link.title} — opens in your browser`}
              className="group text-left"
            >
              <Card
                index={String(position + 3).padStart(2, "0")}
                title={link.badge ?? "Support"}
                className="h-full transition-colors group-hover:bg-sunken-strong"
              >
                <div className="flex flex-col gap-2">
                  <div className="flex items-center justify-between gap-2">
                    <span className="flex items-center gap-2 text-body font-medium text-text-primary">
                      <Icon aria-hidden="true" className="size-4 text-accent" />
                      {link.title}
                    </span>
                    <ExternalLink
                      aria-hidden="true"
                      className="size-3.5 text-text-tertiary transition-colors group-hover:text-text-primary"
                    />
                  </div>
                  <p className="text-caption text-text-secondary">{link.description}</p>
                </div>
              </Card>
            </button>
          );
        })}
      </div>

      <p className="pt-2 text-center text-caption text-text-tertiary">
        Thank you for being part of the Vocs community.
      </p>
    </ScrollArea>
  );
}
