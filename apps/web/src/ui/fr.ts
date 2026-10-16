// Every French text of the interface (informal "tu"), in one module: no i18n library. Error
// texts live in `errors.fr.ts`. Names (cards, heroes, decks) are rendered by React as text and
// used in `aria-label`s, never as HTML.
import type { GameResult, Seat } from "../engine/types";

/** Who is who, to word a result from the player's side. `you` is null when one person plays both camps. */
export interface Outcome {
  you: Seat | null;
  heroName(seat: Seat): string;
}

const other = (s: Seat): Seat => (s === "One" ? "Two" : "One");
const camp = (s: Seat): string => `Le camp ${s === "One" ? 1 : 2}`;

export const fr = {
  appTitle: "duel-ultimate",
  loadingEngine: "Chargement du moteur…",
  reload: "Recharger",

  homeHeading: "Choisis ton deck",
  deckLegend: "Ton deck",
  gameRegion: "Partie",
  seedLabel: "Graine (optionnelle)",
  seedHelp: "Laisse vide pour une graine aléatoire (nombre entier de 0 à 4 294 967 295)",
  start: "Lancer la partie",
  dataVersion: (v: string) => `Données : ${v}`,
  opponentDeck: (name: string) => `L'adversaire joue : ${name}`,
  entropyUnavailable:
    "Ton appareil ne peut pas tirer de graine au hasard. Saisis une graine (nombre entier de 0 à 4 294 967 295) pour lancer la partie.",

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
  emptyHand: "Aucune carte en main",

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

  /** Title of the end screen. */
  winnerTitle: (winner: Seat, you: Seat | null): string =>
    you === null ? `${camp(winner)} a gagné.` : winner === you ? fr.youWon : fr.computerWon,
  /** The winner as a predicate, for the consequence line. */
  winnerPhrase: (winner: Seat, you: Seat | null): string =>
    you === null ? `${camp(winner)} gagne` : winner === you ? "Tu gagnes" : "L'ordinateur gagne",
} as const;

/** Why the game ended, from the loser's side. One wording, shared by the end screen and the consequence lines. */
export function endReason(result: GameResult, who: Outcome): string {
  const loser = other(result.winner);
  // The verb agrees with the subject: "Tu n'avais", "L'ordinateur n'avait".
  const subject =
    who.you === null
      ? { name: camp(loser), lacked: "n'avait", conceded: "a abandonné" }
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
