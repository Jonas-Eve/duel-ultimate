import { Fragment, useId, type ReactNode } from "react";
import type { CardView, EffectLine } from "../engine/types";
import type { DeckEmblem } from "../game/controller";
import { BackArrowIcon, DetailArt, EmblemChip, HeartIcon, HourglassIcon, StarIcon } from "./card-icons";
import { cardEffects, effectLineLabel, effectLineParts } from "./card-effects";
import { fr } from "./fr";

interface CardDetailProps {
  card: CardView;
  emblem?: DeckEmblem;
  deckName: string;
  /** Id of the hidden text that sums the card up for a screen reader (the sheet's `aria-describedby`). */
  summaryId: string;
  /** Id of the deck symbol, which names the deck (listed in the sheet's `aria-describedby`). */
  deckId?: string;
  /** Own board unit (#106): the attack line becomes a button; the rest of the detail stays read-only. */
  attack?: AttackLineProps;
}

export interface AttackLineProps {
  /** An engine preview of an attack by this creature exists (never computed here). */
  available: boolean;
  /** Why the line is inactive, shown as text. */
  reason: string;
  onAttack(): void;
}

/**
 * One effect line: the lead, the cost with its hourglass between the parentheses (activated lines), then each
 * operation in a span that carries its polarity (a border, and the text mark "Malus" for a malus), then the spoken
 * addition. The text content is `effectLineText(line).visible + hidden`, nothing else.
 */
function EffectText({ line }: { line: EffectLine }) {
  const { lead, cost, ops, hidden } = effectLineParts(line);
  return (
    <>
      <b className="card-detail-effect-name">{lead}</b>
      {cost !== undefined && (
        <>
          {" "}
          (<HourglassIcon size={11} />
          {cost})
        </>
      )}{" "}
      :{" "}
      {ops.map((op, i) => (
        <Fragment key={i}>
          {i > 0 && " ; "}
          <span className={`effect-op effect-polarity-${op.polarity}`}>
            {op.text}
            {op.polarity === "malus" && (
              <>
                {" \u2014 "}
                <span className="effect-mark">{fr.malus}</span>
              </>
            )}
          </span>
        </Fragment>
      ))}
      <span className="sr-only">{hidden}</span>
    </>
  );
}

export function EffectLineView({ line }: { line: EffectLine }) {
  return (
    <p className="card-detail-effect">
      <EffectText line={line} />
    </p>
  );
}

/**
 * The attack line as a button (>= 44 px): same words as the read-only line, named by the single
 * `effectLineLabel`. Without an engine preview it is `aria-disabled` (still focusable) and the reason is text.
 */
function AttackLineButton({ line, attack }: { line: EffectLine; attack: AttackLineProps }) {
  const reasonId = useId();
  return (
    <div className="card-detail-attack">
      <button
        type="button"
        className="btn card-detail-effect card-detail-effect-button"
        aria-label={effectLineLabel(line)}
        aria-disabled={attack.available ? undefined : true}
        aria-describedby={attack.available ? undefined : reasonId}
        onClick={attack.available ? attack.onAttack : undefined}
      >
        <EffectText line={line} />
      </button>
      {!attack.available && (
        <p id={reasonId} className="play-reason">
          {attack.reason}
        </p>
      )}
    </div>
  );
}

const capital = (text: string): string => text.charAt(0).toUpperCase() + text.slice(1);

/**
 * What a screen reader hears after the card's name: cost, values (riposte at 0 too; the word "Sort" for a spell,
 * which has none; no attack for an immobile unit), the tags, every effect line, every passive effect.
 */
export function spokenSummary(card: CardView): string {
  const { active, passive, values } = cardEffects(card);
  const figures = [
    values.attack === null ? null : `${fr.attackWord} ${values.attack}`,
    values.riposte === null ? null : `${fr.riposteWord} ${values.riposte}`,
    values.health === null ? null : `${fr.lifeWord} ${values.health}`,
  ].filter((f): f is string => f !== null);
  return [
    `${fr.detailCostCaption} ${values.cost} ${fr.detailTimeWord}.`,
    card.kind === "spell" ? `${fr.spell}.` : `${capital(figures.join(", "))}.`,
    ...(card.tags === undefined || card.tags.length === 0 ? [] : [`${fr.detailTags} : ${card.tags.join(", ")}.`]),
    ...active.map((l) => `${effectLineLabel(l)}.`),
    ...passive.map((k) => `${k.label} : ${k.explanation}`),
  ].join(" ");
}

function Value({ caption, icon, children }: { caption: string; icon: ReactNode; children: number }) {
  return (
    <span className="card-detail-value">
      <span className="card-detail-caption">{caption}</span>
      <span className="card-detail-number">
        {icon}
        {children}
      </span>
    </span>
  );
}

/**
 * The read-only face of a card at reading size: big cost, name, deck symbol, art slot, effects zone,
 * then attack and riposte on the left and the current life on the right. Every figure comes from the
 * card's view (shown, never computed).
 */
export function CardDetail({ card, emblem, deckName, summaryId, deckId, attack }: CardDetailProps) {
  const { active, passive, values } = cardEffects(card);
  return (
    <div className="card-detail">
      <p id={summaryId} hidden>
        {spokenSummary(card)}
      </p>
      <div className="card-detail-head">
        <span className="card-detail-cost">
          <HourglassIcon size={15} />
          <span className="sr-only">{fr.detailCostCaption} </span>
          <b>{values.cost}</b>
          <small> {fr.detailTimeWord}</small>
        </span>
        <span className="card-detail-name" aria-hidden="true">
          {card.name ?? fr.unknownCard}
        </span>
        {emblem !== undefined && <EmblemChip emblem={emblem} size={36} label={fr.detailDeck(deckName)} id={deckId} />}
      </div>
      <DetailArt name={card.name} />
      <div className="card-detail-effects" role="region" aria-label={fr.detailEffects} tabIndex={0}>
        {active.map((line, i) =>
          // Only the normal attack (no `index`, an attack operation) is a button; the others are read-only.
          attack !== undefined && line.index === undefined && line.ops[0]?.op === "attack" ? (
            <AttackLineButton key={i} line={line} attack={attack} />
          ) : (
            <EffectLineView key={i} line={line} />
          ),
        )}
        {passive.map((k) => (
          <p key={k.key} className="card-detail-passive">
            <b>{k.label}</b> : {k.explanation}
          </p>
        ))}
        {card.tags !== undefined && card.tags.length > 0 && (
          // React text nodes only: a tag is data, never markup.
          <p className="card-detail-tags">
            {fr.detailTags} : {card.tags.join(", ")}
          </p>
        )}
      </div>
      {card.kind !== "spell" && (
        <div className="card-detail-bottom">
          <div className="card-detail-left">
            {values.attack !== null && (
              <Value caption={fr.detailAttackCaption} icon={<StarIcon size={20} />}>
                {values.attack}
              </Value>
            )}
            {values.riposte !== null && (
              <Value caption={fr.detailRiposteCaption} icon={<BackArrowIcon size={20} />}>
                {values.riposte}
              </Value>
            )}
          </div>
          {values.health !== null && (
            <Value caption={fr.detailLifeCaption} icon={<HeartIcon size={22} />}>
              {values.health}
            </Value>
          )}
        </div>
      )}
    </div>
  );
}
