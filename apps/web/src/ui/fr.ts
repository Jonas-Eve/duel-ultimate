// Every French text of the interface (informal "tu"), in one module: no i18n library. Error
// texts live in `errors.fr.ts`. Names (cards, heroes, decks) are rendered by React as text and
// used in `aria-label`s, never as HTML.
import type { GameResult, Seat } from "../engine/types";

/**
 * Who is who, to word a result from the player's side. `you` is null when one person plays both
 * camps; `camp` then names a camp independently of its position on screen ("Camp 2 — Rempart").
 */
export interface Outcome {
  you: Seat | null;
  heroName(seat: Seat): string;
  camp(seat: Seat): string;
}

const other = (s: Seat): Seat => (s === "One" ? "Two" : "One");

/** Who a log line is about: `subject` opens a sentence ("Tu", "L'ordinateur"), `tonic` follows a preposition ("toi"). */
export interface Who {
  subject: string;
  tonic: string;
  you: boolean;
}
/** The verb in the second person for the human's seat ("tu invoques"), the third otherwise. */
const verb = (who: Who, third: string, second: string): string => (who.you ? second : third);
const damage = (n: number): string => `${n} dégât${n > 1 ? "s" : ""}`;
const cardCount = (n: number): string => `${n} carte${n > 1 ? "s" : ""}`;
/** A card name, or "une carte" when the view never gave it (never an id). */
const named = (name: string | null): string => name ?? "une carte";
const capital = (text: string): string => text.charAt(0).toUpperCase() + text.slice(1);

