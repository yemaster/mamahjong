import { useEffect, useState, type FormEvent, type ReactNode } from "react";
import { useQuery } from "@tanstack/react-query";
import { apiFailure, gameApi } from "../../api";
import { resumeCurrentActivity } from "../../activity";
import { navigateTo } from "../../routing";
/*
 * 两种麻将的设置项完全不同，下面按种类整片换：立直走原来那套，冲击麻将走
 * `ImpactRuleSettings`。种类名跟房间页、牌谱标题共用一份表。
 */
import { MAHJONG_FAMILY_LABELS, mahjongFamilyOf } from "../../ruleTitle";
import type {
  ImpactAllInRules,
  ImpactKanRules,
  ImpactMode,
  ImpactRuleConfig,
  PlacementUma,
  RiichiRuleConfig,
  RuleConfig,
} from "../../types";

interface Props {
  token: string | null;
  onBack: () => void;
}

/** 自定义规则这一项不对应任何预设，只是标出「这桌的规则是手调的」。 */
const CUSTOM_PRESET_ID = "custom";

/** 配置是哪一家的，看有没有全交那一组就够了。 */
function isImpactConfig(config: RuleConfig): config is ImpactRuleConfig {
  return "all_in" in config;
}

export function impactRulesForMode(
  current: ImpactRuleConfig,
  mode: ImpactMode,
): ImpactRuleConfig {
  const bright = mode === "bright";
  return {
    ...current,
    mode,
    kan: {
      added_kan_single_payer: !bright,
      indicator_pon_counts_as_kan: !bright,
      first_round_repeat_discard: !bright,
      four_identical_discards_as_kan: !bright,
      pon_with_few_tiles_as_kan: !bright,
    },
    all_in: {
      eleven_honor_streak: true,
      all_honors: !bright,
      pure_flush_no_joker: !bright,
      single_wait: !bright,
      three_kans: !bright,
      four_jokers: true,
      pure_seven_pairs: !bright,
      last_tile: !bright,
      blessing: true,
    },
  };
}

