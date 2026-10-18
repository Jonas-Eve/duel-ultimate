# Débit du simulateur

Mesure du débit de `duel-simulator` (story #123, Epic #120). Elle sert de référence aux gardes
de l'Epic (#128, #131) : une régression de débit se lit contre le seuil consigné ici.

## Métrique

- **Parties par seconde** : `games / secondes`, sur le lot entier (énoncé de la story).
- **Actions par seconde**, consignées en complément : la durée des parties change avec les règles
  de #127 ; sans ce complément, un écart de règles se lirait comme une régression de code.

Les deux viennent de l'option `--timing` du binaire, qui écrit **une ligne sur stderr** après le
rapport :

```
timing: 30000 games, 1646347 actions, 2.032 s, 14763.4 games/s, 810190.1 actions/s
```

Le temps couvre le lot seul (`run_batch_observed`, parties interrompues comprises, puisque c'est
le travail réellement fait) : ni l'analyse des arguments, ni la construction des contendants, ni
le rendu du rapport. L'horloge vit dans `main.rs` seulement (`tests/no_clock.rs`) ; stdout est
identique octet pour octet avec ou sans l'option.

## Protocole

```
cargo build --release -p duel-simulator
target/release/duel-simulator 10000 --set starter@1.1.0 --seed 1 --hero-riposte off --board-limit none --timing > /dev/null
```

- Les options de règles sont **explicites** pour que la commande reste vraie après #127 (défaut du
  moteur devenu `standard` avec #172) : la commande ci-dessus mesure les règles historiques ; pour les
  règles par défaut, remplacer par `--hero-riposte on --board-limit 5` (ligne `aa1fdbc` du tableau).
- Lot : 3 matchups x 10 000 parties = 30 000 parties.
- **1 passe de chauffe écartée + 5 passes mesurées** ; valeur de référence = **médiane** des 5
  passes (minimum et maximum consignés aussi).
- Machine inactive, une seule exécution à la fois.
- Consigner avec la mesure : modèle de CPU, nombre de coeurs (`nproc`), `rustc --version`, SHA du
  commit mesuré, date.

## Seuil

**Seuil = 0,8 x la médiane de référence** (valeur de référence moins 20 %, fraction énoncée par
la story). La valeur absolue est écrite dans le tableau, avec la machine et le SHA : elle n'a de
sens que sur cette machine.

## Comparer entre machines

Le seuil absolu n'est valable que sur la classe de machine de la mesure. Un garde (#128, #131)
compare sa médiane de 5 passes au seuil consigné **si et seulement si** il tourne sur la même
classe de machine. Sinon, ou en cas de doute, il **remesure la référence** (le SHA consigné,
construit dans un worktree jetable) **sur sa propre machine, dans la même session, en alternant
les passes référence/candidat**, puis applique 0,8. La valeur remesurée prime alors sur la valeur
absolue consignée. Une exécution sous le seuil se refait une fois avant d'être déclarée
régression (bruit de voisinage).

Le seuil n'est épinglé dans aucun test et aucune mesure de débit ne tourne en CI.

## Mesures

| Date | SHA | Règles (options explicites) | Machine | Parties/s (médiane, min-max) | Actions/s (médiane) | Seuil (0,8 x) |
|---|---|---|---|---|---|---|
| 2026-10-17 | `e798210` | `--hero-riposte off --board-limit none`, `starter@1.1.0`, graine 1 | Intel Xeon 2,10 GHz, 4 coeurs (`nproc`), rustc 1.97.0 (2026-07-07), conteneur Linux | 14 151,4 (13 690,3 - 14 906,7) | 776 602,2 | 11 321,1 parties/s |
| 2026-10-17 | `f21f1c5` | idem, **nouveau format de rapport** (#153, histogramme des durées) | idem (Intel Xeon 2,10 GHz, 4 coeurs, rustc 1.97.0) | 14 136,9 (13 402,5 - 14 778,1) | 775 810,0 | **non gardante** (informative) |
| 2026-10-18 | `aa1fdbc` | **`--hero-riposte on --board-limit 5` (règles par défaut, `standard`)**, `starter@1.1.0`, graine 1 | Intel Xeon 2,10 GHz, 4 coeurs (`nproc`), rustc 1.97.0 (2026-07-07), conteneur Linux | 15 557,6 (14 200,8 - 15 605,9) | 931 366,1 | **12 446,1 parties/s** (gardante sous les règles par défaut) |

Détail de la mesure du 2026-10-17 : 6 passes de 30 000 parties (1 646 347 actions chacune), la
première écartée ; passes retenues en parties/s : 14 151,4 ; 13 954,7 ; 14 906,7 ; 13 690,3 ;
14 766,3. Actions/s retenues : 776 602,2 ; 765 807,3 ; 818 053,2 ; 751 298,6 ; 810 349,0
(min-max 751 298,6 - 818 053,2). Réserve : la machine n'était pas strictement inactive (charge
moyenne d'environ 1,6 sur 4 coeurs, un autre développeur travaillant en parallèle dans le même
conteneur) ; l'exécution est monocoeur, et la dispersion observée est d'environ 9 %.

Ligne `f21f1c5` (informative, **non gardante** ; le seuil gardant reste celui de la référence
`e798210`, 11 321,1 parties/s) : coût de l'histogramme et des compteurs du nouveau format de rapport
(#153), même protocole (1 passe de chauffe écartée à 14 121,4 parties/s, 5 passes retenues en parties/s :
13 402,5 ; 14 600,9 ; 14 778,1 ; 14 136,9 ; 13 603,8 ; actions/s retenues : 735 504,1 ; 801 273,7 ;
810 998,2 ; 775 810,0 ; 746 551,6). Mêmes 30 000 parties et 1 646 347 actions que la référence : seul
le débit varie. La médiane est à 99,9 % de celle de `e798210` (14 151,4), soit bien dans la dispersion
d'environ 9 % observée : le surcoût de l'histogramme n'est pas mesurable sur cette machine (charge
moyenne d'environ 1 sur 4 coeurs).

Ligne `aa1fdbc` (**référence gardante sous les règles par défaut** de #127 : c'est contre elle que #128
puis #131 comparent, seuil **0,8 x 15 557,6 = 12 446,1 parties/s** ; la ligne `e798210` reste la référence
des règles historiques et ne garde plus que les lots lancés avec `--hero-riposte off --board-limit none`) :
même protocole, commande exacte
`target/release/duel-simulator 10000 --set starter@1.1.0 --seed 1 --hero-riposte on --board-limit 5 --timing > /dev/null`.
Lot de 30 000 parties et **1 795 973 actions** par passe (contre 1 646 347 en `legacy` : les parties durent
plus longtemps avec la riposte du héros, d'où le complément actions/s). Passe de chauffe écartée à
15 365,0 parties/s ; 5 passes retenues en parties/s : 14 381,0 ; 15 557,6 ; 15 605,9 ; 14 200,8 ; 15 579,6 ;
en actions/s : 860 929,2 ; 931 366,1 ; 934 260,0 ; 850 140,4 ; 932 685,5 (min-max 850 140,4 - 934 260,0).
Charge moyenne de 0,8 sur 4 coeurs avant et après (machine essentiellement inactive, une seule exécution à la
fois, exécution monocoeur). La médiane est à 109,9 % de celle de `e798210` : une seule mesure, dans une autre session (même classe de machine)
et un autre jour, **ne dit rien d'un gain** ; elle fixe seulement le seuil sous les nouveaux défauts.

## Politique gloutonne (`--policy greedy`, #160)

Série **distincte** de celle de `random` ci-dessus : même protocole, autre politique, autre ordre de
grandeur. **Aucun seuil 0,8 x ici** : les gardes de #128 et #131 restent sur `random` ; la gloutonne
n'est **jamais comparée** à ce seuil ni à la médiane de `random`. Chaque décision de la gloutonne
clone l'état une fois par action légale (`dry_run`), d'où un débit bien plus bas.

```
target/release/duel-simulator 1000 --set starter@1.1.0 --seed 1 --hero-riposte off --board-limit none --policy greedy --timing > /dev/null
```

- Lot : 3 matchups x 1000 parties = 3000 parties (108 083 actions par passe, identiques d'une passe à
  l'autre). 1 passe de chauffe écartée + 5 passes mesurées, valeur de référence = médiane.
- **Plafond de plausibilité** : le rapport de référence (`2000` parties x 3 matchups, `--policy
  random,greedy`, `docs/balance/reports/2026-10-18-starter-1.1.0-greedy.md`) doit tourner en **moins
  de 10 minutes** sur la machine de mesure ; au-delà, ouvrir un ticket `type:tech` d'optimisation
  (pistes : évaluation légère côté moteur, élagage des attaques) au lieu d'optimiser dans la
  tâche. Mesuré : environ 13 s, plafond largement tenu.

| Date | SHA | Règles (options explicites) | Machine | Parties/s (médiane, min-max) | Actions/s (médiane) | Seuil |
|---|---|---|---|---|---|---|
| 2026-10-18 | `7124e99` | `--hero-riposte off --board-limit none`, `starter@1.1.0`, graine 1, `--policy greedy`, 1000 parties/matchup | Intel Xeon 2,10 GHz, 4 coeurs (`nproc`), rustc 1.97.0 (2026-07-07), conteneur Linux | 454,7 (393,7 - 470,6) | 16 380,2 (14 184,3 - 16 953,6) | aucun |

Détail : passe de chauffe écartée à 457,7 parties/s ; passes retenues en parties/s : 393,7 ; 456,0 ;
435,4 ; 454,7 ; 470,6 ; en actions/s : 14 184,3 ; 16 427,2 ; 15 685,3 ; 16 380,2 ; 16 953,6. La
gloutonne tourne à environ 1/49 des actions/s de `random` (810 000) : un peu moins de deux ordres de
grandeur, dans l'attendu. Réserve : la machine n'était **pas inactive** (charge moyenne de 3,5 à 4,3
sur 4 coeurs, une autre session compilait en parallèle) ; l'exécution est monocoeur, la dispersion
observée est d'environ 18 % : lire la valeur comme un ordre de grandeur.

La ligne « nouveaux défauts de #127 » (`aa1fdbc`, `standard`) est dans le tableau « Mesures » ci-dessus,
mesurée avec le même protocole par #173 : c'est elle qui garde #128 puis #131.
