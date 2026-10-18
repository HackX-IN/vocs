/**
 * SOURCE OF TRUTH KEYWORDS: BillingView, CommunityView, community-route, open-source, support
 * WHAT:  The Community & Support view. Vocs is 100% free and open source.
 * WHY:   There is no billing, no paywall, and no subscription. This page
 *        celebrates the open-source spirit, reassures users of their privacy,
 *        and gives direct links to support development via GitHub Sponsors,
 *        Buy Me a Coffee, or giving the repo a star.
 * WHERE: Routed from the capability registry's Community nav entry (route "billing").
 */

import { GlassPanel, ScrollArea } from "@/components/global";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  ExternalLink,
  Heart,
  Star,
  Coffee,
  Code2,
  MessageSquareCode,
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
    <ScrollArea contentClassName="flex flex-col gap-6 px-[var(--page-padding-x)] pb-12 pt-2 max-w-2xl mx-auto">
      {/* Hero statement */}
      <div className="flex flex-col items-center text-center gap-3 pt-6 pb-2">
        <div className="flex h-12 w-12 items-center justify-center rounded-full bg-surface-sunken border border-border-hairline text-text-primary">
          <Heart className="h-6 w-6" aria-hidden="true" />
        </div>
        <h1 className="text-display text-text-primary tracking-tight">
          Free. Local. Forever.
        </h1>
        <p className="max-w-md text-body text-text-secondary">
          Vocs is an open-source, local-first dictation tool for macOS. No
          accounts, no subscriptions, no telemetry, and no audio ever leaves
          your Mac.
        </p>
      </div>

      {/* Philosophy Card */}
      <GlassPanel
        material="elevated"
        radius="card"
        className="p-5 border border-border-hairline flex flex-col gap-3"
      >
        <div className="flex items-center gap-2">
          <MessageSquareCode className="h-4 w-4 text-text-secondary" />
          <h2 className="text-subhead text-text-primary font-medium">
            Built for the Community
          </h2>
        </div>
        <p className="text-caption text-text-secondary leading-relaxed">
          Dictation handles your most confidential thoughts, code, and messages.
          Paid cloud alternatives charge monthly fees and process your voice on
          remote servers. Vocs proves that local Apple Silicon hardware is more
          than fast enough to deliver state-of-the-art accuracy with zero
          compromises on privacy.
        </p>
      </GlassPanel>

      {/* Support Grid */}
      <div className="flex flex-col gap-2">
        <h2 className="text-label-caps text-text-tertiary px-1">
          Ways to Support
        </h2>
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
          {SUPPORT_LINKS.map((link) => {
            const Icon = link.icon;
            return (
              <button
                key={link.title}
                type="button"
                onClick={() => handleOpenLink(link.url)}
                className="group flex flex-col justify-between text-left p-4 rounded-card border border-border-hairline bg-surface-sunken hover:bg-surface-sunken-strong transition-all duration-150 active:scale-[0.99] cursor-pointer"
              >
                <div className="flex flex-col gap-2">
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-2">
                      <Icon className="h-4 w-4 text-text-primary" />
                      <span className="text-body font-medium text-text-primary">
                        {link.title}
                      </span>
                    </div>
                    <ExternalLink className="h-3.5 w-3.5 text-text-tertiary group-hover:text-text-primary transition-colors" />
                  </div>
                  <p className="text-caption text-text-secondary">
                    {link.description}
                  </p>
                </div>
                {link.badge ? (
                  <div className="pt-3">
                    <span className="inline-block text-[11px] font-mono px-2 py-0.5 rounded-full bg-surface-glass border border-border-hairline text-text-secondary">
                      {link.badge}
                    </span>
                  </div>
                ) : null}
              </button>
            );
          })}
        </div>
      </div>

      {/* Footer signoff */}
      <div className="text-center pt-4">
        <p className="text-caption text-text-tertiary">
          Thank you for being part of the Vocs community. Enjoy your writing{" "}
          <span aria-label="grey heart">🩶</span>
        </p>
      </div>
    </ScrollArea>
  );
}