export function CreateRoomPanel({ token, onBack }: Props) {
  const catalog = useQuery({
    queryKey: ["ruleSets"],
    queryFn: gameApi.ruleSets,
  });
  const [name, setName] = useState("");
  const [visibility, setVisibility] = useState<"public" | "private">("public");
  const [ruleSetId, setRuleSetId] = useState("riichi/yonma");
  const [presetId, setPresetId] = useState("");
  const [rules, setRules] = useState<RuleConfig | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const selectedRuleSet = catalog.data?.rule_sets.find(
    (ruleSet) => ruleSet.id === ruleSetId,
  );
  const family = mahjongFamilyOf(ruleSetId);
  const families = Array.from(
    new Set(
      (catalog.data?.rule_sets ?? []).map((ruleSet) =>
        mahjongFamilyOf(ruleSet.id),
      ),
    ),
  );
  const familyRuleSets = (catalog.data?.rule_sets ?? []).filter(
    (ruleSet) => mahjongFamilyOf(ruleSet.id) === family,
  );
  // 底下两片设置只会有一片是非空的：拿到哪一片，就渲染哪一套面板。
  const riichiRules = rules && !isImpactConfig(rules) ? rules : null;
  const impactRules = rules && isImpactConfig(rules) ? rules : null;

  useEffect(() => {
    if (selectedRuleSet && !rules) {
      setRules(cloneRules(selectedRuleSet.default_config));
    }
  }, [rules, selectedRuleSet]);

  const changeRuleSet = (nextId: string) => {
    const next = catalog.data?.rule_sets.find(
      (ruleSet) => ruleSet.id === nextId,
    );
    setRuleSetId(nextId);
    setPresetId("");
    setRules(next ? cloneRules(next.default_config) : null);
    setError(null);
  };

  /** 换麻将种类，底下整片设置项跟着换成那一种的默认规则。 */
  const changeFamily = (nextFamily: string) => {
    if (nextFamily === family) return;
    const first = catalog.data?.rule_sets.find(
      (ruleSet) => mahjongFamilyOf(ruleSet.id) === nextFamily,
    );
    if (first) changeRuleSet(first.id);
  };

  const changePreset = (nextId: string) => {
    setPresetId(nextId);
    setError(null);
    // 选自定义规则只是改个名头，桌上已经调好的每一项都留着。
    if (nextId === CUSTOM_PRESET_ID) return;
    const source =
      selectedRuleSet?.presets.find((preset) => preset.id === nextId)?.config ??
      selectedRuleSet?.default_config;
    setRules(source ? cloneRules(source) : null);
  };

  /** 只要玩家动了任意一项，这桌就算自己调的了。 */
  const markCustomRules = () => setPresetId(CUSTOM_PRESET_ID);

  const updateSection = <
    K extends Exclude<keyof RiichiRuleConfig, "variant">,
  >(
    section: K,
    patch: Partial<RiichiRuleConfig[K]>,
  ) => {
    markCustomRules();
    setRules((current) =>
      current && !isImpactConfig(current)
        ? {
            ...current,
            [section]: Object.assign({}, current[section], patch),
          }
        : current,
    );
  };

  const updateImpactSection = <
    K extends Exclude<keyof ImpactRuleConfig, "mode">,
  >(
    section: K,
    patch: Partial<ImpactRuleConfig[K]>,
  ) => {
    markCustomRules();
    setRules((current) =>
      current && isImpactConfig(current)
        ? {
            ...current,
            [section]: Object.assign({}, current[section], patch),
          }
        : current,
    );
  };

  const changeImpactMode = (mode: ImpactMode) => {
    markCustomRules();
    setRules((current) => {
      if (!current || !isImpactConfig(current)) return current;
      return impactRulesForMode(current, mode);
    });
  };

  const updateRedFive = (suit: "man" | "pin" | "sou", value: number) => {
    markCustomRules();
    setRules((current) =>
      current && !isImpactConfig(current)
        ? {
            ...current,
            bonuses: {
              ...current.bonuses,
              red_fives: {
                ...current.bonuses.red_fives,
                [suit]: value,
              },
            },
          }
        : current,
    );
  };

  const updateUma = (uma: PlacementUma) => {
    updateSection("settlement", { uma });
  };

  const updateUmaValue = (index: number, value: number) => {
    if (!riichiRules || riichiRules.settlement.uma.type !== "fixed") return;
    const values = [...riichiRules.settlement.uma.values];
    values[index] = value;
    updateUma({ type: "fixed", values });
  };

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!token || !rules || loading) return;

    if (
      riichiRules &&
      riichiRules.settlement.uma.type === "fixed" &&
      (riichiRules.settlement.uma.values.length !==
        selectedRuleSet?.seat_count ||
        riichiRules.settlement.uma.values.reduce(
          (sum, value) => sum + value,
          0,
        ) !== 0)
    ) {
      setError("顺位马数量必须与人数一致，并且合计为零");
      return;
    }

    setLoading(true);
    setError(null);
    try {
      const normalizedRules = cloneRules(rules);
      const preset = selectedRuleSet?.presets.find(
        (candidate) => candidate.id === presetId,
      );
      const room = await gameApi.createRoom(
        {
          name: name.trim() || "好友房间",
          visibility,
          rules: {
            rule_set_id: ruleSetId,
            config: {
              ...(preset
                ? { preset: { id: preset.id, revision: preset.revision } }
                : {}),
              overrides: overridesOf(normalizedRules),
            },
          },
        },
        token,
      );
      navigateTo({ kind: "room", roomId: room.id });
    } catch (requestError) {
      const failure = apiFailure(requestError);
      if (failure.code === "lobby.user_busy") {
        const resumed = await resumeCurrentActivity(token).catch(
          () => false,
        );
        if (resumed) return;
      }
      setError(
        failure.code === "request.invalid_rule_config"
          ? "规则设置有误，请检查点数、思考秒数、顺位马和赤宝牌数量"
          : "创建失败，请稍后重试",
      );
    } finally {
      setLoading(false);
    }
  };

  return (
    <form
      className="game-lobby__menu lobby-create"
      aria-label="创建房间"
      onSubmit={submit}
    >
      <header className="lobby-create__header">
        <span aria-hidden="true">创</span>
        <h2>创建房间</h2>
      </header>

      <div className="lobby-create__body">
        {catalog.error ? (
          <div className="lobby-create__error">规则加载失败</div>
        ) : null}
        {error ? <div className="lobby-create__error">{error}</div> : null}

        <label className="lobby-create__field">
          <span>房间名称</span>
          <input
            value={name}
            maxLength={40}
            placeholder="好友房间"
            onChange={(event) => setName(event.target.value)}
          />
        </label>

        <div className="lobby-create__field">
          <span>麻将选项</span>
          <div className="lobby-create__tabs" role="tablist">
            {families.map((option) => (
              <button
                key={option}
                type="button"
                role="tab"
                aria-selected={option === family}
                className={option === family ? "is-active" : ""}
                onClick={() => changeFamily(option)}
              >
                {MAHJONG_FAMILY_LABELS[option] ?? option}
              </button>
            ))}
          </div>
        </div>

        {/*
          冲击麻将只有 `impact/yonma` 一套、也没有流派，这两个下拉框摆出来就是
          一个只有一项、一个「标准 / 自定义」——点开只会让人以为还有别的选择。
          这里按种类判断而不是按「只有一套 / 没有预设」判断：立直的三麻同样没有
          预设，那边的下拉框得原样留着。
        */}
        {family === "impact" ? null : (
          <div className="lobby-create__basic-grid">
            <label className="lobby-create__field">
              <span>人数规则</span>
              <select
                value={ruleSetId}
                onChange={(event) => changeRuleSet(event.target.value)}
              >
                {familyRuleSets.map((ruleSet) => (
                  <option key={ruleSet.id} value={ruleSet.id}>
                    {ruleSet.display_name}
                  </option>
                ))}
              </select>
            </label>

            <label className="lobby-create__field">
              <span>规则设置</span>
              <select
                value={presetId}
                onChange={(event) => changePreset(event.target.value)}
              >
                <option value="">默认规则</option>
                {selectedRuleSet?.presets.map((preset) => (
                  <option key={preset.id} value={preset.id}>
                    {presetLabel(preset.id, preset.display_name)}
                  </option>
                ))}
                <option value={CUSTOM_PRESET_ID}>自定义规则</option>
              </select>
            </label>
          </div>
        )}

        <SettingRow label="房间范围">
          <Choice
            value={visibility}
            options={[
              ["public", "公开"],
              ["private", "私密"],
            ]}
            onChange={setVisibility}
          />
        </SettingRow>

        {riichiRules ? (
          <div className="lobby-create__basic-rules">
            <RuleGroup title="对局设置" open>
              <SettingRow label="对局长度">
                <Choice
                  value={riichiRules.match_rules.length}
                  options={[
                    ["east_only", "东风"],
                    ["hanchan", "半庄"],
                  ]}
                  onChange={(length) =>
                    updateSection("match_rules", { length })
                  }
                />
              </SettingRow>
              {riichiRules.variant === "sanma" ? (
                <SettingRow label="北">
                  <Choice
                    value={riichiRules.match_rules.north ?? "nuki_dora"}
                    options={[
                      ["nuki_dora", "拔北宝牌"],
                      ["yakuhai", "役牌"],
                    ]}
                    onChange={(north) =>
                      updateSection("match_rules", { north })
                    }
                  />
                </SettingRow>
              ) : null}
              <SettingRow label="思考秒数">
                <Choice
                  value={thinkingTimeValue(riichiRules)}
                  options={[
                    ["5+0", "5+0"],
                    ["5+20", "5+20"],
                    ["5+60", "5+60"],
                    ["15+60", "15+60"],
                  ]}
                  onChange={(value) =>
                    updateSection("match_rules", {
                      thinking_time: parseThinkingTime(value),
                    })
                  }
                />
              </SettingRow>
              <NumberSetting
                label="起始点数"
                value={riichiRules.match_rules.initial_points}
                min={1000}
                max={1_000_000}
                step={1000}
                onChange={(initial_points) =>
                  updateSection("match_rules", {
                    initial_points,
                    return_points: initial_points,
                  })
                }
              />
              <NumberSetting
                label="一位必要点数"
                value={riichiRules.match_rules.first_place_required_points}
                min={1000}
                max={1_000_000}
                step={1000}
                onChange={(first_place_required_points) =>
                  updateSection("match_rules", {
                    first_place_required_points,
                  })
                }
              />
              <ToggleSetting
                label="击飞结束"
                checked={riichiRules.match_rules.tobi}
                onChange={(tobi) => updateSection("match_rules", { tobi })}
              />
              <SettingRow label="庄家连庄">
                <Choice
                  value={riichiRules.match_rules.dealer_continuation}
                  options={[
                    ["win_only", "和牌"],
                    ["win_or_tenpai", "和牌或听牌"],
                  ]}
                  onChange={(dealer_continuation) =>
                    updateSection("match_rules", { dealer_continuation })
                  }
                />
              </SettingRow>
              <ToggleSetting
                label="终局止和"
                checked={riichiRules.match_rules.agari_yame}
                onChange={(agari_yame) =>
                  updateSection("match_rules", { agari_yame })
                }
              />
            </RuleGroup>
          </div>
        ) : null}

        {impactRules ? (
          <ImpactRuleSettings
            rules={impactRules}
            onChange={updateImpactSection}
            onModeChange={changeImpactMode}
          />
        ) : null}

        {riichiRules ? (
          <div className="lobby-create__basic-rules">
            <RuleGroup title="计分设置">
              <ToggleSetting
                label="切上满贯"
                checked={riichiRules.scoring.kiriage_mangan}
                onChange={(kiriage_mangan) =>
                  updateSection("scoring", { kiriage_mangan })
                }
              />
              <ToggleSetting
                label="古役"
                checked={riichiRules.scoring.old_yaku}
                onChange={(old_yaku) =>
                  updateSection("scoring", { old_yaku })
                }
              />
              <SettingRow label="役满累计">
                <Choice
                  value={riichiRules.scoring.yakuman_value}
                  options={[
                    ["stacked_only", "仅累计"],
                    ["double_variants_and_stacked", "双倍并累计"],
                  ]}
                  onChange={(yakuman_value) =>
                    updateSection("scoring", { yakuman_value })
                  }
                />
              </SettingRow>
              <ToggleSetting
                label="流局满贯"
                checked={riichiRules.scoring.nagashi_mangan}
                onChange={(nagashi_mangan) =>
                  updateSection("scoring", { nagashi_mangan })
                }
              />
              <ToggleSetting
                label="累计役满"
                checked={riichiRules.scoring.kazoe_yakuman}
                onChange={(kazoe_yakuman) =>
                  updateSection("scoring", { kazoe_yakuman })
                }
              />
              <ToggleSetting
                label="国士无双抢暗杠"
                checked={riichiRules.scoring.kokushi_ankan_chankan}
                onChange={(kokushi_ankan_chankan) =>
                  updateSection("scoring", { kokushi_ankan_chankan })
                }
              />
            </RuleGroup>

            <RuleGroup title="副露设置">
              <ToggleSetting
                label="食断"
                checked={riichiRules.calls.kuitan}
                onChange={(kuitan) => updateSection("calls", { kuitan })}
              />
              <SettingRow label="食替">
                <Choice
                  value={riichiRules.calls.kuikae}
                  options={[
                    ["forbidden", "禁止"],
                    ["same_tile_only", "仅同牌"],
                    ["allowed", "允许"],
                  ]}
                  onChange={(kuikae) => updateSection("calls", { kuikae })}
                />
              </SettingRow>
            </RuleGroup>

            <RuleGroup title="宝牌设置">
              <NumberSetting
                label="赤五万"
                value={riichiRules.bonuses.red_fives.man}
                min={0}
                max={4}
                disabled={riichiRules.variant === "sanma"}
                onChange={(value) => updateRedFive("man", value)}
              />
              <NumberSetting
                label="赤五筒"
                value={riichiRules.bonuses.red_fives.pin}
                min={0}
                max={4}
                onChange={(value) => updateRedFive("pin", value)}
              />
              <NumberSetting
                label="赤五索"
                value={riichiRules.bonuses.red_fives.sou}
                min={0}
                max={4}
                onChange={(value) => updateRedFive("sou", value)}
              />
              <ToggleSetting
                label="一发"
                checked={riichiRules.bonuses.ippatsu}
                onChange={(ippatsu) =>
                  updateSection("bonuses", { ippatsu })
                }
              />
              <ToggleSetting
                label="里宝牌"
                checked={riichiRules.bonuses.ura_dora}
                onChange={(ura_dora) =>
                  updateSection("bonuses", { ura_dora })
                }
              />
              <ToggleSetting
                label="杠宝牌"
                checked={riichiRules.bonuses.kan_dora}
                onChange={(kan_dora) =>
                  updateSection("bonuses", { kan_dora })
                }
              />
            </RuleGroup>

            <RuleGroup title="途中流局">
              <ToggleSetting
                label="四风连打"
                checked={riichiRules.abortive_draws.four_winds}
                disabled={riichiRules.variant === "sanma"}
                onChange={(four_winds) =>
                  updateSection("abortive_draws", { four_winds })
                }
              />
              <ToggleSetting
                label="四杠散了"
                checked={riichiRules.abortive_draws.four_kans}
                onChange={(four_kans) =>
                  updateSection("abortive_draws", { four_kans })
                }
              />
              <ToggleSetting
                label="九种九牌"
                checked={riichiRules.abortive_draws.nine_terminals}
                onChange={(nine_terminals) =>
                  updateSection("abortive_draws", { nine_terminals })
                }
              />
              <ToggleSetting
                label="四家立直"
                checked={riichiRules.abortive_draws.four_riichi}
                disabled={riichiRules.variant === "sanma"}
                onChange={(four_riichi) =>
                  updateSection("abortive_draws", { four_riichi })
                }
              />
            </RuleGroup>

            <RuleGroup title="结算设置">
              <SettingRow label="顺位马">
                <Choice
                  value={riichiRules.settlement.uma.type}
                  options={[
                    ["fixed", "固定"],
                    ...(riichiRules.variant === "yonma"
                      ? ([["jpml_a", "联盟浮动"]] as const)
                      : []),
                  ]}
                  onChange={(type) =>
                    updateUma(
                      type === "jpml_a"
                        ? { type: "jpml_a" }
                        : {
                            type: "fixed",
                            values:
                              riichiRules.variant === "yonma"
                                ? [30, 10, -10, -30]
                                : [30, 0, -30],
                          },
                    )
                  }
                />
              </SettingRow>
              {riichiRules.settlement.uma.type === "fixed" ? (
                <SettingRow label="顺位值">
                  <div className="lobby-create__uma">
                    {riichiRules.settlement.uma.values.map((value, index) => (
                      <input
                        key={index}
                        type="number"
                        value={value}
                        aria-label={`第${index + 1}名顺位值`}
                        onChange={(event) =>
                          updateUmaValue(index, Number(event.target.value))
                        }
                      />
                    ))}
                  </div>
                </SettingRow>
              ) : null}
              <NumberSetting
                label="未听罚符"
                value={riichiRules.settlement.noten_payment}
                min={0}
                max={100_000}
                step={1000}
                onChange={(noten_payment) =>
                  updateSection("settlement", { noten_payment })
                }
              />
              <SettingRow label="多家和牌">
                <Choice
                  value={riichiRules.settlement.ron_resolution}
                  options={[
                    ["head_bump", "头跳"],
                    ["multiple", "同时和牌"],
                  ]}
                  onChange={(ron_resolution) =>
                    updateSection("settlement", { ron_resolution })
                  }
                />
              </SettingRow>
            </RuleGroup>
          </div>
        ) : null}
      </div>

      <footer className="lobby-create__actions">
        <button type="button" onClick={onBack}>
          返回
        </button>
        <button type="submit" disabled={!rules || loading}>
          {loading ? "创建中" : "创建房间"}
        </button>
      </footer>
    </form>
  );
}

