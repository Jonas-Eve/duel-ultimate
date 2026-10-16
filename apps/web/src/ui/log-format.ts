// The readable log: pure, no React. It turns the controller's `StepRecord`s (events as ids plus
// the names and current health read from the views taken after each action) into French lines.
//
// What a line may name is decided upstream, by what a record holds: against the computer the
// records are written from the human seat's view only, so the formatter has NO way to name a
// card the computer drew ("L'ordinateur pioche 2 cartes."). Do not add a global id -> name table
// here: it would name a drawn card after the fact. A display choice, not a security guarantee.
import { eventKind, type EngineEvent, type EventKind, type Seat } from "../engine/types";
import type { Snapshot } from "../game/controller";
import type { StepRecord } from "../game/records";
import { endReason, fr, type Who } from "./fr";

export interface LogContext {
  mode: "computer" | "two-camps";
  /** The seat addressed as "tu" (always One against the computer), null when one person plays both camps. */
  youSeat: "One" | null;
  labels: Record<Seat, string>;
}

export interface LogLine {
  /** Unique and stable: `<seq>-<index of the first event the line is made of>`. */
  id: string;
  seq: number;
  segment: StepRecord["by"];
  actor: Seat;
  text: string;
  tone: "info" | "damage" | "death" | "turn" | "end";
}

export interface LogBlock {
  key: string;
  title: string;
  segment: StepRecord["by"];
  actor: Seat;
  lines: LogLine[];
}

/** Computer: "Toi" / "L'ordinateur". Two camps: "Camp 1 — Ruée" / "Camp 2 — Rempart". Deck names come from the snapshot. */
export function logContext(snapshot: Snapshot): LogContext {
  const camp = (seat: Seat) => `Camp ${seat === "One" ? 1 : 2} — ${snapshot.players[seat].name}`;
  return {
    mode: snapshot.mode,
    youSeat: snapshot.humanSeat,
    labels:
      snapshot.mode === "computer" ? { One: "Toi", Two: "L'ordinateur" } : { One: camp("One"), Two: camp("Two") },
  };
}

const never = (x: never): never => {
  throw new Error(`unknown engine event: ${JSON.stringify(x)}`);
};

type Payload<K extends EventKind> = Extract<EngineEvent, Record<K, unknown>>[K];
const body = <K extends EventKind>(event: EngineEvent, kind: K): Payload<K> => (event as unknown as Record<K, Payload<K>>)[kind];

function format(record: StepRecord, ctx: LogContext): LogLine[] {
  const who = (seat: Seat): Who =>
    seat === ctx.youSeat ? { subject: "Tu", tonic: "toi", you: true } : { subject: ctx.labels[seat], tonic: ctx.labels[seat], you: false };
  const cardName = (id: number): string | null => record.cards[id]?.name ?? null;
  const cardHealth = (id: number): number => record.cards[id]?.health ?? 0;
  const outcome = { you: ctx.youSeat, heroName: (seat: Seat) => record.hero[seat].name };

  const lines: LogLine[] = [];
  const push = (index: number, text: string, tone: LogLine["tone"]) =>
    lines.push({ id: `${record.seq}-${index}`, seq: record.seq, segment: record.by, actor: record.actor, text, tone });

  const events = record.events;
  for (let i = 0; i < events.length; i++) {
    const e = events[i];
    const kind = eventKind(e);
    switch (kind) {
      case "game_started": {
        const p = body(e, kind);
        push(i, fr.log.started(who(p.first), p.budget), "info");
        break;
      }
      case "effect_activated": {
        const p = body(e, kind);
        push(i, fr.log.effect(who(p.player), record.hero[p.player].effects[p.effect]?.name ?? null, p.cost), "info");
        break;
      }
      case "drew": {
        // Consecutive draws of one player are one line. Names only when every card has one.
        const first = body(e, kind);
        const ids = [first.card];
        while (i + 1 < events.length && eventKind(events[i + 1]) === "drew" && body(events[i + 1], "drew").player === first.player) {
          i += 1;
          ids.push(body(events[i], "drew").card);
        }
        const names = ids.map(cardName);
        const known = names.every((n): n is string => n !== null);
        push(i - ids.length + 1, fr.log.drew(who(first.player), ids.length, known ? (names as string[]) : null), "info");
        break;
      }
      case "played": {
        const p = body(e, kind);
        push(i, fr.log.played(who(p.player), cardName(p.card), p.cost), "info");
        break;
      }
      case "attacked": {
        const p = body(e, kind);
        push(i, fr.log.attacked(who(p.player), cardName(p.attacker), p.cost), "info");
        break;
      }
      case "damaged": {
        const p = body(e, kind);
        push(i, fr.log.heroDamaged(record.hero[p.player].name, p.amount, record.hero[p.player].health), "damage");
        break;
      }
      case "creature_damaged": {
        const p = body(e, kind);
        push(i, fr.log.creatureDamaged(cardName(p.creature), p.amount, cardHealth(p.creature)), "damage");
        break;
      }
      case "riposted": {
        const p = body(e, kind);
        push(i, fr.log.riposted(cardName(p.creature), p.amount, cardHealth(p.creature)), "damage");
        break;
      }
      case "died": {
        push(i, fr.log.died(cardName(body(e, kind).creature)), "death");
        break;
      }
      case "token_moved": {
        const p = body(e, kind);
        const next = events[i + 1];
        if (next !== undefined && eventKind(next) === "active_player_changed") {
          i += 1;
          push(i - 1, fr.log.handPasses(who(body(next, "active_player_changed").player), p.budget), "turn");
        } else {
          push(i, fr.log.handKept(who(p.player), p.budget), "turn");
        }
        break;
      }
      case "active_player_changed": {
        push(i, fr.log.handPasses(who(body(e, kind).player), null), "turn");
        break;
      }
      case "game_won": {
        const p = body(e, kind);
        push(i, fr.log.won(who(p.winner), endReason(p, outcome)), "end");
        break;
      }
      default:
        return never(kind);
    }
  }
  return lines;
}

/** One French line per event (some events merge), in order, from the point of view `ctx` describes. */
export function formatRecords(records: StepRecord[], ctx: LogContext): LogLine[] {
  return records.flatMap((r) => format(r, ctx));
}

/**
 * The lines grouped in titled blocks: consecutive records of one author. The key is `by` for the
 * setup and the computer; for the human it is (`by`, `actor`), because with two camps `by` is
 * always "human" and only `actor` tells the camps apart.
 */
export function formatBlocks(records: StepRecord[], ctx: LogContext): LogBlock[] {
  const blocks: LogBlock[] = [];
  for (const record of records) {
    const key = record.by === "human" ? `human-${record.actor}` : record.by;
    const lines = format(record, ctx);
    const last = blocks[blocks.length - 1];
    if (last !== undefined && last.key === key) {
      last.lines.push(...lines);
      continue;
    }
    const title =
      record.by === "setup"
        ? fr.blockStart
        : record.by === "computer"
          ? fr.blockComputer
          : ctx.youSeat === record.actor
            ? fr.blockYou
            : ctx.labels[record.actor];
    blocks.push({ key, title, segment: record.by, actor: record.actor, lines });
  }
  return blocks;
}
