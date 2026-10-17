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
  sera re-mesuré par #127 (une attaque dont l'attaquant meurt est déjà mal notée : la créature
  disparaît de la vue).

## Brancher une autre politique

#132 (défensive) et #133 (valeur de carte) réutilisent `choose_best` avec d'autres `Weights` ou
une autre `card_value`, puis s'inscrivent dans `POLICIES`. `noisy(inner, percent)` enveloppe
n'importe quelle politique : un tirage `random_range(0..100u8)` par décision, un second par
`random_policy` seulement quand le bruit joue ; `percent == 0` reprend les décisions de `inner`.
