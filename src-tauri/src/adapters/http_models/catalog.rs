/*!
 * SOURCE OF TRUTH KEYWORDS: MODEL_CATALOG, catalog, descriptor_for,
 *   DEFAULT_MODEL_ID, FALLBACK_MODEL_ID, HF_BASE_URL, CatalogEntry
 * WHAT:  The static table of every model Vocs offers, with its URL, size and
 *        SHA-256.
 * WHY:   A static table rather than a fetched manifest, because listing models
 *        has to work on a plane. The whole model manager — names, sizes, RAM
 *        warnings, which one is default — is local data plus a hash check of
 *        what is on disk, so the only thing that ever needs the network is the
 *        download itself. The hashes are the upstream Git-LFS object ids read
 *        from Hugging Face's `X-Linked-Etag`; they are what makes a truncated
 *        574MB file a caught error instead of a crash inside inference.
 * WHERE: Read by adapters/http_models/store.rs; surfaced through the ModelStore
 *        port. Assets listed in docs/03-IMPLEMENTATION-NOTES.md §2.7.
 */

use crate::types::{ModelDescriptor, ModelId, ModelKind};

/// Every file below is resolved against this. whisper.cpp's own repository.
pub const HF_BASE_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/";

/// The on-device grammar model's catalog id. Not a whisper model: see ModelKind.
pub const GRAMMAR_MODEL_ID: &str = "t5-grammar";

/**
 * The grammar model's repository, pinned to a commit. `main` can move; the
 * SHA-256 below would then reject the download, and the user would be told their
 * network damaged a file that was never damaged. A revision cannot move.
 */
const GRAMMAR_BASE_URL: &str =
    "https://huggingface.co/AventIQ-AI/T5-small-grammar-correction/resolve/b82fb3ffb7b740d1b20848d6101043438e51b3c9/";

/// The model the app installs unless the user chooses otherwise.
pub const DEFAULT_MODEL_ID: &str = "large-v3-turbo-q5_0";
/// Offered when the default will not fit comfortably in RAM.
pub const FALLBACK_MODEL_ID: &str = "small-q5_1";

/**
 * SOURCE OF TRUTH KEYWORDS: CatalogEntry
 * WHAT:  One row of the table, in the shape a `const` can hold.
 * WHY:   ModelDescriptor owns `String`s so it can cross IPC, which no `const`
 *        can build. This is the same data with `&'static str`, converted once
 *        on read — rather than a lazily-initialised global, which would be a
 *        second source of truth with a lifetime.
 * WHERE: MODEL_CATALOG; converted by CatalogEntry::descriptor.
 */
#[derive(Debug, Clone, Copy)]
pub struct CatalogEntry {
    pub id: &'static str,
    pub display_name: &'static str,
    pub description: &'static str,
    /// Resolved against `base_url`, and the name the file has on disk for kinds
    /// that do not use the whisper `ggml-<id>.bin` convention.
    pub file_name: &'static str,
    pub base_url: &'static str,
    pub kind: ModelKind,
    pub sha256: &'static str,
    pub size_bytes: u64,
    pub approx_ram_mb: u64,
    pub is_default: bool,
}

impl CatalogEntry {
    /**
     * WHAT:  The on-disk name for kinds that do not use whisper's
     *        `ggml-<id>.bin` convention: the catalog id plus the upstream
     *        file's extension.
     * WHY:   The upstream name (`model.safetensors`) is the same for every
     *        repository that ever ships one; the id is what makes it ours.
     * WHERE: adapters/http_models/store.rs for ModelKind::Grammar.
     */
    pub fn local_file_name(&self) -> String {
        let extension = std::path::Path::new(self.file_name)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("bin");
        format!("{}.{extension}", self.id)
    }

    pub fn descriptor(&self) -> ModelDescriptor {
        ModelDescriptor {
            id: ModelId(self.id.to_string()),
            display_name: self.display_name.to_string(),
            description: self.description.to_string(),
            url: format!("{}{}", self.base_url, self.file_name),
            sha256: self.sha256.to_string(),
            size_bytes: self.size_bytes,
            approx_ram_mb: self.approx_ram_mb,
            is_default: self.is_default,
            kind: self.kind,
        }
    }
}