/** 改冲击麻将某一组设置。签名与 `updateImpactSection` 一致，直接把它传下来。 */
type ImpactSectionUpdater = <K extends Exclude<keyof ImpactRuleConfig, "mode">>(
  section: K,
  patch: Partial<ImpactRuleConfig[K]>,
) => void;

/** 杠牌设置；瞎子默认全开，亮子默认全关。 */
const IMPACT_KAN_LABELS: readonly (readonly [keyof ImpactKanRules, string])[] = [
  ["added_kan_single_payer", "加杠时仅单人支付"],
  ["indicator_pon_counts_as_kan", "指示牌碰牌算杠"],
  ["first_round_repeat_discard", "第一巡连打需要庄家支付杠点"],
  ["four_identical_discards_as_kan", "打出四张相同牌算杠"],
  ["pon_with_few_tiles_as_kan", "手牌≤4张时碰牌收杠点"],
];

/** 全交设置；瞎子默认全开，亮子只默认开连打11风、四龙、天和地和。 */
const IMPACT_ALL_IN_LABELS: readonly (readonly [
  keyof ImpactAllInRules,
  string,
])[] = [
  ["eleven_honor_streak", "连打11风全交"],
  ["all_honors", "全风全交"],
  ["pure_flush_no_joker", "无龙清一色全交"],
  ["single_wait", "单吊全交"],
  ["three_kans", "三杠全交"],
  ["four_jokers", "四龙全交"],
  ["pure_seven_pairs", "清七对全交"],
  ["last_tile", "海底全交"],
  ["blessing", "天和地和全交"],
];

