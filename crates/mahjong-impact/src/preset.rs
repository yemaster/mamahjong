//! 冲击麻将的预设目录。
//!
//! 目前只有一份「标准冲击麻将」，也就是规则书写死的那套默认值。留着这一层是为了
//! 和立直麻将走同一条「预设 + 覆盖」的解析路径，将来加流派时不用改调用方。

use std::num::NonZeroU32;

use mahjong_core::{PresetId, PresetRef};

use crate::config::ImpactRules;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ImpactPreset {
    Standard,
}

impl ImpactPreset {
    pub const ALL: [Self; 1] = [Self::Standard];

    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Standard => "standard",
        }
    }

    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Standard => "标准冲击麻将",
        }
    }

    #[must_use]
    pub const fn short_name(self) -> &'static str {
        match self {
            Self::Standard => "标准",
        }
    }

    #[must_use]
    pub const fn revision(self) -> NonZeroU32 {
        NonZeroU32::MIN
    }

    #[must_use]
    pub fn preset_ref(self) -> PresetRef {
        PresetRef::new(
            PresetId::parse(self.id()).expect("built-in preset IDs are valid"),
            self.revision(),
        )
    }

    #[must_use]
    pub fn find(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|preset| preset.id() == id)
    }

    #[must_use]
    pub fn rules(self) -> ImpactRules {
        let rules = match self {
            Self::Standard => ImpactRules::standard(),
        };
        rules
            .validate()
            .expect("built-in preset configurations are valid");
        rules
    }
}

#[cfg(test)]
mod tests {
    use super::ImpactPreset;

    #[test]
    fn catalog_ids_and_revisions_are_stable() {
        let metadata: Vec<_> = ImpactPreset::ALL
            .into_iter()
            .map(|preset| (preset.id(), preset.revision().get()))
            .collect();

        assert_eq!(metadata, vec![("standard", 1)]);
        assert_eq!(ImpactPreset::find("standard"), Some(ImpactPreset::Standard));
        assert_eq!(ImpactPreset::find("unknown"), None);
    }

    #[test]
    fn standard_revision_one_is_locked() {
        let rules = ImpactPreset::Standard.rules();

        assert!(rules.kan.added_kan_single_payer);
        assert!(rules.kan.indicator_pon_counts_as_kan);
        assert!(rules.kan.first_round_repeat_discard);
        assert!(!rules.special.seven_gaps);
        assert!(rules.all_in.eleven_honor_streak);
        assert!(rules.all_in.blessing);
    }
}
