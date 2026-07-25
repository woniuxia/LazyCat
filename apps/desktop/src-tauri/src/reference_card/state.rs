use std::collections::{HashMap, HashSet};

pub(crate) const MAX_CARDS: usize = 6;
pub(crate) const MAX_TEXT_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ResolveCard {
    Focus { label: String },
    Create { label: String, ordinal: usize },
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ResolveError {
    message: String,
    pub(crate) recent_label: Option<String>,
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

struct CardState {
    source_hash: String,
    last_used: u64,
}

#[derive(Default)]
pub(crate) struct CardRegistry {
    cards: HashMap<String, CardState>,
    sources: HashMap<String, String>,
    pending: HashMap<String, String>,
    usage: u64,
}

impl CardRegistry {
    pub(crate) fn resolve(&mut self, text: String) -> Result<ResolveCard, ResolveError> {
        if text.trim().is_empty() {
            return Err(ResolveError {
                message: "参考卡内容不能为空".to_string(),
                recent_label: None,
            });
        }
        if text.len() > MAX_TEXT_BYTES {
            return Err(ResolveError {
                message: "参考卡内容不能超过 8 MiB".to_string(),
                recent_label: None,
            });
        }

        let normalized = text.replace("\r\n", "\n");
        let source_hash = blake3::hash(normalized.trim().as_bytes())
            .to_hex()
            .to_string();
        self.usage += 1;
        if let Some(label) = self.sources.get(&source_hash).cloned() {
            if let Some(card) = self.cards.get_mut(&label) {
                card.last_used = self.usage;
            }
            return Ok(ResolveCard::Focus { label });
        }
        if self.cards.len() >= MAX_CARDS {
            let recent_label = self
                .cards
                .iter()
                .max_by_key(|(_, card)| card.last_used)
                .map(|(label, _)| label.clone());
            return Err(ResolveError {
                message: "最多同时打开 6 张参考卡，请先关闭一张".to_string(),
                recent_label,
            });
        }

        let number = (1..=MAX_CARDS)
            .find(|number| !self.cards.contains_key(&format!("reference-card-{number}")))
            .expect("available reference card label");
        let label = format!("reference-card-{number}");
        let ordinal = self.cards.len();
        self.sources.insert(source_hash.clone(), label.clone());
        self.cards.insert(
            label.clone(),
            CardState {
                source_hash,
                last_used: self.usage,
            },
        );
        self.pending.insert(label.clone(), text);
        Ok(ResolveCard::Create { label, ordinal })
    }
    pub(crate) fn take_pending(&mut self, label: &str) -> Option<String> {
        self.pending.remove(label)
    }
    pub(crate) fn remove_label(&mut self, label: &str) {
        if let Some(card) = self.cards.remove(label) {
            self.sources.remove(&card.source_hash);
        }
        self.pending.remove(label);
    }
    pub(crate) fn retain_labels<'a>(&mut self, labels: impl IntoIterator<Item = &'a str>) {
        let retained = labels.into_iter().collect::<HashSet<_>>();
        let removed = self
            .cards
            .keys()
            .filter(|label| !retained.contains(label.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        for label in removed {
            self.remove_label(&label);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CardRegistry, ResolveCard, MAX_CARDS, MAX_TEXT_BYTES};
    fn created(registry: &mut CardRegistry, text: &str) -> (String, usize) {
        match registry.resolve(text.to_string()).expect("create card") {
            ResolveCard::Create { label, ordinal } => (label, ordinal),
            ResolveCard::Focus { .. } => panic!("expected create"),
        }
    }
    #[test]
    fn normalized_source_focuses_and_pending_preserves_original() {
        let mut registry = CardRegistry::default();
        let original = "  第一行\r\n第二行  ";
        let (label, _) = created(&mut registry, original);
        assert_eq!(registry.take_pending(&label).as_deref(), Some(original));
        assert_eq!(registry.take_pending(&label), None);
        assert_eq!(
            registry.resolve("第一行\n第二行".to_string()).unwrap(),
            ResolveCard::Focus { label }
        );
    }
    #[test]
    fn removed_source_can_be_created_again() {
        let mut registry = CardRegistry::default();
        let (label, _) = created(&mut registry, "alpha");
        registry.remove_label(&label);
        assert!(matches!(
            registry.resolve("alpha".to_string()).unwrap(),
            ResolveCard::Create { .. }
        ));
    }
    #[test]
    fn capacity_error_reports_recent_label() {
        let mut registry = CardRegistry::default();
        for index in 1..=MAX_CARDS {
            let (label, ordinal) = created(&mut registry, &format!("card-{index}"));
            assert_eq!(label, format!("reference-card-{index}"));
            assert_eq!(ordinal, index - 1);
        }
        let error = registry.resolve("overflow".to_string()).unwrap_err();
        assert_eq!(error.to_string(), "最多同时打开 6 张参考卡，请先关闭一张");
        assert_eq!(error.recent_label.as_deref(), Some("reference-card-6"));
    }
    #[test]
    fn rejects_blank_and_more_than_eight_mibibytes() {
        let mut registry = CardRegistry::default();
        assert_eq!(
            registry
                .resolve(" \r\n\t".to_string())
                .unwrap_err()
                .to_string(),
            "参考卡内容不能为空"
        );
        assert_eq!(
            registry
                .resolve("a".repeat(MAX_TEXT_BYTES + 1))
                .unwrap_err()
                .to_string(),
            "参考卡内容不能超过 8 MiB"
        );
    }
    #[test]
    fn utf8_limit_uses_bytes_and_accepts_exact_multibyte_boundary() {
        let mut registry = CardRegistry::default();
        let exact = format!("{}aa", "你".repeat((MAX_TEXT_BYTES - 2) / 3));
        assert_eq!(exact.len(), MAX_TEXT_BYTES);
        assert!(matches!(
            registry.resolve(exact),
            Ok(ResolveCard::Create { .. })
        ));
        let mut registry = CardRegistry::default();
        let over = format!("{}aaa", "你".repeat((MAX_TEXT_BYTES - 2) / 3));
        assert!(registry.resolve(over).is_err());
    }
    #[test]
    fn retain_labels_removes_stale_cards_and_sources() {
        let mut registry = CardRegistry::default();
        let (first, _) = created(&mut registry, "first");
        let (second, _) = created(&mut registry, "second");
        registry.retain_labels([second.as_str()]);
        assert!(matches!(
            registry.resolve("first".to_string()),
            Ok(ResolveCard::Create { .. })
        ));
        assert_eq!(
            registry.resolve("second".to_string()).unwrap(),
            ResolveCard::Focus { label: second }
        );
        assert_ne!(first, "");
    }
}
