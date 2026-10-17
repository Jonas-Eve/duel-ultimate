# Équilibrage : mesures et protocoles

Index des documents de mesure du simulateur (`duel-simulator`, Epic #120).

**Règle : une mesure = commande + graine + commit + date.** Un nombre sans ces quatre éléments
n'est pas une mesure : il ne se compare à rien et ne se reproduit pas.

- [`reports/`](reports/) : les rapports d'équilibrage consignés, un fichier par mesure
  (`<AAAA-MM-JJ>-<jeu>-<version>-<politique>-<variante>.md`). Le premier est la ligne de base du
  comportement historique : [`2026-10-17-starter-1.1.0-random-legacy.md`](reports/2026-10-17-starter-1.1.0-random-legacy.md).

## Lire le rapport du simulateur

Format détaillé : `README.md` à la racine (« Format du rapport du simulateur »). Ce qui suit dit
comment en tirer un verdict.

- **Une section par politique.** Chaque politique rejoue les mêmes graines de moteur : les
  sections se comparent deux à deux (comparaison appariée).
- **Tableau 1** : taux de victoire de chaque deck, part des parties gagnées par le siège One,
  part du premier joueur, parties interrompues. Chaque taux porte son intervalle de Wilson à 95 %.
  **Un taux hors de la bande avec un intervalle qui chevauche la bande n'est pas un déséquilibre
  démontré** : sur 2000 parties, l'intervalle est d'environ +-2,2 points. Lire l'intervalle, pas
  seulement le chiffre.
- **Tableau 2** : durée moyenne, médiane et 90e centile (en actions, **parties terminées
  seulement**), parts des fins (PV du héros, épuisement, abandon) et marqueur `vs target`.
- **Parties interrompues** : une ligne `WARNING interrupted` les nomme, jamais supprimée. Taux et
  durées les excluent : elles sont censurées à droite (une partie coupée au plafond aurait été
  longue), donc un `WARNING` rend les durées **optimistes**.

### Seuils (Epic #120) et constantes

Toutes les décisions sont des comparaisons entières exactes, sur l'estimation ponctuelle et les
parties terminées ; les seuils sont des constantes publiques de `duel_simulator`.

| Verdict | Règle | Constante |
|---|---|---|
| Paire croisée dans la bande | taux du deck A entre 45 % et 55 %, **bornes incluses** | `BAND_LOW_PERCENT`, `BAND_HIGH_PERCENT` |
| Premier joueur en miroir | au plus 57 % des parties terminées | `MIRROR_FIRST_PLAYER_MAX_PERCENT` |
| Alerte « épuisement dominant » | épuisement **strictement** au-dessus de 70 % des parties terminées | `EXHAUSTION_ALERT_PERCENT` |
| Cible de durée | moyenne entre 40 et 60 actions : **provisoire**, #124 la remplace | `DURATION_TARGET_ACTIONS` |

- La colonne « first player » d'un matchup **croisé** n'a pas de verdict : elle y est confondue avec
  le deck (Ruée commence toujours). Le signal du premier joueur est celui des miroirs.
- La **pire paire** et la **dispersion** (min, moyenne, max de l'écart à 50 %, en points) ne
  portent que sur les paires croisées : un miroir est à 50 % par symétrie.
- Une alerte, une paire hors bande ou un miroir au-dessus de 57 % sont des **lignes d'information** :
  le programme sort avec le code 0. Seul un rapport qui contredit ses propres invariants sort avec
  le code 1, sans rien écrire sur stdout.

### « Équilibré » n'est pas un test

« Équilibré » veut dire : **un fichier de `reports/`** (commande, graine, commit, date, sortie
verbatim) qui montre le jeu dans les seuils. Ce n'est jamais un test de CI : un test qui épingle
un taux casserait à chaque changement de règle voulu, et la CI ne lance pas de lots de milliers de
parties. Les tests du simulateur épinglent le **format** et les **compteurs d'un lot court**, pas
l'équilibre.
