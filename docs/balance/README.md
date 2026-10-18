# Équilibrage : mesures et protocoles

Index des documents de mesure du simulateur (`duel-simulator`, Epic #120).

**Règle : une mesure = commande + graine + commit + date.** Un nombre sans ces quatre éléments
n'est pas une mesure : il ne se compare à rien et ne se reproduit pas.

- [`throughput.md`](throughput.md) : le débit du simulateur (parties par seconde), son protocole,
  la règle de comparaison entre machines, le seuil de régression et le tableau daté des mesures
  (#123) ; la ligne `aa1fdbc` est la référence gardante sous les règles par défaut (#173).
- [`duree.md`](duree.md) : le protocole et la feuille de relevé de la durée d'une partie jouée à la
  main (cible de 15 minutes humain contre humain, convertie en actions comparables au rapport du
  simulateur ; résultats consignés par la tâche suivante de #124).
- [`reports/`](reports/) : les rapports d'équilibrage consignés, un fichier par mesure
  (`<AAAA-MM-JJ>-<jeu>-<version>-<politique>-<variante>.md`). Le premier est la ligne de base du
  comportement historique : [`2026-10-17-starter-1.1.0-random-legacy.md`](reports/2026-10-17-starter-1.1.0-random-legacy.md).
  Politique gloutonne appariée à `random` (mêmes graines, #160) :
  [`2026-10-18-starter-1.1.0-greedy.md`](reports/2026-10-18-starter-1.1.0-greedy.md).
  Effet des options de règles de #127 (#173), `random` et `greedy` appariées :
  [`riposte-seule`](reports/2026-10-18-starter-1.1.0-riposte-seule.md),
  [`limite-seule`](reports/2026-10-18-starter-1.1.0-limite-seule.md),
  [`standard`](reports/2026-10-18-starter-1.1.0-standard.md) (le défaut du moteur).
- [`rules-options.md`](rules-options.md) : la synthèse de l'effet de chaque option de règles, isolément puis
  combinées (taux, durées, fins de partie), avec la mention que l'équilibre n'est garanti que sur les
  règles par défaut (#173).

**Règles historiques et règles par défaut (#127, #172).** Depuis #172 le défaut du moteur est
`RuleSet::standard()` (riposte du héros active, limite de plateau 5) : un lot sans option affiche
`rules: hero_riposte=on board_limit=5`. Les rapports consignés avant ce changement (dont la ligne de base
`random-legacy`) restent des mesures des **règles historiques** : ils ont été produits avec des options
**explicites** (`--hero-riposte off --board-limit none`, en-tête `rules: hero_riposte=off board_limit=none`) et
se rejouent à l'identique avec elles ; ils ne se comparent pas à un lot aux règles par défaut sans le dire.
Les rapports historiques ne sont pas réécrits : les mesures aux nouvelles règles sont dans les rapports
`standard`, `riposte-seule` et `limite-seule`, et la ligne de débit `aa1fdbc` de `throughput.md` (référence
gardante sous les règles par défaut, #173).

## Lire le rapport du simulateur

Format détaillé : `README.md` à la racine (« Format du rapport du simulateur »). Ce qui suit dit
comment en tirer un verdict.

- **Une section par politique.** Chaque politique rejoue les mêmes graines de moteur : les
  sections se comparent deux à deux (comparaison appariée). Cela vaut **entre politiques d'un même
  jeu de données**, pas entre deux versions du jeu (voir « Lire un diff »).
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

### Lire un diff (`--against`)

`duel-simulator 2000 --set starter@2.0.0 --against starter@1.1.0 --seed 1` imprime le diff seul,
« 2.0.0 contre 1.1.0 » : la référence (avant) est `--against`, le candidat (après) est `--set`,
delta = après - avant, en points (différence des pour-mille entiers affichés).

- **Deux échantillons indépendants.** La graine d'une partie dépend de l'indice de son matchup, et
  des données différentes changent de toute façon les parties : un delta de l'ordre de l'intervalle
  de Wilson (environ +-2,2 points à 2000 parties) est du **bruit d'échantillonnage**, pas un effet
  du jeu de données. Seul `--set X --against X` rejoue exactement les mêmes parties (tous les deltas
  valent 0). La ligne `NOTE: the two batches are independent samples` le rappelle chaque fois que
  les jeux diffèrent.
- **`significant`** : `yes` quand les intervalles de Wilson à 95 % de la part de A (paire croisée)
  ou du premier joueur (miroir) ne se chevauchent pas ; heuristique conservatrice (elle manque de
  petits effets réels, elle ne crie presque jamais au loup), jamais un test statistique complet.
- **`new` / `removed`** : paire (ou politique) présente d'un seul côté. L'appariement se fait par la
  clé `(deck A, deck B, type)`, pas par l'indice ; une paire croisée dont l'ordre des decks est
  inversé d'une version à l'autre apparaît `removed` + `new` (l'ordre de `decks.json` est canonique).
- **`NOTE:`** : un paramètre (graine, parties, plafond, règles, politiques) qui diffère rend les
  deltas non comparables ; jamais un échec, jamais un code de sortie.
- La synthèse reprend celle du rapport (pire paire, bande 45-55 %, premier joueur en miroir) sous la
  forme `avant -> après`.

### « Équilibré » n'est pas un test

« Équilibré » veut dire : **un fichier de `reports/`** (commande, graine, commit, date, sortie
verbatim) qui montre le jeu dans les seuils. Ce n'est jamais un test de CI : un test qui épingle
un taux casserait à chaque changement de règle voulu, et la CI ne lance pas de lots de milliers de
parties. Les tests du simulateur épinglent le **format** et les **compteurs d'un lot court**, pas
l'équilibre.

Aucune mesure de débit ne tourne en CI, et aucun seuil n'est épinglé dans un test.
