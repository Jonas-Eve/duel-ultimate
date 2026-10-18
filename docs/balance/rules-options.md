# Effet des options de règles de #127

Mesures de l'effet de chaque option de règles, **isolément** puis combinées (story #127, tâche #173,
Epic #120). Elles documentent le choix des défauts pour #135 et servent d'entrée à #128 et #133.

> **L'équilibre n'est garanti que sur les règles par défaut du moteur (`RuleSet::standard()`).
> Les variantes où une option est désactivée sont testées pour la validité du moteur, pas
> équilibrées.** Les chiffres ci-dessous décrivent des effets, pas des objectifs atteints.

**Une mesure = commande + graine + commit + date.** Toutes : commit `aa1fdbc` (`main` après #170,
#171, #172), jeu `starter@1.1.0`, graine `1`, 2000 parties par matchup, politiques `random,greedy` dans
le même lot (mêmes graines moteur, seule la politique change), options de règles **explicites**,
2026-10-18, Intel Xeon 2,10 GHz, 4 coeurs, rustc 1.97.0.

| Combinaison | Options | Rapport (sortie verbatim) |
|---|---|---|
| `legacy` | `--hero-riposte off --board-limit none` | [`2026-10-18-starter-1.1.0-greedy.md`](reports/2026-10-18-starter-1.1.0-greedy.md) (et section `random` : [`2026-10-17-starter-1.1.0-random-legacy.md`](reports/2026-10-17-starter-1.1.0-random-legacy.md)) |
| `riposte seule` | `--hero-riposte on --board-limit none` | [`2026-10-18-starter-1.1.0-riposte-seule.md`](reports/2026-10-18-starter-1.1.0-riposte-seule.md) |
| `limite seule` | `--hero-riposte off --board-limit 5` | [`2026-10-18-starter-1.1.0-limite-seule.md`](reports/2026-10-18-starter-1.1.0-limite-seule.md) |
| `standard` | `--hero-riposte on --board-limit 5` | [`2026-10-18-starter-1.1.0-standard.md`](reports/2026-10-18-starter-1.1.0-standard.md) |

Commande type (remplacer les deux options) :

```
target/release/duel-simulator 2000 --set starter@1.1.0 --seed 1 --hero-riposte <on|off> --board-limit <5|none> --policy random,greedy
```

## Vérifications (non-régression et déterminisme)

- **Déterminisme** : chacune des quatre combinaisons a été lancée **deux fois**, sorties identiques
  octet pour octet, code de sortie 0 (`Report::check` s'exécute dans le binaire). Durées de lot en
  release : `legacy` 13,2 et 13,7 s, `limite seule` 12,9 et 13,1 s, `riposte seule` 62,2 et 61,0 s,
  `standard` 55,6 et 55,7 s, très en dessous du plafond de 10 minutes de `throughput.md`.
- **Non-régression `legacy`** : la sortie rejouée de `--hero-riposte off --board-limit none` est
  **identique octet pour octet** au bloc « Sortie (verbatim) » de `2026-10-18-starter-1.1.0-greedy.md`
  (38 lignes) : le travail de #170 à #172 ne change rien quand les deux options sont désactivées.
- **Défaut = `standard`** : le lot **sans aucune option** (en-tête `rules: hero_riposte=on
  board_limit=5`) est identique octet pour octet au lot `standard` explicite.
- `--against` (#154) compare deux **jeux de données** avec les mêmes règles des deux côtés : il ne peut
  pas opposer deux jeux d'options sur le même jeu. Les écarts ci-dessous sont donc relevés dans les
  rapports ; ce sont des échantillons de 2000 parties dont l'intervalle de Wilson est d'environ
  +-2 points (pour un taux moyen) : **un écart plus petit est du bruit**. Comme les graines moteur sont
  communes, les lots restent appariés pour les décisions identiques, sans que cela dispense de lire
  l'intervalle.

## Ce que la limite ne mesure pas

La limite de plateau (une place sur 5) change le **prix des bâtiments** (une carte qui reste sur le plateau
occupe une place). Cet effet est **hors mesure ici** : il relève de la valeur de carte, #133.

## Politique `random`

Taux de la 1re colonne : victoires de Ruée (deck A, siège One) contre Rempart, avec intervalle de Wilson à
95 %. Miroirs : part de victoires du **premier joueur**. Durées en actions, parties terminées
seulement (moyenne / médiane / p90). Fins : `hero_down` (PV du héros), `exhaustion` (épuisement), abandon
toujours à 0 %. Parties interrompues : **0 dans tous les lots**.

| Combinaison | `rush vs bulwark` (Ruée) | miroir `rush` (1er joueur) | miroir `bulwark` (1er joueur) |
|---|---|---|---|
| `legacy` | 97,2 % [96,4-97,8] | 62,0 % [59,8-64,1] | 62,4 % [60,3-64,5] |
| `riposte seule` | 99,1 % [98,6-99,4] | 59,9 % [57,7-62,0] | 58,7 % [56,5-60,8] |
| `limite seule` | 97,2 % [96,4-97,9] | 62,9 % [60,8-65,0] | 58,9 % [56,7-61,0] |
| `standard` | 99,2 % [98,8-99,5] | 60,5 % [58,4-62,7] | 57,0 % [54,9-59,2] |

| Combinaison | Durée `rush vs bulwark` | Durée miroir `rush` | Durée miroir `bulwark` | Fins |
|---|---|---|---|---|
| `legacy` | 41,9 / 40 / 65 | 37,2 / 35 / 59 | 86,3 / 84 / 128 | 100 % `hero_down` partout |
| `riposte seule` | 47,8 / 46 / 69 | 43,3 / 42 / 63 | 92,8 / 90 / 131 | 100 % `hero_down` partout |
| `limite seule` | 39,7 / 38 / 61 | 36,6 / 35 / 57 | 81,8 / 79 / 121 | 100 % `hero_down` partout |
| `standard` | 46,5 / 45 / 65 | 43,2 / 41 / 63 | 88,8 / 86 / 123 | 100 % `hero_down` partout |

Lecture (isolément, contre `legacy`) :

- **Riposte seule** : les parties s'allongent d'environ 6 actions en moyenne dans les deux premiers matchups
  (41,9 vers 47,8 ; 37,2 vers 43,3) et de 6,5 dans le miroir `bulwark` ; Ruée passe de 97,2 à 99,1 %
  (les intervalles ne se chevauchent pas, mais l'effet est de l'ordre de 2 points sur un taux déjà
  saturé). L'avantage du premier joueur recule un peu (59,9 et 58,7 % ; écarts de l'ordre de
  l'intervalle).
- **Limite seule** : les parties raccourcissent (41,9 vers 39,7 ; 86,3 vers 81,8) ; le taux de Ruée ne
  bouge pas (97,2 %). Le premier joueur du miroir `bulwark` recule (62,4 vers 58,9 %).
- **Combinées (`standard`)** : les effets se composent grossièrement (durées entre les deux options
  isolées, plus proches de `riposte seule`) ; Ruée 99,2 %, miroir `bulwark` à 57,0 % [54,9-59,2] (la
  seule mesure `random` qui s'approche du seuil de 57 % du premier joueur, sans le franchir
  nettement : la borne basse de l'intervalle est sous 57 %). Le déséquilibre massif de la paire croisée sous
  `random` **s'aggrave** (écart à 50 % : 47,2 vers 49,2 points) : aucune option ne le corrige.

## Politique `greedy`

| Combinaison | `rush vs bulwark` (Ruée) | miroir `rush` (1er joueur) | miroir `bulwark` (1er joueur) |
|---|---|---|---|
| `legacy` | 16,1 % [14,6-17,8] | 84,7 % [83,0-86,2] | 70,3 % [68,3-72,3] |
| `riposte seule` | 39,5 % [37,3-41,6] | 89,8 % [88,4-91,1] | 83,8 % [82,1-85,3] |
| `limite seule` | 16,2 % [14,7-17,9] | 84,7 % [83,0-86,2] | 70,5 % [68,4-72,4] |
| `standard` | 39,0 % [36,9-41,2] | 89,8 % [88,4-91,1] | 83,2 % [81,4-84,7] |

| Combinaison | Durée `rush vs bulwark` | Durée miroir `rush` | Durée miroir `bulwark` |
|---|---|---|---|
| `legacy` | 28,6 / 22 / 53 | 57,5 / 62 / 69 | 21,7 / 21 / 29 |
| `riposte seule` | 101,0 / 90 / 125 | 113,9 / 114 / 120 | 266,8 / 268 / 314 |
| `limite seule` | 28,6 / 22 / 53 | 57,5 / 62 / 69 | 22,3 / 21 / 29 |
| `standard` | 92,5 / 89 / 105 | 104,3 / 102 / 115 | 269,2 / 274 / 313 |

| Combinaison | Fins `rush vs bulwark` (`hero_down` / `exhaustion`) | Fins miroir `rush` | Fins miroir `bulwark` | Alertes « épuisement dominant » (> 70 %) |
|---|---|---|---|---|
| `legacy` | 97,8 % / 2,2 % | 100 % / 0 % | 100 % / 0 % | aucune |
| `riposte seule` | 39,5 % / 60,5 % | 5,6 % / 94,4 % | 0 % / 100 % | miroir `rush`, miroir `bulwark` |
| `limite seule` | 97,9 % / 2,1 % | 100 % / 0 % | 100 % / 0 % | aucune |
| `standard` | 39,0 % / 61,0 % | 98,2 % / 1,9 % | 0 % / 100 % | miroir `bulwark` |

Abandon : 0 % partout. Parties interrompues : **0 dans tous les lots** (aucun `WARNING interrupted`).

Lecture :

- **La riposte du héros change tout pour la gloutonne** : les parties passent de 22 à 90 actions
  (médiane de `rush vs bulwark`), et la fin dominante bascule du coup au héros vers l'épuisement (2,2 %
  vers 60,5 % de `exhaustion` en `rush vs bulwark`, 100 % en miroir `bulwark`). Le taux de Ruée passe de
  16,1 à 39,5 %, plus proche de la bande 45-55 % sans y entrer (écart à 50 % : 33,9 vers 10,6 points).
  Sans riposte, la gloutonne frappe le héros dès qu'elle le peut ; avec, elle refuse les coups non létaux
  (voir `docs/ai/greedy.md`), d'où les parties longues.
- **La limite de plateau seule est sans effet mesurable sur la gloutonne** : taux et durées identiques à
  `legacy` à l'intervalle près (miroir `bulwark` : 21,7 vers 22,3 actions), parce que la gloutonne ne
  remplit pas un plateau de 5 dans ces parties courtes. Voir « Ce que la limite ne mesure pas ».
- **`standard`** : l'effet est celui de la riposte (la limite n'ajoute presque rien) : 92,5, 104,3 et 269,2
  actions en moyenne, au-dessus de la cible provisoire de 40-60 actions pour les trois matchups ; le
  premier joueur gagne 89,8 % du miroir `rush` et 83,2 % du miroir `bulwark` (0 miroir sur 2 sous le seuil
  de 57 %).
- **Alerte « épuisement dominant » sous `standard`** : miroir `bulwark` à 100 % d'épuisement (270 actions
  en moyenne). Les parties de la gloutonne dans ce miroir ne se terminent jamais par un coup au héros.

## Constat : le robot d'usure sous les règles standard

Constat établi en revue de #172 (**non remesuré par #173**, rapporté tel quel) : sous les règles
standard, le robot d'usure des tests de bout en bout n'atteint plus l'épuisement sur Ruée contre Rempart
(1200 parties sur 1200 en `hero_down`) ; seul le miroir Rempart y arrive. Ce constat est **à trancher
avec #135** (choix des défauts) : les mesures ci-dessus montrent un comportement voisin pour la
gloutonne, dont l'épuisement dépend lui aussi fortement de la riposte du héros.

## Débit

La ligne « `standard` » du tableau de [`throughput.md`](throughput.md) est la référence gardante de #128
puis #131 sous les règles par défaut ; sous ces règles les parties durent plus longtemps (1 795 973
actions pour 30 000 parties de `random`, contre 1 646 347 en `legacy`), d'où le complément actions/s.
