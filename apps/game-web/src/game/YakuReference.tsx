import { useMemo, useState } from "react";
import { ArrowLeft, X } from "lucide-react";
import { tileAssetPath } from "./tileAssets";
import {
  yakuReferenceEntries,
  yakuEntryTabs,
  yakuReferenceTabs,
  type YakuReferenceTab,
  yakuValueTags,
} from "./yakuReferenceData";
import {
  IMPACT_ALL_IN_TEXT,
  IMPACT_BASE_VALUE_TEXT,
  impactAllInEntries,
  impactReferenceTabs,
  impactRuleSections,
  impactYakuEntries,
  type ImpactReferenceTab,
} from "./impactReferenceData";

export function YakuReferencePage({
  onBack,
}: {
  onBack: () => void;
}) {
  return (
    <section className="yaku-reference-page">
      <YakuReferenceHeader onBack={onBack} />
      <YakuReferenceBrowser />
    </section>
  );
}

export function YakuReferenceModal({
  onClose,
}: {
  onClose: () => void;
}) {
  return (
    <div
      className="yaku-reference-modal"
      role="dialog"
      aria-modal="true"
      aria-label="帮助"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <section className="yaku-reference-modal__panel">
        <YakuReferenceHeader onClose={onClose} />
        <YakuReferenceBrowser />
      </section>
    </div>
  );
}

function YakuReferenceHeader({
  onBack,
  onClose,
}: {
  onBack?: () => void;
  onClose?: () => void;
}) {
  return (
    <header className="yaku-reference__header">
      {onBack && (
        <button
          type="button"
          onClick={onBack}
          aria-label="返回大厅"
          title="返回大厅"
        >
          <ArrowLeft aria-hidden="true" />
        </button>
      )}
      <h1>帮助</h1>
      {onClose && (
        <button
          type="button"
          onClick={onClose}
          aria-label="关闭"
          title="关闭"
        >
          <X aria-hidden="true" />
        </button>
      )}
    </header>
  );
}

/**
 * 帮助页分两级：先选麻将种类，点进去才是那一种的规则。
 *
 * 不做成并排的一排 tab，是因为两家的内容从数据到画法都不共用，摆在同一排上像是
 * 同一份东西的两个筛选项。分成两级之后，进来先看到的是「有哪几种麻将」，
 * 里面那层 tab 就明确只属于当前这一种。
 */
const REFERENCE_FAMILIES = [
  {
    key: "riichi",
    label: "立直麻将",
    summary: "标准四人 / 三人日麻。按番数查役种，每条都配示例牌型。",
  },
  {
    key: "impact",
    label: "冲击麻将",
    summary: "只自摸、财神百搭、另有一本杠点账。规则、番种与全交。",
  },
] as const;

type ReferenceFamily = (typeof REFERENCE_FAMILIES)[number]["key"];

function YakuReferenceBrowser() {
  /* null 就是还停在种类列表那一层。 */
  const [family, setFamily] = useState<ReferenceFamily | null>(null);

  if (family === null) {
    return (
      <div className="yaku-reference__browser">
        <ul className="yaku-reference__families">
          {REFERENCE_FAMILIES.map((entry) => (
            <li key={entry.key}>
              <button type="button" onClick={() => setFamily(entry.key)}>
                <h2>{entry.label}</h2>
                <p>{entry.summary}</p>
                <span aria-hidden="true">查看规则 →</span>
              </button>
            </li>
          ))}
        </ul>
      </div>
    );
  }

  const current = REFERENCE_FAMILIES.find((entry) => entry.key === family);

  return (
    <div className="yaku-reference__browser">
      {/* 这一行只退回种类列表；标题栏那个箭头仍然是退出整个帮助页。 */}
      <div className="yaku-reference__crumb">
        <button
          type="button"
          onClick={() => setFamily(null)}
          aria-label="返回麻将种类"
        >
          <ArrowLeft aria-hidden="true" />
          <span>麻将种类</span>
        </button>
        <strong>{current?.label}</strong>
      </div>
      {family === "riichi" ? <RiichiReference /> : <ImpactReference />}
    </div>
  );
}

function RiichiReference() {
  const [activeTab, setActiveTab] =
    useState<YakuReferenceTab>("1番");
  const entries = useMemo(
    () =>
      yakuReferenceEntries.filter((entry) =>
        yakuEntryTabs(entry).includes(activeTab),
      ),
    [activeTab],
  );

  return (
    <>
      <div
        className="yaku-reference__tabs"
        role="tablist"
        aria-label="按番数筛选"
      >
        {yakuReferenceTabs.map((tab) => (
          <button
            key={tab}
            type="button"
            role="tab"
            aria-selected={activeTab === tab}
            onClick={() => setActiveTab(tab)}
          >
            {tab}
          </button>
        ))}
      </div>
      <YakuReferenceList entries={entries} />
    </>
  );
}

