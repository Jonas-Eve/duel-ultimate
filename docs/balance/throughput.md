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

- Les options de règles sont **explicites** pour que la commande reste vraie après #127, qui
  change les défauts du moteur.
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

Détail de la mesure du 2026-10-17 : 6 passes de 30 000 parties (1 646 347 actions chacune), la
première écartée ; passes retenues en parties/s : 14 151,4 ; 13 954,7 ; 14 906,7 ; 13 690,3 ;
14 766,3. Actions/s retenues : 776 602,2 ; 765 807,3 ; 818 053,2 ; 751 298,6 ; 810 349,0
(min-max 751 298,6 - 818 053,2). Réserve : la machine n'était pas strictement inactive (charge
moyenne d'environ 1,6 sur 4 coeurs, un autre développeur travaillant en parallèle dans le même
conteneur) ; l'exécution est monocoeur, et la dispersion observée est d'environ 9 %.

La ligne « nouveaux défauts de #127 » est ajoutée par #127 avec le même protocole : c'est elle
qui garde #128 puis #131.
