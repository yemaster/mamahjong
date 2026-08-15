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
import { projectAboutTemplate } from "./projectAboutData";

export function YakuReferencePage({
  onBack,
}: {
  onBack: () => void;
}) {
  const [family, setFamily] = useState<ReferenceFamily | null>(null);
  const title = referenceTitle(family);

  return (
    <section className="yaku-reference-page">
      <YakuReferenceHeader
        title={title}
        onBack={family === null ? onBack : () => setFamily(null)}
        backLabel={family === null ? "返回大厅" : "返回帮助主页"}
      />
      <YakuReferenceBrowser family={family} onFamilyChange={setFamily} />
    </section>
  );
}

export function YakuReferenceModal({
  onClose,
}: {
  onClose: () => void;
}) {
  const [family, setFamily] = useState<ReferenceFamily | null>(null);

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
        <YakuReferenceHeader
          title={referenceTitle(family)}
          onBack={family === null ? undefined : () => setFamily(null)}
          backLabel="返回帮助主页"
          onClose={onClose}
        />
        <YakuReferenceBrowser family={family} onFamilyChange={setFamily} />
      </section>
    </div>
  );
}

function YakuReferenceHeader({
  title,
  onBack,
  backLabel = "返回",
  onClose,
}: {
  title: string;
  onBack?: () => void;
  backLabel?: string;
  onClose?: () => void;
}) {
  return (
    <header className="yaku-reference__header">
      {onBack && (
        <button
          type="button"
          onClick={onBack}
          aria-label={backLabel}
          title={backLabel}
        >
          <ArrowLeft aria-hidden="true" />
        </button>
      )}
      <h1>{title}</h1>
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
 * 帮助页分两级：先选帮助类别，再进入对应内容。
 *
 * 不做成并排的一排 tab，是因为两家的内容从数据到画法都不共用，摆在同一排上像是
 * 同一份东西的两个筛选项。分成两级之后，进来先看到的是「有哪几种麻将」，
 * 里面那层 tab 就明确只属于当前这一种。
 */
const REFERENCE_FAMILIES = [
  {
    key: "riichi",
    label: "立直麻将",
    summary: "经典四人 / 三人立直麻将。",
    action: "查看规则 →",
  },
  {
    key: "impact",
    label: "冲击麻将",
    summary: "宁波余慈地区冲击麻将。",
    action: "查看规则 →",
  },
  {
    key: "about",
    label: "关于本项目",
    summary: "查看项目介绍与更新日志。",
    action: "查看内容 →",
  },
] as const;

type ReferenceFamily = (typeof REFERENCE_FAMILIES)[number]["key"];

function referenceTitle(family: ReferenceFamily | null): string {
  if (family === null) return "帮助";
  if (family === "about") return "关于本项目";
  const label = REFERENCE_FAMILIES.find((entry) => entry.key === family)?.label;
  return `${label ?? "麻将"}-帮助`;
}

function YakuReferenceBrowser({
  family,
  onFamilyChange,
}: {
  family: ReferenceFamily | null;
  onFamilyChange: (family: ReferenceFamily | null) => void;
}) {

  if (family === null) {
    return (
      <div className="yaku-reference__browser">
        <ul className="yaku-reference__families">
          {REFERENCE_FAMILIES.map((entry) => (
            <li key={entry.key}>
              <button type="button" onClick={() => onFamilyChange(entry.key)}>
                <h2>{entry.label}</h2>
                <p>{entry.summary}</p>
                <span aria-hidden="true">{entry.action}</span>
              </button>
            </li>
          ))}
        </ul>
      </div>
    );
  }

  return (
    <div className="yaku-reference__browser">
      {family === "riichi" && <RiichiReference />}
      {family === "impact" && <ImpactReference />}
      {family === "about" && <ProjectAbout />}
    </div>
  );
}

function ProjectAbout() {
  return (
    <main
      className="project-about"
      role="region"
      aria-label="项目介绍与更新日志"
    >
      {projectAboutTemplate.sections.map((section) => (
        <section className="project-about__section" key={section.title}>
          <h2>{section.title}</h2>
          {section.paragraphs.map((paragraph, paragraphIndex) => (
            <p key={paragraphIndex}>{paragraph}</p>
          ))}
        </section>
      ))}

      <section className="project-about__section">
        <h2>更新日志</h2>
        <ol className="project-about__changelog">
          {projectAboutTemplate.changelog.map((entry) => (
            <li key={`${entry.version}-${entry.date}`}>
              <header>
                <strong>{entry.version}</strong>
                <time>{entry.date}</time>
              </header>
              <ul>
                {entry.changes.map((change, changeIndex) => (
                  <li key={changeIndex}>{change}</li>
                ))}
              </ul>
            </li>
          ))}
        </ol>
      </section>
    </main>
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
        {activeTab === "基本规则"
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
        {activeTab === "记分" && (
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