function ImpactReference() {
  const [activeTab, setActiveTab] =
    useState<ImpactReferenceTab>("基本规则");

  return (
    <>
      <div
        className="yaku-reference__tabs"
        role="tablist"
        aria-label="按主题筛选"
      >
        {impactReferenceTabs.map((tab) => (
          <button
            key={tab}
            type="button"
            role="tab"
            aria-selected={activeTab === tab}
            onClick={() => setActiveTab(tab)}
          >
            {tab}
          </button>
        ))}
      </div>
      <div className="yaku-reference__list is-impact" role="tabpanel">
        {activeTab === "基本规则" || activeTab === "杠点"
          ? impactRuleSections[activeTab].map((section) => (
              <article className="yaku-reference__item" key={section.title}>
                <div className="yaku-reference__title">
                  <h2>{section.title}</h2>
                </div>
                <dl className="impact-reference__terms">
                  {section.items.map((item) => (
                    <div key={item.term}>
                      <dt>{item.term}</dt>
                      <dd>{item.detail}</dd>
                    </div>
                  ))}
                </dl>
              </article>
            ))
          : null}
        {activeTab === "番种" && (
          <ImpactYakuTable
            note={IMPACT_BASE_VALUE_TEXT}
            entries={impactYakuEntries}
          />
        )}
        {activeTab === "全交" && (
          <ImpactYakuTable
            note={IMPACT_ALL_IN_TEXT}
            entries={impactAllInEntries}
          />
        )}
      </div>
    </>
  );
}

function ImpactYakuTable({
  note,
  entries,
}: {
  note: string;
  entries: typeof impactYakuEntries;
}) {
  return (
    <>
      <p className="impact-reference__note">{note}</p>
      {entries.map((entry) => (
        <article className="yaku-reference__item" key={entry.name}>
          <div className="yaku-reference__title">
            <h2>{entry.name}</h2>
            <div className="yaku-reference__tags">
              <span>{entry.value}</span>
            </div>
          </div>
          <p>{entry.description}</p>
        </article>
      ))}
    </>
  );
}

function YakuReferenceList({
  entries,
}: {
  entries: typeof yakuReferenceEntries;
}) {
  return (
    <div className="yaku-reference__list" role="tabpanel">
      {entries.map((entry) => (
        <article className="yaku-reference__item" key={entry.name}>
          <div className="yaku-reference__title">
            <h2>{entry.name}</h2>
            <div className="yaku-reference__tags">
              {yakuValueTags(entry.value).map((value) => (
                <span key={value}>{value}</span>
              ))}
              {entry.openReduction && (
                <span className="is-reduction">
                  副露减{entry.openReduction}番
                </span>
              )}
              {entry.menzenRequired && (
                <span className="is-menzen">门前清限定</span>
              )}
            </div>
          </div>
          <p>{entry.description}</p>
          <div
            className="yaku-reference__example"
            aria-label={`${entry.name}示例牌型`}
          >
            {entry.tiles.map((code, index) => {
              const meld = entry.meldGroups?.find(
                (group) =>
                  index >= group.start &&
                  index < group.start + group.length,
              );
              return (
                <MiniHandTile
                  key={`${entry.name}-${index}-${code}`}
                  code={code}
                  groupStart={
                    entry.groupStarts?.includes(index) ?? false
                  }
                  meldStart={meld?.start === index}
                  winning={entry.winningTileIndex === index}
                />
              );
            })}
          </div>
        </article>
      ))}
    </div>
  );
}

function MiniHandTile({
  code,
  groupStart,
  meldStart,
  winning,
}: {
  code: string;
  groupStart: boolean;
  meldStart: boolean;
  winning: boolean;
}) {
  return (
    <span
      className={`yaku-reference__tile${
        groupStart ? " is-group-start" : ""
      }${meldStart ? " is-meld-start" : ""}${
        winning ? " is-winning" : ""
      }`}
      aria-hidden="true"
    >
      <span className="yaku-reference__tile-body">
        <span className="yaku-reference__tile-face">
          <img src={tileAssetPath(code, "jp")} alt="" />
        </span>
      </span>
    </span>
  );
}
