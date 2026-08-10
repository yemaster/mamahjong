use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::model::ApiFailure;

#[derive(Clone, Debug, Deserialize)]
pub struct RuleSetCatalog {
    pub rule_sets: Vec<RuleSetOption>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RuleSetOption {
    pub id: String,
    pub display_name: String,
    pub seat_count: u8,
    pub default_config: RiichiConfig,
    pub presets: Vec<PresetOption>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PresetOption {
    pub id: String,
    pub revision: u32,
    pub display_name: String,
    pub config: RiichiConfig,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RiichiConfig {
    pub variant: String,
    pub match_rules: MatchRules,
    pub scoring: ScoringRules,
    pub calls: CallRules,
    pub bonuses: BonusRules,
    pub abortive_draws: AbortiveDrawRules,
    pub settlement: SettlementRules,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MatchRules {
    pub length: String,
    pub initial_points: u32,
    pub return_points: u32,
    pub first_place_required_points: u32,
    pub thinking_time: ThinkingTimeRules,
    pub tobi: bool,
    pub dealer_continuation: String,
    pub agari_yame: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ThinkingTimeRules {
    pub base_seconds: u16,
    pub reserve_seconds: u16,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ScoringRules {
    pub kiriage_mangan: bool,
    pub old_yaku: bool,
    pub yakuman_value: String,
    pub nagashi_mangan: bool,
    pub kazoe_yakuman: bool,
    pub kokushi_ankan_chankan: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CallRules {
    pub kuitan: bool,
    pub kuikae: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BonusRules {
    pub red_fives: RedFives,
    pub ippatsu: bool,
    pub ura_dora: bool,
    pub kan_dora: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RedFives {
    pub man: u8,
    pub pin: u8,
    pub sou: u8,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AbortiveDrawRules {
    pub four_winds: bool,
    pub four_kans: bool,
    pub nine_terminals: bool,
    pub four_riichi: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SettlementRules {
    pub uma: PlacementUma,
    pub noten_payment: u32,
    pub ron_resolution: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PlacementUma {
    Fixed { values: Vec<i16> },
    JpmlA,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RulePage {
    Room,
    Match,
    Scoring,
    Bonuses,
    Settlement,
}

impl RulePage {
    pub const ALL: [Self; 5] = [
        Self::Room,
        Self::Match,
        Self::Scoring,
        Self::Bonuses,
        Self::Settlement,
    ];

    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::Room => "房间",
            Self::Match => "对局",
            Self::Scoring => "和牌",
            Self::Bonuses => "宝牌与流局",
            Self::Settlement => "结算",
        }
    }

    #[must_use]
    pub fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|page| *page == self)
            .expect("rule page is in ALL")
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CreateField {
    RoomName,
    Visibility,
    Variant,
    Preset,
    Length,
    InitialPoints,
    ReturnPoints,
    Tobi,
    DealerContinuation,
    AgariYame,
    Kuikae,
    KiriageMangan,
    Kuitan,
    OldYaku,
    YakumanValue,
    NagashiMangan,
    KazoeYakuman,
    KokushiAnkanChankan,
    RedMan,
    RedPin,
    RedSou,
    Ippatsu,
    UraDora,
    KanDora,
    FourWinds,
    FourKans,
    NineTerminals,
    FourRiichi,
    UmaType,
    UmaValues,
    NotenPayment,
    RonResolution,
}

const ROOM_FIELDS: [CreateField; 4] = [
    CreateField::RoomName,
    CreateField::Visibility,
    CreateField::Variant,
    CreateField::Preset,
];
const MATCH_FIELDS: [CreateField; 7] = [
    CreateField::Length,
    CreateField::InitialPoints,
    CreateField::ReturnPoints,
    CreateField::Tobi,
    CreateField::DealerContinuation,
    CreateField::AgariYame,
    CreateField::Kuikae,
];
const SCORING_FIELDS: [CreateField; 7] = [
    CreateField::Kuitan,
    CreateField::KiriageMangan,
    CreateField::OldYaku,
    CreateField::YakumanValue,
    CreateField::NagashiMangan,
    CreateField::KazoeYakuman,
    CreateField::KokushiAnkanChankan,
];
const BONUS_FIELDS: [CreateField; 10] = [
    CreateField::RedMan,
    CreateField::RedPin,
    CreateField::RedSou,
    CreateField::Ippatsu,
    CreateField::UraDora,
    CreateField::KanDora,
    CreateField::FourWinds,
    CreateField::FourKans,
    CreateField::NineTerminals,
    CreateField::FourRiichi,
];
const SETTLEMENT_FIELDS: [CreateField; 4] = [
    CreateField::UmaType,
    CreateField::UmaValues,
    CreateField::NotenPayment,
    CreateField::RonResolution,
];

#[derive(Clone, Debug)]
pub struct CreateRoomForm {
    pub page: RulePage,
    pub active_field: usize,
    pub name: String,
    pub visibility: String,
    pub catalog: RuleSetCatalog,
    pub rule_set_index: usize,
    pub preset_index: Option<usize>,
    pub base_config: RiichiConfig,
    pub edited_config: RiichiConfig,
    pub initial_points: String,
    pub return_points: String,
    pub noten_payment: String,
    pub uma_values: String,
}

impl CreateRoomForm {
    pub fn new(catalog: RuleSetCatalog) -> Result<Self, ApiFailure> {
        let rule_set_index = catalog
            .rule_sets
            .iter()
            .position(|rule_set| rule_set.id == "riichi/yonma")
            .ok_or_else(|| invalid_input("规则目录缺少四人日麻"))?;
        let base_config = catalog.rule_sets[rule_set_index].default_config.clone();
        Ok(Self {
            page: RulePage::Room,
            active_field: 0,
            name: "日麻房间".to_owned(),
            visibility: "public".to_owned(),
            catalog,
            rule_set_index,
            preset_index: None,
            initial_points: base_config.match_rules.initial_points.to_string(),
            return_points: base_config.match_rules.return_points.to_string(),
            noten_payment: base_config.settlement.noten_payment.to_string(),
            uma_values: format_uma_values(&base_config.settlement.uma),
            edited_config: base_config.clone(),
            base_config,
        })
    }

    #[must_use]
    pub fn page_fields(&self) -> &'static [CreateField] {
        fields_for_page(self.page)
    }

    #[must_use]
    pub fn active(&self) -> CreateField {
        self.page_fields()[self.active_field]
    }

    #[must_use]
    pub fn rule_set(&self) -> &RuleSetOption {
        &self.catalog.rule_sets[self.rule_set_index]
    }

    #[must_use]
    pub fn preset(&self) -> Option<&PresetOption> {
        self.preset_index
            .and_then(|index| self.rule_set().presets.get(index))
    }

    #[must_use]
    pub fn preset_label(&self) -> String {
        let label = self.preset().map_or_else(
            || "普通规则".to_owned(),
            |preset| preset.display_name.clone(),
        );
        if self.is_modified() {
            format!("{label} · 已修改")
        } else {
            label
        }
    }

    #[must_use]
    pub fn is_modified(&self) -> bool {
        self.config_with_text()
            .is_ok_and(|config| config != self.base_config)
    }

    /// Text-field validation message, shown under the field list before submitting.
    #[must_use]
    pub fn validation_message(&self) -> Option<String> {
        self.config_with_text().err().map(|failure| failure.message)
    }

    /// Summary state of the four abortive-draw switches, shown beside the tab title.
    #[must_use]
    pub fn abortive_summary(&self) -> &'static str {
        let draws = &self.edited_config.abortive_draws;
        let sanma = self.rule_set().seat_count == 3;
        let mut available = vec![draws.four_kans, draws.nine_terminals];
        if !sanma {
            available.push(draws.four_winds);
            available.push(draws.four_riichi);
        }
        if available.iter().all(|enabled| *enabled) {
            "全部开启"
        } else if available.iter().all(|enabled| !*enabled) {
            "全部关闭"
        } else {
            "自定义"
        }
    }

    /// Grouped summary of the configuration currently being edited.
    #[must_use]
    pub fn summary(&self) -> Vec<(&'static str, String)> {
        let config = &self.edited_config;
        vec![
            ("预设", self.preset_label()),
            ("人数", self.rule_set().display_name.clone()),
            (
                "长度",
                label_pair(&config.match_rules.length, "hanchan", "东南战", "东风战"),
            ),
            (
                "点数",
                format!("{} / {}", self.initial_points, self.return_points),
            ),
            ("马点", self.field_value(CreateField::UmaType)),
            ("击飞", bool_label(config.match_rules.tobi)),
            (
                "荣和",
                label_pair(
                    &config.settlement.ron_resolution,
                    "head_bump",
                    "头跳",
                    "多家和",
                ),
            ),
            (
                "赤牌",
                format!(
                    "{}",
                    u16::from(config.bonuses.red_fives.man)
                        + u16::from(config.bonuses.red_fives.pin)
                        + u16::from(config.bonuses.red_fives.sou)
                ),
            ),
            ("一发", bool_label(config.bonuses.ippatsu)),
            ("里宝", bool_label(config.bonuses.ura_dora)),
            ("杠宝", bool_label(config.bonuses.kan_dora)),
            ("途中流局", self.abortive_summary().to_owned()),
            ("流局罚点", self.noten_payment.clone()),
        ]
    }

    pub fn next_field(&mut self) {
        let count = self.page_fields().len();
        for _ in 0..count {
            self.active_field = (self.active_field + 1) % count;
            if !self.field_unavailable(self.active()) {
                break;
            }
        }
    }

    pub fn previous_field(&mut self) {
        let count = self.page_fields().len();
        for _ in 0..count {
            self.active_field = (self.active_field + count - 1) % count;
            if !self.field_unavailable(self.active()) {
                break;
            }
        }
    }

    pub fn change_page(&mut self, delta: i8) {
        let count = RulePage::ALL.len();
        let next = if delta.is_negative() {
            (self.page.index() + count - 1) % count
        } else {
            (self.page.index() + 1) % count
        };
        self.page = RulePage::ALL[next];
        self.active_field = 0;
        if self.field_unavailable(self.active()) {
            self.next_field();
        }
    }

    pub fn change_active(&mut self, delta: i8) {
        let field = self.active();
        match field {
            CreateField::Visibility => {
                self.visibility = if self.visibility == "public" {
                    "private".to_owned()
                } else {
                    "public".to_owned()
                };
            }
            CreateField::Variant => self.change_variant(),
            CreateField::Preset => self.change_preset(delta),
            CreateField::Length => cycle_string(
                &mut self.edited_config.match_rules.length,
                &["east_only", "hanchan"],
                delta,
            ),
            CreateField::Tobi => toggle(&mut self.edited_config.match_rules.tobi),
            CreateField::DealerContinuation => cycle_string(
                &mut self.edited_config.match_rules.dealer_continuation,
                &["win_or_tenpai", "win_only"],
                delta,
            ),
            CreateField::AgariYame => {
                toggle(&mut self.edited_config.match_rules.agari_yame);
            }
            CreateField::Kuikae => cycle_string(
                &mut self.edited_config.calls.kuikae,
                &["forbidden", "same_tile_only", "allowed"],
                delta,
            ),
            CreateField::Kuitan => toggle(&mut self.edited_config.calls.kuitan),
            CreateField::KiriageMangan => {
                toggle(&mut self.edited_config.scoring.kiriage_mangan);
            }
            CreateField::OldYaku => toggle(&mut self.edited_config.scoring.old_yaku),
            CreateField::YakumanValue => cycle_string(
                &mut self.edited_config.scoring.yakuman_value,
                &["double_variants_and_stacked", "stacked_only"],
                delta,
            ),
            CreateField::NagashiMangan => {
                toggle(&mut self.edited_config.scoring.nagashi_mangan);
            }
            CreateField::KazoeYakuman => {
                toggle(&mut self.edited_config.scoring.kazoe_yakuman);
            }
            CreateField::KokushiAnkanChankan => {
                toggle(&mut self.edited_config.scoring.kokushi_ankan_chankan);
            }
            CreateField::RedMan => adjust_red(&mut self.edited_config.bonuses.red_fives.man, delta),
            CreateField::RedPin => adjust_red(&mut self.edited_config.bonuses.red_fives.pin, delta),
            CreateField::RedSou => adjust_red(&mut self.edited_config.bonuses.red_fives.sou, delta),
            CreateField::Ippatsu => toggle(&mut self.edited_config.bonuses.ippatsu),
            CreateField::UraDora => toggle(&mut self.edited_config.bonuses.ura_dora),
            CreateField::KanDora => toggle(&mut self.edited_config.bonuses.kan_dora),
            CreateField::FourWinds => {
                toggle(&mut self.edited_config.abortive_draws.four_winds);
            }
            CreateField::FourKans => {
                toggle(&mut self.edited_config.abortive_draws.four_kans);
            }
            CreateField::NineTerminals => {
                toggle(&mut self.edited_config.abortive_draws.nine_terminals);
            }
            CreateField::FourRiichi => {
                toggle(&mut self.edited_config.abortive_draws.four_riichi);
            }
            CreateField::UmaType => self.toggle_uma_type(),
            CreateField::RonResolution => cycle_string(
                &mut self.edited_config.settlement.ron_resolution,
                &["multiple", "head_bump"],
                delta,
            ),
            CreateField::RoomName
            | CreateField::InitialPoints
            | CreateField::ReturnPoints
            | CreateField::UmaValues
            | CreateField::NotenPayment => {}
        }
    }

    pub fn push_character(&mut self, character: char) {
        match self.active() {
            CreateField::RoomName if !character.is_control() => self.name.push(character),
            CreateField::InitialPoints if character.is_ascii_digit() => {
                self.initial_points.push(character);
            }
            CreateField::ReturnPoints if character.is_ascii_digit() => {
                self.return_points.push(character);
            }
            CreateField::NotenPayment if character.is_ascii_digit() => {
                self.noten_payment.push(character);
            }
            CreateField::UmaValues
                if character.is_ascii_digit() || matches!(character, '+' | '-' | ',') =>
            {
                self.uma_values.push(character);
            }
            _ => {}
        }
    }

    pub fn backspace(&mut self) {
        match self.active() {
            CreateField::RoomName => {
                self.name.pop();
            }
            CreateField::InitialPoints => {
                self.initial_points.pop();
            }
            CreateField::ReturnPoints => {
                self.return_points.pop();
            }
            CreateField::NotenPayment => {
                self.noten_payment.pop();
            }
            CreateField::UmaValues => {
                self.uma_values.pop();
            }
            _ => {}
        }
    }

    #[must_use]
    pub fn active_accepts_text(&self) -> bool {
        matches!(
            self.active(),
            CreateField::RoomName
                | CreateField::InitialPoints
                | CreateField::ReturnPoints
                | CreateField::UmaValues
                | CreateField::NotenPayment
        )
    }

    #[must_use]
    pub fn field_unavailable(&self, field: CreateField) -> bool {
        let sanma = self.rule_set().seat_count == 3;
        (sanma
            && matches!(
                field,
                CreateField::RedMan | CreateField::FourWinds | CreateField::FourRiichi
            ))
            || (matches!(field, CreateField::UmaValues)
                && matches!(self.edited_config.settlement.uma, PlacementUma::JpmlA))
    }

    #[must_use]
    pub const fn field_label(field: CreateField) -> &'static str {
        match field {
            CreateField::RoomName => "房间名",
            CreateField::Visibility => "可见性",
            CreateField::Variant => "人数",
            CreateField::Preset => "规则预设",
            CreateField::Length => "对局长度",
            CreateField::InitialPoints => "初始点数",
            CreateField::ReturnPoints => "返点",
            CreateField::Tobi => "击飞",
            CreateField::DealerContinuation => "连庄",
            CreateField::AgariYame => "和了止",
            CreateField::Kuikae => "食替",
            CreateField::KiriageMangan => "切上满贯",
            CreateField::Kuitan => "食断",
            CreateField::OldYaku => "古役",
            CreateField::YakumanValue => "特殊役满",
            CreateField::NagashiMangan => "流局满贯",
            CreateField::KazoeYakuman => "累计役满",
            CreateField::KokushiAnkanChankan => "国士抢暗杠",
            CreateField::RedMan => "赤五万",
            CreateField::RedPin => "赤五筒",
            CreateField::RedSou => "赤五索",
            CreateField::Ippatsu => "一发",
            CreateField::UraDora => "里宝",
            CreateField::KanDora => "杠宝",
            CreateField::FourWinds => "四风连打",
            CreateField::FourKans => "四杠散了",
            CreateField::NineTerminals => "九种九牌",
            CreateField::FourRiichi => "四家立直",
            CreateField::UmaType => "马点类型",
            CreateField::UmaValues => "马点值",
            CreateField::NotenPayment => "流局罚点",
            CreateField::RonResolution => "荣和方式",
        }
    }

    #[must_use]
    pub fn field_value(&self, field: CreateField) -> String {
        if self.field_unavailable(field) {
            return "不可用".to_owned();
        }
        match field {
            CreateField::RoomName => self.name.clone(),
            CreateField::Visibility => label_pair(&self.visibility, "public", "公开", "私有"),
            CreateField::Variant => self.rule_set().display_name.clone(),
            CreateField::Preset => self.preset_label(),
            CreateField::Length => label_pair(
                &self.edited_config.match_rules.length,
                "hanchan",
                "东南战",
                "东风战",
            ),
            CreateField::InitialPoints => self.initial_points.clone(),
            CreateField::ReturnPoints => self.return_points.clone(),
            CreateField::Tobi => bool_label(self.edited_config.match_rules.tobi),
            CreateField::DealerContinuation => label_pair(
                &self.edited_config.match_rules.dealer_continuation,
                "win_or_tenpai",
                "和牌或听牌",
                "仅和牌",
            ),
            CreateField::AgariYame => bool_label(self.edited_config.match_rules.agari_yame),
            CreateField::Kuikae => kuikae_label(&self.edited_config.calls.kuikae).to_owned(),
            CreateField::Kuitan => bool_label(self.edited_config.calls.kuitan),
            CreateField::KiriageMangan => bool_label(self.edited_config.scoring.kiriage_mangan),
            CreateField::OldYaku => bool_label(self.edited_config.scoring.old_yaku),
            CreateField::YakumanValue => label_pair(
                &self.edited_config.scoring.yakuman_value,
                "double_variants_and_stacked",
                "双倍并叠加",
                "仅叠加",
            ),
            CreateField::NagashiMangan => bool_label(self.edited_config.scoring.nagashi_mangan),
            CreateField::KazoeYakuman => bool_label(self.edited_config.scoring.kazoe_yakuman),
            CreateField::KokushiAnkanChankan => {
                bool_label(self.edited_config.scoring.kokushi_ankan_chankan)
            }
            CreateField::RedMan => self.edited_config.bonuses.red_fives.man.to_string(),
            CreateField::RedPin => self.edited_config.bonuses.red_fives.pin.to_string(),
            CreateField::RedSou => self.edited_config.bonuses.red_fives.sou.to_string(),
            CreateField::Ippatsu => bool_label(self.edited_config.bonuses.ippatsu),
            CreateField::UraDora => bool_label(self.edited_config.bonuses.ura_dora),
            CreateField::KanDora => bool_label(self.edited_config.bonuses.kan_dora),
            CreateField::FourWinds => bool_label(self.edited_config.abortive_draws.four_winds),
            CreateField::FourKans => bool_label(self.edited_config.abortive_draws.four_kans),
            CreateField::NineTerminals => {
                bool_label(self.edited_config.abortive_draws.nine_terminals)
            }
            CreateField::FourRiichi => bool_label(self.edited_config.abortive_draws.four_riichi),
            CreateField::UmaType => match self.edited_config.settlement.uma {
                PlacementUma::Fixed { .. } => "固定".to_owned(),
                PlacementUma::JpmlA => "联盟 A 浮动".to_owned(),
            },
            CreateField::UmaValues => self.uma_values.clone(),
            CreateField::NotenPayment => self.noten_payment.clone(),
            CreateField::RonResolution => label_pair(
                &self.edited_config.settlement.ron_resolution,
                "head_bump",
                "头跳",
                "多家和",
            ),
        }
    }

    pub fn create_payload(&self) -> Result<Value, ApiFailure> {
        let edited = self.config_with_text()?;
        let overrides = config_diff(&self.base_config, &edited)?;
        let preset = self.preset().map(|preset| {
            json!({
                "id": preset.id,
                "revision": preset.revision,
            })
        });
        let mut config = Map::new();
        if let Some(preset) = preset {
            config.insert("preset".to_owned(), preset);
        }
        config.insert("overrides".to_owned(), overrides);

        Ok(json!({
            "name": self.name,
            "visibility": self.visibility,
            "rules": {
                "rule_set_id": self.rule_set().id,
                "config": config,
            }
        }))
    }

    fn change_variant(&mut self) {
        let next_id = if self.rule_set().id == "riichi/yonma" {
            "riichi/sanma"
        } else {
            "riichi/yonma"
        };
        if let Some(index) = self
            .catalog
            .rule_sets
            .iter()
            .position(|rule_set| rule_set.id == next_id)
        {
            self.rule_set_index = index;
            self.preset_index = None;
            let config = self.rule_set().default_config.clone();
            self.load_base(config);
        }
    }

    fn change_preset(&mut self, delta: i8) {
        let option_count = self.rule_set().presets.len() + 1;
        let current = self.preset_index.map_or(0, |index| index + 1);
        let next = if delta.is_negative() {
            (current + option_count - 1) % option_count
        } else {
            (current + 1) % option_count
        };
        self.preset_index = next.checked_sub(1);
        let config = self.preset().map_or_else(
            || self.rule_set().default_config.clone(),
            |preset| preset.config.clone(),
        );
        self.load_base(config);
    }

    fn load_base(&mut self, config: RiichiConfig) {
        self.initial_points = config.match_rules.initial_points.to_string();
        self.return_points = config.match_rules.return_points.to_string();
        self.noten_payment = config.settlement.noten_payment.to_string();
        self.uma_values = match &config.settlement.uma {
            PlacementUma::Fixed { .. } => format_uma_values(&config.settlement.uma),
            PlacementUma::JpmlA => "+30,+10,-10,-30".to_owned(),
        };
        self.base_config = config.clone();
        self.edited_config = config;
    }

    fn toggle_uma_type(&mut self) {
        self.edited_config.settlement.uma = match self.edited_config.settlement.uma {
            PlacementUma::Fixed { .. } => {
                if self.rule_set().seat_count == 4 {
                    PlacementUma::JpmlA
                } else {
                    return;
                }
            }
            PlacementUma::JpmlA => PlacementUma::Fixed {
                values: parse_uma_values(&self.uma_values)
                    .unwrap_or_else(|_| vec![30, 10, -10, -30]),
            },
        };
    }

    fn config_with_text(&self) -> Result<RiichiConfig, ApiFailure> {
        let mut config = self.edited_config.clone();
        config.match_rules.initial_points = parse_u32(&self.initial_points, "初始点数")?;
        config.match_rules.return_points = parse_u32(&self.return_points, "返点")?;
        config.settlement.noten_payment = parse_u32(&self.noten_payment, "流局罚点")?;
        if matches!(config.settlement.uma, PlacementUma::Fixed { .. }) {
            let values = parse_uma_values(&self.uma_values)?;
            let expected = usize::from(self.rule_set().seat_count);
            if values.len() != expected {
                return Err(invalid_input(if expected == 4 {
                    "四麻马点必须填写四项"
                } else {
                    "三麻马点必须填写三项"
                }));
            }
            if values.iter().map(|value| i32::from(*value)).sum::<i32>() != 0 {
                return Err(invalid_input("马点合计必须为 0"));
            }
            config.settlement.uma = PlacementUma::Fixed { values };
        }
        Ok(config)
    }
}

#[must_use]
pub const fn fields_for_page(page: RulePage) -> &'static [CreateField] {
    match page {
        RulePage::Room => &ROOM_FIELDS,
        RulePage::Match => &MATCH_FIELDS,
        RulePage::Scoring => &SCORING_FIELDS,
        RulePage::Bonuses => &BONUS_FIELDS,
        RulePage::Settlement => &SETTLEMENT_FIELDS,
    }
}

/// Room page rule summary, grouped the same way as the create form tabs.
/// Reads the server rule snapshot so the room always shows the effective rules.
#[must_use]
pub fn snapshot_summary(snapshot: &Value) -> Vec<(&'static str, Vec<(&'static str, String)>)> {
    let config = &snapshot["config"];
    let flag = |group: &str, key: &str| -> String {
        match config[group][key].as_bool() {
            Some(true) => "有".to_owned(),
            Some(false) => "无".to_owned(),
            None => "—".to_owned(),
        }
    };
    let number = |group: &str, key: &str| -> String {
        config[group][key]
            .as_u64()
            .map_or_else(|| "—".to_owned(), |value| value.to_string())
    };
    let text = |group: &str,
                key: &str,
                first: &str,
                first_label: &'static str,
                second_label: &'static str|
     -> String {
        if config[group][key].as_str() == Some(first) {
            first_label.to_owned()
        } else {
            second_label.to_owned()
        }
    };

    vec![
        (
            "对局",
            vec![
                (
                    "长度",
                    text("match_rules", "length", "hanchan", "东南战", "东风战"),
                ),
                ("初始点数", number("match_rules", "initial_points")),
                ("返点", number("match_rules", "return_points")),
                ("击飞", flag("match_rules", "tobi")),
                (
                    "连庄",
                    text(
                        "match_rules",
                        "dealer_continuation",
                        "win_or_tenpai",
                        "和牌或听牌",
                        "仅和牌",
                    ),
                ),
                ("和了止", flag("match_rules", "agari_yame")),
                (
                    "食替",
                    kuikae_label(config["calls"]["kuikae"].as_str().unwrap_or_default()).to_owned(),
                ),
            ],
        ),
        (
            "和牌",
            vec![
                ("食断", flag("calls", "kuitan")),
                ("切上满贯", flag("scoring", "kiriage_mangan")),
                ("古役", flag("scoring", "old_yaku")),
                (
                    "特殊役满",
                    text(
                        "scoring",
                        "yakuman_value",
                        "double_variants_and_stacked",
                        "双倍并叠加",
                        "仅叠加",
                    ),
                ),
                ("流局满贯", flag("scoring", "nagashi_mangan")),
                ("累计役满", flag("scoring", "kazoe_yakuman")),
                ("国士抢暗杠", flag("scoring", "kokushi_ankan_chankan")),
            ],
        ),
        (
            "宝牌",
            vec![
                (
                    "赤牌",
                    format!(
                        "万{} 筒{} 索{}",
                        number_at(&config["bonuses"]["red_fives"], "man"),
                        number_at(&config["bonuses"]["red_fives"], "pin"),
                        number_at(&config["bonuses"]["red_fives"], "sou"),
                    ),
                ),
                ("一发", flag("bonuses", "ippatsu")),
                ("里宝", flag("bonuses", "ura_dora")),
                ("杠宝", flag("bonuses", "kan_dora")),
            ],
        ),
        (
            "流局",
            vec![
                ("四风连打", flag("abortive_draws", "four_winds")),
                ("四杠散了", flag("abortive_draws", "four_kans")),
                ("九种九牌", flag("abortive_draws", "nine_terminals")),
                ("四家立直", flag("abortive_draws", "four_riichi")),
            ],
        ),
        (
            "结算",
            vec![
                ("马点", snapshot_uma(&config["settlement"]["uma"])),
                ("流局罚点", number("settlement", "noten_payment")),
                (
                    "荣和方式",
                    text(
                        "settlement",
                        "ron_resolution",
                        "head_bump",
                        "头跳",
                        "多家和",
                    ),
                ),
            ],
        ),
    ]
}

fn number_at(value: &Value, key: &str) -> String {
    value[key]
        .as_u64()
        .map_or_else(|| "—".to_owned(), |number| number.to_string())
}

fn snapshot_uma(uma: &Value) -> String {
    if uma["type"].as_str() == Some("jpml_a") {
        return "联盟 A 浮动".to_owned();
    }
    uma["values"].as_array().map_or_else(
        || "—".to_owned(),
        |values| {
            values
                .iter()
                .filter_map(serde_json::Value::as_i64)
                .map(|value| format!("{value:+}"))
                .collect::<Vec<_>>()
                .join(",")
        },
    )
}

fn config_diff(base: &RiichiConfig, edited: &RiichiConfig) -> Result<Value, ApiFailure> {
    let base = serde_json::to_value(base).map_err(invalid_serialization)?;
    let edited = serde_json::to_value(edited).map_err(invalid_serialization)?;
    let mut overrides = Map::new();
    for group in [
        "match_rules",
        "scoring",
        "calls",
        "bonuses",
        "abortive_draws",
        "settlement",
    ] {
        let group_diff = object_diff(&base[group], &edited[group]);
        if let Some(value) = group_diff {
            overrides.insert(group.to_owned(), value);
        }
    }
    Ok(Value::Object(overrides))
}

fn object_diff(base: &Value, edited: &Value) -> Option<Value> {
    let (Value::Object(base), Value::Object(edited)) = (base, edited) else {
        return (base != edited).then(|| edited.clone());
    };
    let mut difference = Map::new();
    for (key, value) in edited {
        let nested = match base.get(key) {
            Some(base_value) => object_diff(base_value, value),
            None => Some(value.clone()),
        };
        if let Some(nested) = nested {
            difference.insert(key.clone(), nested);
        }
    }
    (!difference.is_empty()).then_some(Value::Object(difference))
}

fn parse_u32(value: &str, label: &str) -> Result<u32, ApiFailure> {
    value
        .parse()
        .map_err(|_| invalid_input(format!("{label}必须是非负整数")))
}

fn parse_uma_values(value: &str) -> Result<Vec<i16>, ApiFailure> {
    value
        .split(',')
        .map(|part| {
            part.trim()
                .parse::<i16>()
                .map_err(|_| invalid_input("马点格式应为 +30,+10,-10,-30"))
        })
        .collect()
}

fn format_uma_values(uma: &PlacementUma) -> String {
    match uma {
        PlacementUma::Fixed { values } => values
            .iter()
            .map(|value| format!("{value:+}"))
            .collect::<Vec<_>>()
            .join(","),
        PlacementUma::JpmlA => String::new(),
    }
}

fn cycle_string(value: &mut String, options: &[&str], delta: i8) {
    let current = options
        .iter()
        .position(|option| *option == value)
        .unwrap_or(0);
    let next = if delta.is_negative() {
        (current + options.len() - 1) % options.len()
    } else {
        (current + 1) % options.len()
    };
    *value = options[next].to_owned();
}

fn adjust_red(value: &mut u8, delta: i8) {
    *value = if delta.is_negative() {
        value.saturating_sub(1)
    } else {
        value.saturating_add(1).min(4)
    };
}

fn toggle(value: &mut bool) {
    *value = !*value;
}

/// Three-state kuikae rule; suji only ever applies to a chi at a sequence end.
fn kuikae_label(value: &str) -> &'static str {
    match value {
        "allowed" => "允许",
        "same_tile_only" => "仅禁现物",
        _ => "禁止",
    }
}

fn bool_label(value: bool) -> String {
    if value { "有" } else { "无" }.to_owned()
}

fn label_pair(value: &str, first: &str, first_label: &str, second_label: &str) -> String {
    if value == first {
        first_label
    } else {
        second_label
    }
    .to_owned()
}

fn invalid_input(message: impl Into<String>) -> ApiFailure {
    ApiFailure {
        code: "client.invalid_input".to_owned(),
        message: message.into(),
    }
}

fn invalid_serialization(error: serde_json::Error) -> ApiFailure {
    ApiFailure {
        code: "client.invalid_response".to_owned(),
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{CreateField, CreateRoomForm};
    use crate::fixtures::rule_catalog;

    #[test]
    fn preset_override_contains_only_changed_field() {
        let mut form = CreateRoomForm::new(rule_catalog()).expect("create form");
        form.active_field = 3;
        form.change_active(1);
        assert_eq!(form.preset().expect("preset").id, "m-league");

        form.page = super::RulePage::Match;
        form.active_field = 3;
        assert_eq!(form.active(), CreateField::Tobi);
        form.change_active(1);

        let payload = form.create_payload().expect("payload");
        assert_eq!(
            payload["rules"]["config"]["preset"],
            json!({"id": "m-league", "revision": 1})
        );
        assert_eq!(
            payload["rules"]["config"]["overrides"],
            json!({"match_rules": {"tobi": true}})
        );
    }

    #[test]
    fn sanma_resets_incompatible_preset_and_fields() {
        let mut form = CreateRoomForm::new(rule_catalog()).expect("create form");
        form.active_field = 2;
        form.change_active(1);

        assert_eq!(form.rule_set().id, "riichi/sanma");
        assert!(form.preset().is_none());
        assert!(form.field_unavailable(CreateField::RedMan));
        assert!(form.field_unavailable(CreateField::FourWinds));
        assert!(form.field_unavailable(CreateField::FourRiichi));
    }
}
