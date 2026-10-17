// The card-art lookup, given by `App` (the only reader of `data/art`) and read by the tiles
// (#89). Without a provider no card has an image: the placeholder stays.
import { createContext, useContext, type ReactNode } from "react";
import type { ArtLookup } from "../game/art";

const noArt: ArtLookup = () => undefined;
const ArtContext = createContext<ArtLookup>(noArt);

export function ArtProvider({ lookup, children }: { lookup: ArtLookup; children: ReactNode }) {
  return <ArtContext.Provider value={lookup}>{children}</ArtContext.Provider>;
}

export function useArtLookup(): ArtLookup {
  return useContext(ArtContext);
}