/**
 * 冲击麻将的设置面板。
 *
 * 与立直那片完全不共用：起始点数（100）、杠点起始（0）、连庄条件、对局长度
 * 都是规则写死的，能调的只有思考秒数 + 三组开关。
 */
function ImpactRuleSettings({
  rules,
  onChange,
  onModeChange,
}: {
  rules: ImpactRuleConfig;
  onChange: ImpactSectionUpdater;
  onModeChange: (mode: ImpactMode) => void;
}) {
  return (
    <div className="lobby-create__basic-rules">
      <RuleGroup title="模式" open>
        <SettingRow label="模式">
          <Choice
            value={rules.mode}
            options={[
              ["blind", "瞎子麻将"],
              ["bright", "亮子麻将"],
            ]}
            onChange={(mode) => onModeChange(mode as ImpactMode)}
          />
        </SettingRow>
      </RuleGroup>

      <RuleGroup title="对局设置" open>
        <SettingRow label="思考秒数">
          <Choice
            value={thinkingTimeValue(rules)}
            options={[
              ["5+0", "5+0"],
              ["5+20", "5+20"],
              ["5+60", "5+60"],
              ["15+60", "15+60"],
            ]}
            onChange={(value) =>
              onChange("match_rules", {
                thinking_time: parseThinkingTime(value),
              })
            }
          />
        </SettingRow>
      </RuleGroup>

      <RuleGroup title="杠牌设置" open>
        {IMPACT_KAN_LABELS.map(([key, label]) => (
          <ToggleSetting
            key={key}
            label={label}
            checked={rules.kan[key]}
            onChange={(value) => onChange("kan", { [key]: value })}
          />
        ))}
      </RuleGroup>

      <RuleGroup title="特殊规则设置">
        <ToggleSetting
          label="七嵌"
          checked={rules.special.seven_gaps}
          onChange={(seven_gaps) => onChange("special", { seven_gaps })}
        />
      </RuleGroup>

      <RuleGroup title="全交设置">
        {IMPACT_ALL_IN_LABELS.map(([key, label]) => (
          <ToggleSetting
            key={key}
            label={label}
            checked={rules.all_in[key]}
            onChange={(value) => onChange("all_in", { [key]: value })}
          />
        ))}
      </RuleGroup>
    </div>
  );
}