/// The table. Ordered as the model manager should list it: default first.
pub const MODEL_CATALOG: &[CatalogEntry] = &[
    CatalogEntry {
        id: DEFAULT_MODEL_ID,
        display_name: "Large v3 Turbo",
        description: "The default. Full accuracy across all 99 languages, quantised so it stays fast on Apple silicon.",
        file_name: "ggml-large-v3-turbo-q5_0.bin",
        base_url: HF_BASE_URL,
        kind: ModelKind::Speech,
        sha256: "394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2",
        size_bytes: 574_041_195,
        approx_ram_mb: 1_100,
        is_default: true,
    },
    CatalogEntry {
        id: FALLBACK_MODEL_ID,
        display_name: "Small",
        description: "A third of the size and memory. Noticeably less accurate on accents and on languages other than English.",
        file_name: "ggml-small-q5_1.bin",
        base_url: HF_BASE_URL,
        kind: ModelKind::Speech,
        sha256: "ae85e4a935d7a567bd102fe55afc16bb595bdb618e11b2fc7591bc08120411bb",
        size_bytes: 190_085_487,
        approx_ram_mb: 450,
        is_default: false,
    },
    CatalogEntry {
        id: GRAMMAR_MODEL_ID,
        display_name: "Grammar model",
        description: "A small on-device model (T5) that fixes grammar the rules cannot — agreement, tense, wrong word forms. English only, runs in a fraction of a second, and never leaves your Mac.",
        file_name: "model.safetensors",
        base_url: GRAMMAR_BASE_URL,
        kind: ModelKind::Grammar,
        sha256: "b56da1e76567f50b5f6ebe3169c91d03c8c2205970f4ce98631888eeafda4c74",
        size_bytes: 121_028_528,
        // F32 weights after load (the file is F16), plus activations.
        approx_ram_mb: 300,
        is_default: true,
    },
];

/// The catalog entry with this id, if the app offers it.
pub fn descriptor_for(id: &ModelId) -> Option<&'static CatalogEntry> {
    MODEL_CATALOG.iter().find(|entry| entry.id == id.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn there_is_exactly_one_default_per_kind() {
        for kind in [ModelKind::Speech, ModelKind::Grammar] {
            let defaults = MODEL_CATALOG.iter().filter(|e| e.kind == kind && e.is_default).count();
            assert_eq!(defaults, 1, "{kind:?} needs exactly one default");
        }
        assert!(descriptor_for(&ModelId(DEFAULT_MODEL_ID.into())).is_some_and(|e| e.is_default));
    }

    #[test]
    fn the_grammar_model_is_a_pinned_revision_and_never_a_speech_model() {
        let entry = descriptor_for(&ModelId(GRAMMAR_MODEL_ID.into())).expect("listed");
        assert_eq!(entry.kind, ModelKind::Grammar);
        let url = entry.descriptor().url;
        assert!(!url.contains("/resolve/main/"), "a moving branch would fail the hash: {url}");
        assert!(url.ends_with("/model.safetensors"));
        assert_eq!(entry.local_file_name(), "t5-grammar.safetensors");
    }

    #[test]
    fn the_fallback_is_offered_and_is_smaller() {
        let default = descriptor_for(&ModelId(DEFAULT_MODEL_ID.into())).expect("default listed");
        let fallback = descriptor_for(&ModelId(FALLBACK_MODEL_ID.into())).expect("fallback listed");
        assert!(fallback.size_bytes < default.size_bytes);
        assert!(fallback.approx_ram_mb < default.approx_ram_mb);
    }

    #[test]
    fn every_entry_carries_a_full_sha256_and_a_reachable_shaped_url() {
        for entry in MODEL_CATALOG {
            assert_eq!(entry.sha256.len(), 64, "{} sha256 is not 32 bytes", entry.id);
            assert!(
                entry.sha256.chars().all(|c| c.is_ascii_hexdigit()),
                "{} sha256 is not hex",
                entry.id
            );
            assert!(entry.size_bytes > 0);
            let descriptor = entry.descriptor();
            assert!(descriptor.url.starts_with("https://huggingface.co/"));
            assert!(descriptor.url.ends_with(entry.file_name));
        }
    }

    #[test]
    fn ids_are_unique_because_they_name_files_on_disk() {
        let mut ids: Vec<&str> = MODEL_CATALOG.iter().map(|e| e.id).collect();
        ids.sort_unstable();
        let count = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), count);
    }

    #[test]
    fn an_unknown_id_is_not_invented() {
        assert!(descriptor_for(&ModelId("ggml-imaginary".into())).is_none());
    }
}
