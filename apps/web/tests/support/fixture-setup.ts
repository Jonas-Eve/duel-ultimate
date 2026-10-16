import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { repoRoot } from "./paths";

// TEST-ONLY and provisional: builds the JSON `GameSetup` of the `fixture` set by hand, as
// `crates/engine-wasm/tests/e2e_duel.rs` does on the Rust side. Story #52 replaces it with
// a setup built from the data inside the binding; delete this file then.

type Json = Record<string, unknown>;

function readJson<T>(name: string): T {
  return JSON.parse(readFileSync(resolve(repoRoot, "data/fixture/1.0.0", name), "utf8")) as T;
}

/** The fixture's first deck against its second, ready for `new Game(setup, seed)`. */
export function fixtureSetup(): string {
  const cards = readJson<Json[]>("cards.json");
  const decks = readJson<{ hero: Json; cards: string[] }[]>("decks.json");
  const byId = new Map(cards.map((c) => [c.id as string, c]));
  const side = (i: number) => ({
    hero: decks[i].hero,
    cards: decks[i].cards.map((id) => {
      const card = byId.get(id);
      if (!card) throw new Error(`unknown fixture card ${id}`);
      // `CreatureStats` denies unknown fields: only the stats go through.
      const { kind: _kind, id: _id, name: _name, ...stats } = card;
      return stats;
    }),
  });
  return JSON.stringify({ one: side(0), two: side(1), data_versions: ["fixture@1.0.0"] });
}