function RuleGroup({
  title,
  open,
  children,
}: {
  title: string;
  open?: boolean;
  children: ReactNode;
}) {
  return (
    <details className="lobby-create__group" open={open}>
      <summary>{title}</summary>
      <div>{children}</div>
    </details>
  );
}

function SettingRow({
  label,
  children,
}: {
  label: string;
  children: ReactNode;
}) {
  return (
    <div className="lobby-create__setting">
      <span>{label}</span>
      {children}
    </div>
  );
}

function ToggleSetting({
  label,
  checked,
  disabled,
  onChange,
}: {
  label: string;
  checked: boolean;
  disabled?: boolean;
  onChange: (value: boolean) => void;
}) {
  return (
    <SettingRow label={label}>
      <button
        type="button"
        className={`lobby-create__toggle${checked ? " is-on" : ""}`}
        role="switch"
        aria-checked={checked}
        disabled={disabled}
        onClick={() => onChange(!checked)}
      >
        {checked ? "开" : "关"}
      </button>
    </SettingRow>
  );
}

function NumberSetting({
  label,
  value,
  min,
  max,
  step = 1,
  disabled,
  onChange,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  step?: number;
  disabled?: boolean;
  onChange: (value: number) => void;
}) {
  return (
    <SettingRow label={label}>
      <input
        className="lobby-create__number"
        type="number"
        value={value}
        min={min}
        max={max}
        step={step}
        disabled={disabled}
        onChange={(event) => onChange(Number(event.target.value))}
      />
    </SettingRow>
  );
}

