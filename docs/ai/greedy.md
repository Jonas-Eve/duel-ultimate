# Politique gloutonne (`duel-ai`, #159)

Code : `crates/ai/src/policies/{eval,greedy,noisy}.rs`, registre dans `policies/mod.rs`.

## Principe

Un coup d'avance, sans recherche et sans hasard. Pour chaque action légale (hors `Concede`) la
politique demande à l'`Observation` son résultat (`outcome(i)`) : l'aperçu du moteur et la vue
du siège après l'action, où une carte piochée est seulement comptée, jamais montrée. Elle note
ce résultat et prend le meilleur. Elle ne lit donc jamais la main ni la pioche adverses : la
signature de l'observation l'interdit, `tests/greedy.rs::greedy_sees_nothing_hidden` le montre
par l'exemple.

## Priorités (conséquences du score, pas des règles codées)

1. Une partie qui se termine vaut `+WIN` ou `-WIN` (`WIN = 1 << 40`), lue sur `ends_game` :
   le coup létal passe en premier, et une action que l'aperçu montre perdante (par exemple une
   attaque dont l'attaquant meurt et laisse le siège sans rien à jouer) n'est jamais choisie
   tant qu'une autre existe. « Frapper le héros » n'est qu'un terme de PV.
2. Sinon la somme décide.
3. Égalité : coût de l'action croissant (sans coût en dernier), puis indice (ordre du moteur).

## Formule (entiers, `i64`, additions saturées)

```
score = terminal
      + hero_hp * (PV héros du siège - PV héros adverse)
      + valeur(plateau propre) - valeur(plateau adverse)
      + hand_percent/100 * valeur(main connue) + draw_value * cartes piochées - draw_value * main adverse
      + time * budget                         si le siège garde la main
      - time * budget - handover              s'il passe la main (budget reçu par l'adversaire)
valeur(carte) = 0 si vie = 0, sinon force*3 + riposte*2 + vie*2 + provocation*4
```

`card_value` (dans `eval.rs`) est le seul endroit qui lit les champs d'une carte ; #133 le
remplacera sans toucher au reste.

## Poids (provisoires)

| Poids | Valeur | Raison |
|---|---|---|
| `hero_hp` | 2 | un PV de héros vaut moins qu'un point de statistique de créature : on tue d'abord ce qui riposte |
| `force` / `riposte` / `health` | 3 / 2 / 2 | l'attaque pèse plus que la défense ; la riposte compte pour la valeur défensive |
| `provocation` | 4 | tuer un provocateur ouvre l'accès au héros |
| `hand_percent` | 50 | une carte en main vaut la moitié de sa valeur en jeu : poser une carte rapporte l'autre moitié |
| `draw_value` | 6 | une carte inconnue vaut en moyenne une carte moyenne en main |
| `time` | 3 | une unité de temps vaut à peu près un point de force |
| `handover` | 4 | passer la main est un coût en soi |

## Force mesurée

`tests/greedy.rs::greedy_beats_random_on_starter` : 200 parties sur `starter@1.1.0`
(`rush` au siège One, `bulwark` au siège Two ; la gloutonne tient le siège One dans les 100
premières parties, le siège Two dans les 100 suivantes, graines fixes). Seuil : au moins 70 %
(140 victoires). Taux observé avec ces poids : 176 victoires sur 200 (88 %).

## Limites

- Un coup d'avance : aucune combinaison sur plusieurs actions, aucune anticipation du tour adverse.
- La pioche est non évaluable (constante `draw_value`).
- Poids provisoires, à revoir avec la valeur de carte (#133) ; l'effet de la riposte du héros
  est mesuré par #127 (#173 : voir plus bas ; une attaque dont l'attaquant meurt est déjà mal notée : la créature
  disparaît de la vue). Depuis #170 (option `hero_riposte`, aucun poids retouché) c'est testé dans
  `crates/ai/tests/greedy.rs` : face au héros, la gloutonne refuse le coup non létal (un sacrifice) et
  garde le coup létal. Depuis #172 la riposte du héros est **active par défaut** (`RuleSet::standard()`) :
  le sacrifice est pesé par l'essai à blanc (`attacker_destroyed` de l'aperçu, l'attaquant disparaît de la
  vue après l'action), aucun poids n'est retouché ici.
  **Effet mesuré (#173)** : voir [`docs/balance/rules-options.md`](../balance/rules-options.md) et les rapports
  de `docs/balance/reports/` (`starter@1.1.0`, graine 1, 2000 parties par matchup). Riposte seule contre
  règles historiques : la durée de `rush vs bulwark` passe de 28,6 à 101,0 actions en moyenne et
  l'épuisement de 2,2 à 60,5 % des fins ; sous `standard`, 92,5 actions et 61,0 % d'épuisement, 100 % en
  miroir `bulwark`. La gloutonne **cesse donc de finir les parties au héros** quand la riposte est active,
  ce qui est cohérent avec le refus des coups non létaux. La lecture est **indirecte** : le rapport ne
  donne pas la part d'attaques sur le héros hors coup létal, seulement les durées et les fins de partie, et
  aucune métrique n'a été ajoutée pour cela. La limite de plateau seule n'a pas d'effet mesurable sur la
  gloutonne.

## Brancher une autre politique

#132 (défensive) et #133 (valeur de carte) réutilisent `choose_best` avec d'autres `Weights` ou
une autre `card_value`, puis s'inscrivent dans `POLICIES`. `noisy(inner, percent)` enveloppe
n'importe quelle politique : un tirage `random_range(0..100u8)` par décision, un second par
`random_policy` seulement quand le bruit joue ; `percent == 0` reprend les décisions de `inner`.