export const fr = {
  appTitle: "duel-ultimate",
  loadingEngine: "Chargement du moteur…",
  reload: "Recharger",

  homeHeading: "Choisis ton deck",
  modeLegend: "Mode",
  modeComputer: "Contre l'ordinateur",
  modeTwoCamps: "Je joue les deux camps",
  deckLegend: "Ton deck",
  campDeckLegend: (n: 1 | 2) => `Deck du camp ${n}`,
  gameRegion: "Partie",
  seedLabel: "Graine (optionnelle)",
  seedHelp: "Laisse vide pour une graine aléatoire (nombre entier de 0 à 4 294 967 295)",
  start: "Lancer la partie",
  dataVersion: (v: string) => `Données : ${v}`,
  buildVersion: (id: string) => `Version : ${id}`,
  opponentDeck: (name: string) => `L'adversaire joue : ${name}`,
  entropyUnavailable:
    "Ton appareil ne peut pas tirer de graine au hasard. Saisis une graine (nombre entier de 0 à 4 294 967 295) pour lancer la partie.",

  /** Two camps: the camp holding the main, named (replaces "À toi" / "À l'adversaire"). */
  turnOf: (label: string) => `Au tour de ${label}`,
  /** Two camps: a region named after its camp, so that the mirror stays unambiguous ("Camp 2 — Ruée : main"). */
  campRegion: (label: string, what: string) => `${label} : ${what}`,
  campConcedeTitle: (n: 1 | 2) => `Le camp ${n} abandonne ?`,
  yourTurn: "À toi",
  opponentTurn: "À l'adversaire",
  time: (n: number) => `Temps : ${n}`,
  maxTime: (n: number) => `max ${n}`,
  handCount: (n: number) => `Main : ${n}`,
  deckCount: (n: number) => `Pioche : ${n}`,
  health: (n: number) => `PV ${n}`,
  force: (n: number) => `Force ${n}`,
  attackCost: (n: number) => `Attaque ${n}`,
  cost: (n: number) => `Coût ${n}`,
  provocation: "Provocation",
  riposte: (n: number) => `Riposte ${n}`,
  noCreature: "Aucune créature",

  selected: "Sélectionnée",
  targetPossible: "Cible possible",
  targetUnavailable: "Cible non disponible",
  cancelSelection: "Annuler la sélection",
  attackTitle: "Confirmer l'attaque",
  attackSummary: (attacker: string, target: string) => `${attacker} attaque ${target}`,
  heroAsTarget: (name: string) => `le héros ${name}`,
  confirm: "Confirmer",
  cancel: "Annuler",
  emptyHand: "Aucune carte en main",

  /** Visible caption above the other camp's hand (two camps): the region itself is named by `campRegion`. */
  otherHandCaption: (label: string) => `Main de ${label} (non jouable)`,
  regionHero: "héros",
  regionBoard: "plateau",
  regionHand: "main",
  yourHero: "Ton héros",
  opponentHero: "Héros adverse",
  yourBoard: "Ton plateau",
  opponentBoard: "Plateau adverse",
  yourHand: "Ta main",
  timeAndToken: "Temps et jeton",
  actions: "Actions",
  seed: (n: number) => `Graine : ${n}`,
  aiSeed: (n: number) => `Graine de l'ordinateur : ${n}`,
  gameInfo: "Infos de partie",

  concede: "Abandonner",
  concedeTitle: "Abandonner la partie ?",
  keepPlaying: "Continuer",
  closeError: "Fermer",

  computerPlaying: "L'ordinateur joue…",
  computerPlayed: (n: number) => `L'ordinateur a joué ${n} action${n > 1 ? "s" : ""}. À toi de jouer.`,

  youWon: "Tu as gagné !",
  computerWon: "L'ordinateur a gagné.",
  replaySame: "Rejouer avec la même graine",
  replayNew: "Rejouer avec une nouvelle graine",
  backHome: "Retour à l'accueil",
  fatalTitle: "Partie interrompue",
  unknownCard: "Carte",

  close: "Fermer",
  logRegion: "Journal",
  logButton: "Journal",
  logTitle: "Journal",
  logLast: "Dernière ligne du journal",
  logFull: "Journal complet",
  logEmpty: "Aucune action pour le moment.",
  blockStart: "Début de partie",
  blockComputer: "Tour de l'ordinateur",
  blockYou: "Ton tour",
  discardButton: (n: number) => `Défausse (${n})`,
  discardTitle: (who: string) => `Défausse : ${who}`,
  discardEmpty: "Aucune carte détruite",
  destroyed: "Détruite",

  /** Journal lines. Names are plain text; a missing name reads "une carte", never an identifier. */
  log: {
    started: (w: Who, budget: number) => `La partie commence : ${w.subject} ${verb(w, "a", "as")} la main (temps ${budget}).`,
    effect: (w: Who, effect: string | null, cost: number) =>
      `${w.subject} ${verb(w, "utilise", "utilises")} ${effect ?? "un effet"} (coût ${cost}).`,
    drew: (w: Who, count: number, names: string[] | null) =>
      `${w.subject} ${verb(w, "pioche", "pioches")} ${cardCount(count)}${names === null ? "" : ` : ${names.join(", ")}`}.`,
    played: (w: Who, card: string | null, cost: number) => `${w.subject} ${verb(w, "invoque", "invoques")} ${named(card)} (coût ${cost}).`,
    attacked: (w: Who, card: string | null, cost: number) =>
      `${w.subject} ${verb(w, "attaque", "attaques")} avec ${named(card)} (coût ${cost}).`,
    heroDamaged: (hero: string, amount: number, left: number) => `Le héros ${hero} subit ${damage(amount)} (PV restants : ${left}).`,
    creatureDamaged: (card: string | null, amount: number, left: number) =>
      `${capital(named(card))} subit ${damage(amount)} (PV restants : ${left}).`,
    riposted: (card: string | null, amount: number, left: number) =>
      `${capital(named(card))} subit ${damage(amount)} de riposte (PV restants : ${left}).`,
    died: (card: string | null) => `${capital(named(card))} meurt.`,
    handPasses: (w: Who, budget: number | null) => `La main passe à ${w.tonic}${budget === null ? "" : ` (temps ${budget})`}.`,
    handKept: (w: Who, budget: number) => `${w.subject} ${verb(w, "garde", "gardes")} la main (temps restant ${budget}).`,
    won: (w: Who, why: string) => `${w.subject} ${verb(w, "gagne", "gagnes")} : ${why}`,
  },

  /** Title of the end screen. */
  winnerTitle: (winner: Seat, who: Outcome): string =>
    who.you === null ? `${who.camp(winner)} a gagné.` : winner === who.you ? fr.youWon : fr.computerWon,
  /** The winner as a predicate, for the consequence line. */
  winnerPhrase: (winner: Seat, who: Outcome): string =>
    who.you === null ? `${who.camp(winner)} gagne` : winner === who.you ? "Tu gagnes" : "L'ordinateur gagne",
} as const;

/** Why the game ended, from the loser's side. One wording, shared by the end screen and the consequence lines. */
export function endReason(result: GameResult, who: Outcome): string {
  const loser = other(result.winner);
  // The verb agrees with the subject: "Tu n'avais", "L'ordinateur n'avait".
  const subject =
    who.you === null
      ? { name: who.camp(loser), lacked: "n'avait", conceded: "a abandonné" }
      : loser === who.you
        ? { name: "Tu", lacked: "n'avais", conceded: "as abandonné" }
        : { name: "L'ordinateur", lacked: "n'avait", conceded: "a abandonné" };
  switch (result.reason) {
    case "hero_down":
      return `Le héros ${who.heroName(loser)} est tombé à 0 PV.`;
    case "exhaustion":
      return `${subject.name} ${subject.lacked} plus aucune action possible (épuisement).`;
    case "concede":
      return `${subject.name} ${subject.conceded}.`;
  }
}
