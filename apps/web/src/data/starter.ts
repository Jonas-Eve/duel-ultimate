export const DATA_VERSION = "";
export const CARDS_JSON = "";
export const DECKS_JSON = "";
export interface DeckInfo {
  id: string;
  name: string;
}
export const DECKS: DeckInfo[] = [];
export const parseDeckList = (_json: string): DeckInfo[] => [];
