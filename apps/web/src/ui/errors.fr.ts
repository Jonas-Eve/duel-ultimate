import type { ErrorCode } from "../engine/errors";

/** French text of every error code. The English engine messages are never displayed. */
export const ERROR_TEXT: Record<ErrorCode, string> = {
  game_over: "La partie est terminée.",
  card_not_in_hand: "Cette carte n'est pas dans votre main. Choisissez une carte de votre main.",
  unknown_effect: "Ce pouvoir du héros n'existe pas.",
  effect_not_applicable: "Ce pouvoir ne peut pas être utilisé maintenant : la pioche est vide.",
  attacker_not_on_board: "Cette créature n'est pas sur votre plateau. Choisissez une créature en jeu pour attaquer.",
  target_not_on_board: "Cette cible n'est pas une créature du plateau adverse.",
  target_forbidden_by_provocation: "Une créature adverse avec provocation doit être attaquée en premier.",
  invalid_setup: "Les données de la partie sont invalides. Rechargez la page.",
  invalid_json: "La demande n'a pas pu être lue. Rechargez la page et recommencez.",
  unknown_deck: "Ce deck n'existe pas. Choisissez un deck de la liste.",
  input_too_large: "La demande est trop volumineuse pour être traitée.",
  invalid_seat: "Ce joueur n'existe pas dans la partie.",
  not_active_seat: "Ce n'est pas le tour de ce camp de jouer dans la partie.",
  invalid_policy: "Ce niveau d'ordinateur n'existe pas pour cette partie.",
  serialization_failed: "La partie ne peut plus être affichée. Lancez une nouvelle partie.",
  invalid_seed: "La graine doit être un nombre entier entre 0 et 4 294 967 295.",
  wasm_load_failed: "Le moteur de jeu n'a pas pu être chargé. Vérifiez votre connexion puis rechargez la page.",
  computer_stuck: "L'ordinateur n'a pas pu jouer son tour. Lancez une nouvelle partie.",
  computer_chain_limit: "L'ordinateur a joué trop d'actions de suite. Lancez une nouvelle partie.",
  internal_error: "Une erreur interne s'est produite. Lancez une nouvelle partie.",
};

/** Never throws: an unknown code gets the generic text. */
export function messageFor(code: string): string {
  return Object.hasOwn(ERROR_TEXT, code)
    ? ERROR_TEXT[code as ErrorCode]
    : `Une erreur inattendue s'est produite (code : ${code}).`;
}