function Choice<T extends string>({
  value,
  options,
  onChange,
}: {
  value: T;
  options: readonly (readonly [T, string])[];
  onChange: (value: T) => void;
}) {
  return (
    <div className="lobby-create__choice">
      {options.map(([optionValue, label]) => (
        <button
          key={optionValue}
          type="button"
          className={optionValue === value ? "is-active" : ""}
          onClick={() => onChange(optionValue)}
        >
          {label}
        </button>
      ))}
    </div>
  );
}

function cloneRules(rules: RuleConfig): RuleConfig {
  const clone = JSON.parse(JSON.stringify(rules)) as RuleConfig;
  clone.match_rules.thinking_time ??= {
    base_seconds: 5,
    reserve_seconds: 20,
  };
  // 下面两项是立直特有的；冲击麻将的对局设置只有思考秒数，补不出这些字段。
  if (isImpactConfig(clone)) return clone;
  clone.match_rules.return_points = clone.match_rules.initial_points;
  clone.match_rules.first_place_required_points ??= 30_000;
  return clone;
}

/**
 * 把整份配置摊成后端认的 overrides。
 *
 * 两家的 overrides 都是 `deny_unknown_fields`，字段对不上会被整份拒掉，
 * 所以这里按家族分别只挑该家族有的那几组，多余的一律不发。
 */
function overridesOf(config: RuleConfig): Record<string, unknown> {
  if (isImpactConfig(config)) {
    return {
      mode: config.mode,
      match_rules: { thinking_time: config.match_rules.thinking_time },
      kan: config.kan,
      special: config.special,
      all_in: config.all_in,
    };
  }
  return {
    match_rules: config.match_rules,
    scoring: config.scoring,
    calls: config.calls,
    bonuses: config.bonuses,
    abortive_draws: config.abortive_draws,
    settlement: config.settlement,
  };
}

function thinkingTimeValue(rules: RuleConfig): string {
  const { base_seconds, reserve_seconds } = rules.match_rules.thinking_time;
  return `${base_seconds}+${reserve_seconds}`;
}

function parseThinkingTime(value: string): {
  base_seconds: number;
  reserve_seconds: number;
} {
  const [base_seconds = 5, reserve_seconds = 20] = value
    .split("+")
    .map(Number);
  return { base_seconds, reserve_seconds };
}

function presetLabel(id: string, fallback: string): string {
  const labels: Record<string, string> = {
    "jpml-a": "联盟 A 规",
    saikouisen: "最高位战规则",
    "m-league": "ML 规",
  };
  return labels[id] ?? fallback;
}
