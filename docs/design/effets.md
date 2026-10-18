# Catalogue des effets : opérations, sélecteurs, déclencheurs, durées et types de carte

Document normatif de la story #125 (Epic #120, moteur d'effets générique). Il fixe les
**ensembles fermés** sur lesquels reposent le moteur (#128), le format de données (#137), le
prix (#133) et les cartes à venir. Le tableau de la section 3 a un format fixe, lu par un test
de CI (`crates/cards/tests/effects_catalog.rs`) ; le recoupement avec les enums Rust est un
critère de #137.

## Validation

- [x] Catalogue validé par le propriétaire (décision explicite, y compris les quatre décisions
      de la section « Décisions du propriétaire » ci-dessous).

Validé par le propriétaire le : 2026-10-18, décisions relayées dans le commentaire du ticket #125 (https://github.com/Jonas-Eve/duel-ultimate/issues/125#issuecomment-6246571955).

Case cochée ici sur décision explicite du propriétaire, qui a validé en session ; un agent ne la
coche jamais de sa propre initiative. Le test de CI ne lit pas ce bloc.

## Décisions du propriétaire

1. **Riposte unique du mode `simultaneous`** (section 7), modifiée par le propriétaire : la
   riposte frappe **l'attaquant qu'elle peut tuer** (riposte nominale >= PV courants de
   l'attaquant, sur l'état d'avant l'action) ; si elle peut tuer **les deux**, celui qui a le
   **plus de PV** ; si elle n'en peut tuer **aucun**, celui qui a le **plus de PV**. Ajout
   d'agent, à signaler : à égalité de PV, le plus petit identifiant d'instance (départage
   déterministe).
2. **`on_play` et `activated`** (section 4) : pour une créature ou un bâtiment, la pose est
   toujours utilisable et les opérations `on_play` non résolubles sont ignorées ; pour un
   sort, la pose exige qu'au moins une opération D de ses effets `on_play` se résolve ; un
   effet `activated` exige toujours au moins une opération D résoluble.
3. **`scales_with_force`** (section 7) : une attaque spéciale explicite a son propre montant,
   insensible à `ModifyStat` sur la force, sauf si elle déclare `scales_with_force` (le montant
   s'ajoute alors à la force effective de la source ; défaut faux).
4. **Forme exacte de l'opération `Attack` explicite** (section 7), dont `attackers` borné à 2
   en vague 1.

## 1. Modèle unique

Une **carte** est : un **type** (un faisceau d'effets) + un **coût** + une **zone** + une
**durée de vie**. Le moteur n'a qu'une entité ; le `kind` JSON (créature, bâtiment, sort) est un
discriminant de forme, du sucre de donnée.

Un **effet** est : un déclencheur + un coût (effets `activated` seulement) + un sélecteur (qui
porte le camp) + une liste bornée d'opérations + une durée. Une **opération** est la brique
atomique du catalogue : chacune est rangée dans une classe Φ (section 5) et reçoit un prix
(section 12).

Principes de forme :

- **Les actions sont portées par les effets.** Les actions légales d'un joueur sont dérivées
  des effets utilisables présents (main, plateau, héros), pas d'une énumération par type de
  carte.
- **Ensembles fermés.** Déclencheurs, durées, sélecteurs, camps, opérations et mots-clés sont
  des enums Rust ; tout le reste est refusé par la validation avec une erreur `CardsError`
  (réalisé par #137, aucun code ici).
- **Polarité dérivée, jamais stockée** (section 12).
- **Une carte reste une donnée** : jamais de code par carte, jamais de lecture par nom ni par
  identifiant de carte par le moteur.

## 2. Types de carte

Un type de carte est un faisceau d'effets avec des champs et une durée de vie propres.

| Type | Où il se joue | Reste en jeu | Cible | Mort ou défausse |
|---|---|---|---|---|
| Créature | Depuis la main vers le plateau du propriétaire (`Action::Play`). | Oui, sur le plateau (durée de vie `while_source_in_play`). | Porte une attaque normale intrinsèque (champs plats `force`, `attack_cost`), plus des effets optionnels. | À 0 PV, défausse publique de son propriétaire, dans l'ordre de mort. |
| Bâtiment | Depuis la main vers le plateau du propriétaire. | Oui, sur le plateau. | Mêmes champs que la créature sans `force` ni `attack_cost` ; **aucun effet Attaque**, donc aucune action d'attaque ; il peut être ciblé. | Comme la créature : défausse publique à 0 PV. |
| Sort | Depuis la main ; ses effets `on_play` se résolvent à la pose. | Non : il ne va jamais sur le plateau. | Selon les sélecteurs « choisis » de ses effets, via `picks` (section 9). | Défausse publique à la résolution ; vie nulle. |
| Événement | Comme un sort. | Non. | Comme un sort. | Défausse publique. **Pas un kind de plus** : c'est un sort dont l'opération est une attaque à plusieurs attaquants (#138). |
| Équipement | Plus tard (vague 2+). | Oui, attaché à une créature, durée « attaché ». | À préciser avec sa vague. | Quitte le jeu avec sa cible ; à préciser. |

Notes :

- L'état invalide « bâtiment avec `force` » n'est pas représentable : le bâtiment est un kind
  distinct.
- L'attaque normale d'une créature n'est jamais écrite dans `effects` (section 7).
- Le moteur ne distingue pas ces types par une énumération de règles : il lit des effets, des
  statistiques et une zone.

## Tableau du catalogue

Format fixe lu par le test : huit colonnes, valeurs dans les listes fermées, cellules jamais
vides (`n/a` si sans objet), aucun `|` dans une cellule. Toute opération a une classe Φ (S, D
ou N) et une condition d'usage.

| élément | famille | déclencheur | sélecteur | durée | condition d'usage | classe Φ | vague |
|---|---|---|---|---|---|---|---|
| Attack | opération | activated, on_play | hero_or_creature(camp) | instant | au moins une cible légale résoluble (provocation comprise) ; pour l'attaque normale et `scales_with_force`, force effective >= 1 ; sinon `amount` >= 1 | D | 1 |
| DealDamage | opération | on_play, activated | hero_or_creature(camp), creature(camp), all_creatures(camp) | instant | au moins une cible résoluble (héros ou créature en jeu), hors provocation | D | 1 |
| Draw | opération | on_play, activated | hero(camp) | instant | pioche non vide : au moins une carte résoluble à piocher | D | 1 |
| ModifyStat | opération | continuous | self, creature(camp), all_creatures(camp) | while_source_in_play | toujours résoluble tant que la source est en jeu (valeur lue, état inchangé) | S | 1 |
| GrantKeyword | opération | continuous | self, creature(camp), all_creatures(camp) | while_source_in_play | toujours résoluble tant que la source est en jeu (valeur lue, état inchangé) | S | 1 |
| Soin | opération | on_play, activated | hero_or_creature(camp) | instant | cible blessée résoluble | N | 2+ |
| Bouclier | opération | on_play, activated | hero_or_creature(camp) | instant | cible résoluble | N | 2+ |
| AuraDeVie | opération | continuous | creature(camp), all_creatures(camp) | while_source_in_play | source en jeu, valeur résoluble | N | 2+ |
| Résurrection | opération | on_play, activated | creature(camp) | instant | créature de la défausse résoluble | N | 2+ |
| RetourEnMain | opération | on_play, activated | creature(camp) | instant | créature du plateau résoluble | N | 2+ |
| Contrôle | opération | on_play, activated | creature(camp) | while_source_in_play | créature adverse résoluble | N | 2+ |
| FixerA | opération | continuous | self, creature(camp), all_creatures(camp) | while_source_in_play | source en jeu, ordre d'application résoluble à définir | S | 2+ |
| Multiplier | opération | continuous | self, creature(camp), all_creatures(camp) | while_source_in_play | source en jeu, ordre d'application résoluble à définir | S | 2+ |
| self | sélecteur | n/a | n/a | n/a | aucun choix : la source de l'effet | n/a | 1 |
| hero(camp) | sélecteur | n/a | n/a | n/a | aucun choix : le héros du camp | n/a | 1 |
| creature(camp) | sélecteur | n/a | n/a | n/a | un choix (pick) : une créature du plateau du camp | n/a | 1 |
| hero_or_creature(camp) | sélecteur | n/a | n/a | n/a | un choix (pick) : un héros ou une créature du camp | n/a | 1 |
| all_creatures(camp) | sélecteur | n/a | n/a | n/a | aucun choix : toutes les créatures du camp | n/a | 1 |
| on_play | déclencheur | on_play | n/a | instant | à la pose de la carte, depuis la main | n/a | 1 |
| activated | déclencheur | activated | n/a | instant | à l'action du joueur actif, au coût propre de l'effet | n/a | 1 |
| continuous | déclencheur | continuous | n/a | while_source_in_play | tant que la source est en jeu, recalculé sans cache | n/a | 1 |
| on_event | déclencheur | on_event | n/a | n/a | réservé : prévu par le format, refusé par la validation tant qu'il n'est pas implémenté | n/a | réservé |
| when | déclencheur | n/a | n/a | n/a | condition explicite au-delà de la résolubilité, refusée en vague 1 | n/a | 2+ |
| instant | durée | n/a | n/a | instant | effet appliqué à la résolution, sans suite | n/a | 1 |
| while_source_in_play | durée | n/a | n/a | while_source_in_play | tant que la source est sur le plateau | n/a | 1 |
| créature | type de carte | n/a | n/a | while_source_in_play | Attaque intrinsèque et effets, sur le plateau | n/a | 1 |
| bâtiment | type de carte | n/a | n/a | while_source_in_play | effets sans Attaque, sur le plateau | n/a | 1 |
| sort | type de carte | on_play | n/a | instant | effets on_play puis défausse publique | n/a | 1 |
| équipement | type de carte | n/a | n/a | while_source_in_play | attaché à une créature, plus tard | n/a | 2+ |
| ranged | mot-clé | continuous | n/a | while_source_in_play | mot-clé d'attaquant (Tir) accordé par GrantKeyword, lu par provocation_allows | n/a | 1 |
| paratonnerre | mot-clé | continuous | n/a | while_source_in_play | nom provisoire, à nommer par le propriétaire : redirige ou protège contre les dégâts directs | n/a | 2+ |

Notes sur le tableau :

- `when` est rangé en famille `déclencheur` faute de famille dédiée dans la liste fermée ; ce
  n'est ni un déclencheur ni une opération, mais l'emplacement des conditions explicites.
- Les sélecteurs « choisis » (`creature(camp)`, `hero_or_creature(camp)`) demandent un choix
  porté par l'action ; `self`, `hero(camp)` et `all_creatures(camp)` n'en demandent aucun.
- `camp` vaut : propriétaire, adversaire, ou les deux.
- Pour `ModifyStat`, `stat` est parmi force, riposte, coût d'invocation, coût d'attaque ;
  `delta` est signé. Pour `GrantKeyword`, `keyword` vaut `ranged` en vague 1.

## 4. Condition d'usage et résolution partielle

**« Non résoluble = non utilisable. »** Un effet est utilisable si chaque opération requise est
résoluble. Un effet non utilisable n'est pas proposé dans `legal_actions`, son bouton « Jouer »
est grisé avec une raison fournie par le moteur, il est invisible pour l'IA, et il ne compte pas
comme action pour l'épuisement (« échec et mat »).

**Résolution partielle** (décidée par le propriétaire) : un effet à plusieurs opérations est
utilisable si **au moins une opération D se résout** ; les opérations non résolubles sont
ignorées.

**Distinction `on_play` et `activated`** (décision validée, n° 2) :

- Pour une **créature** ou un **bâtiment**, la pose est toujours utilisable (sous réserve de la
  limite de plateau, #127) et ses opérations `on_play` non résolubles sont ignorées : une
  créature « pioche 1 » reste jouable pioche vide.
- Pour un **sort**, la pose exige qu'au moins une opération D de ses effets `on_play` se
  résolve.
- Pour un effet **`activated`**, au moins une opération D doit se résoudre.

La garde de l'`on_play` d'une créature est un choix produit (la pose fait déjà baisser Φ) ; celle
de `activated` est une nécessité de terminaison (section 5).

Il n'y a pas de champ de condition d'usage dans les données en vague 1 : la condition est
implicite. Le champ `when` est réservé pour des conditions au-delà de la résolubilité et refusé
par la validation en vague 1.

## 5. Classes Φ et règle de terminaison

Le moteur termine parce que le potentiel Φ décroît strictement à chaque action. Φ reste inchangé :
PV des héros, plus, par carte, la vie courante, +2 en pioche, +1 en main, 0 sur le plateau, rien
en défausse. Un sort a une vie nulle : en main -1 à la pose.

Toute opération est rangée dans **une classe Φ**, par un `match` exhaustif sans bras `_` :

- **S, statique** : ne change que des valeurs lues, ne touche pas l'état. `ModifyStat`,
  `GrantKeyword`. Continue seulement.
- **D, décroissante** : fait baisser Φ d'au moins 1 quand elle se résout.
- **N, neutre ou croissante** : soin, bouclier, retour en main, résurrection, aura de vie,
  contrôle. **Refusées en vague 1.**

Ce que chaque opération de vague 1 retire à Φ :

- `Attack` : chaque attaquant retire `min(amount, vie) >= 1` à la vie courante de la cible (PV du
  héros ou vie de la créature), sans jamais rien ajouter ; la riposte ne retire que des PV finis
  de plus à une créature en jeu.
- `DealDamage` : retire `min(amount, vie) >= 1` à la cible.
- `Draw` : déplace chaque carte de la pioche vers la main, soit -1 par carte.
- `ModifyStat`, `GrantKeyword` : 0, valeurs lues seulement ; c'est pourquoi elles ne sont jamais
  une action à elles seules.

**Règle de terminaison** : une action n'est utilisable que si au moins une de ses opérations D se
résout. La décision « non résoluble = non utilisable » est donc la garde de terminaison.

- Un **effet activé réutilisable** contient au moins une opération D ; un effet activé uniquement
  S ou N exigerait un compteur d'usage dans l'état (Φ étendu) : **hors vague 1**.
- **Aucune aura ne modifie la vie en vague 1** : sinon Φ devrait lire la vie de base moins les
  dégâts subis. L'aura de vie est de vague 2, avec cette ré-dérivation.
- Les effets sur événement (`on_event`, plus tard) ne contiendront que des opérations S ou D, avec
  un plafond dur de résolutions par action (section 13).

## 6. Planchers et ordre

- Les modificateurs de statistiques sont **additifs puis bornés** : l'ordre d'application n'a
  aucune importance. Pas d'opération « fixer à » ni « multiplier » en vague 1, qui introduiraient
  un ordre.
- **Force effective** : pas de plancher à 1 (décidé par le propriétaire). Une force effective à 0
  rend l'attaque **impossible** : la créature n'offre aucune action d'attaque, comme un bâtiment.
  C'est sans danger pour la terminaison (pas d'action, donc pas de boucle), et un joueur qui n'a
  que des créatures à force 0 et aucune autre action utilisable peut être « mat ».
- **Invariant** : une créature porteuse de l'effet Attaque a une attaque légale **si et seulement
  si** sa force effective est au moins 1 et qu'une cible existe. Le test de `rules.rs` qui
  affirmait « une créature vivante a toujours une attaque » est **adapté** par #128, pas supprimé.
  `force >= 1` reste exigé dans les données par #137 ; seul un modificateur peut ramener la force
  effective à 0.
- **Riposte effective** >= 0.
- **Coût effectif >= 1**, coût d'invocation comme coût d'attaque (confirmé par le propriétaire).
- Un montant de dégâts nul est refusé par la validation (cohérent avec « force 0 = pas d'attaque »).

## 7. L'attaque est un effet

L'attaque normale et les attaques spéciales sont toutes des effets d'attaque, **affichées partout
de la même façon** : aucune distinction visible « normale ou spéciale » (même ligne dans le détail
de carte, même aperçu). Une carte liste ses effets d'attaque.

- **Attaque normale (intrinsèque)** : synthétisée depuis les champs plats `force` et `attack_cost`
  de la créature, **jamais écrite dans `effects`** (les octets de `starter@1.0.0` et `1.1.0`
  restent figés ; le prix et le détail de carte lisent les champs plats ; l'écrire en plus ajouterait
  de la redondance et un état invalide représentable). Elle lit toujours la **force effective**.
- **Attaques spéciales** : effets `activated` **explicites** écrits sur la carte, dont l'opération
  `Attack` porte ses propres paramètres. Une carte peut en porter en plus de l'attaque normale. Il
  n'y a **pas** de déclaration « remplacée ou conservée ».

**Forme de l'opération `Attack` explicite** (`deny_unknown_fields`, décision validée n° 4) :

```json
{
  "op": "attack",
  "target": "<sélecteur>",
  "amount": 3,
  "ignores_provocation": false,
  "attackers": 2,
  "mode": "simultaneous",
  "scales_with_force": false
}
```

| Champ | Type | Règle |
|---|---|---|
| `op` | chaîne | `attack`. |
| `target` | sélecteur | Un seul pour tous les attaquants ; typiquement `hero_or_creature(camp)`. |
| `amount` | u32 | >= 1 (zéro refusé). |
| `cost` | u32 | porté par l'effet (une seule fois), absent de l'opération : voir ci-dessous. |
| `ignores_provocation` | booléen | Défaut faux ; lu par le filtre unique `provocation_allows`. |
| `attackers` | entier | 1 à 2 en vague 1, défaut 1. |
| `mode` | `simultaneous` ou `chained` | Exigé si `attackers` >= 2, refusé si `attackers` = 1. |
| `scales_with_force` | booléen | Défaut faux (décision validée n° 3). |

**Où est écrit le coût (précision de #137, D1).** Le coût d'une attaque spéciale est le coût de
**l'effet** `activated` qui la contient (>= 1, écrit une seule fois : `{"trigger":"activated",
"cost":2,"ops":[{"op":"attack",...}]}`) ; l'opération `attack` n'a pas de clé `cost`, parce qu'un
effet à plusieurs opérations n'a qu'un coût. La décision n° 4 est respectée sur le fond (un coût
>= 1 propre à l'effet), seule la place de la clé est précisée. **D4** : en vague 1, le `side` du
sélecteur d'une opération `attack` vaut `opposing` seulement (une attaque ne vise jamais son
propre camp) ; relâcher plus tard est additif. **D2** : une unité immobile ne déclare aucun effet
`attack` en vague 1 (elle peut déclarer tout autre effet).

**`ModifyStat` sur la force.** Il s'applique aux effets d'attaque **dérivés du champ plat `force`**
(l'attaque normale). Une attaque spéciale explicite utilise **son propre montant**, insensible à
`ModifyStat`, sauf si elle déclare `scales_with_force` (une attaque spéciale a donc une valeur fixe ou évolue avec la force) : `amount` s'**ajoute** alors à la force
effective de la source. Défaut écrit ici : montant propre.

**Attaque à plusieurs attaquants.** Le **coût est payé une seule fois** dans les deux modes. Le
`target` est unique. La provocation et les `picks` sont validés **une fois, sur l'état d'avant
l'action**. Classe Φ : D (chaque attaquant retire `min(amount, vie) >= 1`).

- **`chained`** : les attaques s'enchaînent l'une après l'autre, **une riposte par attaquant**. Si
  la cible meurt en cours de chaîne, les attaques suivantes deviennent non résolubles et sont
  **ignorées** (ce n'est pas une erreur).
- **`simultaneous`** : les attaquants frappent ensemble et la cible n'inflige **qu'une seule
  riposte**. Lecture déterministe de « sur la cible de son choix » (**décision validée n° 1**) : la riposte
  frappe **l'attaquant qu'elle peut tuer** (riposte nominale >= PV courants de l'attaquant, sur
  l'état d'avant l'action) ; si elle peut tuer **les deux**, celui qui a le **plus de PV** ; si elle
  n'en peut tuer **aucun**, celui qui a le **plus de PV** ; à égalité de PV, le plus petit
  identifiant d'instance (départage déterministe ajouté). Les dégâts de tous les attaquants sont
  appliqués ensemble, calculés sur l'état d'avant, comme `plan_damage` aujourd'hui.

Lectures alternatives refusées :

- « Le défenseur choisit » : cela casserait le modèle « une action = un nouvel état » (le choix
  du défenseur interromprait l'action de l'adversaire), l'essai à blanc de l'aperçu et l'IA.
- « L'attaquant choisit » : la riposte deviendrait un malus qu'il dirige toujours vers sa
  créature la plus solide.
- « Le plus petit identifiant d'instance » (première version de ce document) : arbitraire, sans
  rapport avec l'état du combat.

## 8. Ciblage et provocation

- La provocation ne concerne que les **attaques**.
- Un sort de dégâts directs (opération `DealDamage`, sans riposte ni provocation) peut viser un
  héros ou une créature sans être soumis à la provocation.
- **Exception explicite** : un sort dont l'opération est une **attaque** (l'attaque combinée de
  #138) **est** soumis à la provocation : c'est une attaque, `provocation_allows` s'applique à ses
  cibles.
- **Tir** (`ranged`, « ignore la provocation ») est un mot-clé d'**attaquant**, accordé par
  `GrantKeyword` et lu par le filtre unique `provocation_allows` ; sans objet pour les sorts. Une
  attaque spéciale peut aussi porter l'option `ignores_provocation`, lue par le même filtre.
- Le filtre `provocation_allows` reste unique, recalculé sur l'état courant, sans cache.

## 9. Choix portés par une action (`picks`)

Un effet déclare ses choix : un par sélecteur « choisi » (`creature(camp)`,
`hero_or_creature(camp)`). L'action porte une **liste ordonnée libre** de choix, autant que
l'effet en demande, bornée par la **constante de sécurité 8** (confirmée par le propriétaire, à
ajuster si besoin).

- **Ordre** : d'abord les attaquants (si `attackers` >= 2, ou pour une carte-sort d'attaque), puis
  les cibles dans l'ordre de déclaration des sélecteurs « choisis ».
- Une liste trop longue, trop courte, un doublon d'attaquant ou un choix hors sélecteur est refusé
  par le moteur comme toute action invalide.
- **Une erreur de choix ne distingue jamais main, pioche, défausse ou identifiant inconnu** : même
  code d'erreur, comme `target_not_on_board` aujourd'hui, pour ne rien révéler des zones cachées.
  Un sort ne révèle aucune information cachée en vague 1.
- **Conséquence** : le type `Action` perd `Copy`. C'est réalisé par #128, avec un coût de
  propagation (simulateur, `duel-ai`, binding, contrat TypeScript).

## 10. Principes

- **Effets continus recalculés sur l'état courant, sans cache**, par une lecture unique
  `effective` (coût, force, coût d'attaque, riposte, mots-clés effectifs), appelée par la
  validation, les dégâts, `legal_actions`, la vue et l'aperçu.
- **Réductions de coût** : valeur d'entrée de la règle de temps unique `time::spend`, inchangée ;
  plancher 1.
- **Une opération n'influence pas les decks qui ne l'utilisent pas.**
- **Une carte reste une donnée** : jamais de code par carte, jamais de lecture par nom ni par
  identifiant par le moteur.
- Moteur **pur et déterministe**, sans I/O ni horloge ; aucune opération du catalogue ne consomme
  de hasard ; Φ strictement décroissant.

## 11. Héros

Les héros utilisent le même modèle d'effets : la pioche est l'opération `Draw`. Des PV variables
ne viennent qu'avec les opérations N (vague 2). Le JSON actuel d'effet de héros
`{ "name", "cost", "effect": { "kind": "draw", "count" } }` reste accepté comme **sucre de
données** et est normalisé vers le type d'effet unifié.

## 12. Prix et polarité

- **Prix** : renvoi à #133. Le prix est **par opération** (selon montant, sélecteur, camp, polarité
  dérivée), par un `match` exhaustif ; une opération sans prix refuse la carte (« non chiffrée »).
- **Polarité dérivée, jamais stockée** : elle se déduit du signe de l'opération et du **camp du
  sélecteur** (+2 force sur l'adversaire est un malus pour le propriétaire). Le camp affecté fait
  partie du sélecteur, pas de l'effet. Le client et le prix lisent la polarité dérivée
  (décidé par le propriétaire).

## 13. Bornes de combinatoire

- Valeurs finales fixées par #137 : `MAX_CARD_EFFECTS` = 4, `MAX_OPS_PER_EFFECT` = 3 et
  `MAX_EFFECT_PICKS` = 3 (choix attendus par effet : une attaque à deux attaquants et une cible, ou
  trois choix simples). Avec `board_limit` = 5, chaque choix a environ 12 cibles, soit environ
  1 700 actions par usage à 3 choix ; à 8 l'aperçu (un clone par action légale) exploserait.
- `picks` d'une action : au plus 8 (constante de sécurité du moteur, `MAX_PICKS`) ;
  `MAX_EFFECT_PICKS` <= `MAX_PICKS`, gardé par un test du moteur.
- Un plafond dur de résolutions par action sera posé avec `on_event`, en défense en profondeur.
- Les valeurs de carte et les `picks` sont **non fiables** : le moteur les valide toujours.

## 14. Vagues ultérieures et hors périmètre

Au catalogue mais **non implémentés** (vague 2+) : soin, bouclier, aura de vie, résurrection, retour
en main, contrôle (toutes de classe N, Φ à étendre) ; opérations « fixer à » et « multiplier »
(l'ordre d'application reste à définir) ; mot-clé « paratonnerre » (nom provisoire, à nommer par le
propriétaire) ; conditions explicites `when` ; déclencheur `on_event` (réservé) ; effets activés
de classe S ou N avec compteur d'usage ; type équipement.

Hors périmètre de ce document : tout code moteur, les enums Rust et la validation `CardsError`
(#137), le prix (#133), les nouvelles cartes, les illustrations.

## 15. Écriture JSON (#137)

Les effets s'écrivent dans `cards.json`, sur une unité (`kind` `unit`, lu aussi `creature`) ou sur
un sort (`kind` `spell`). Un effet est `{ "trigger", "cost"?, "ops" }` : `cost` seulement pour un
effet `activated` ; la durée est dérivée du déclencheur et jamais écrite (`on_play` et `activated`
: `instant`, `continuous` : `while_source_in_play`). Une opération est un objet à clé `op`
(`attack`, `deal_damage`, `draw`, `modify_stat`, `grant_keyword`) ; son sélecteur est
`{ "select": ..., "side": ... }` (`self` n'a pas de `side`). Les valeurs par défaut sont omises
(`ignores_provocation` et `scales_with_force` faux, `attackers` 1, pas de `mode`). Bornes : 4
effets par carte, 3 opérations par effet, 3 choix attendus par effet. L'attaque normale d'une unité
n'est jamais écrite : elle reste dérivée de `force` et `attack_cost`. Un sort s'écrit avec `cost` et
des effets `on_play` seulement (au moins un), sans `force`, `health` ni les autres champs d'une
unité.

Un sort de dégâts directs puis pioche, et une unité qui pioche à la pose :

```json
[
  {
    "kind": "spell", "id": "eclair", "name": "Éclair", "cost": 2,
    "effects": [
      { "trigger": "on_play", "ops": [
        { "op": "deal_damage", "target": { "select": "hero_or_unit", "side": "opposing" }, "amount": 2 },
        { "op": "draw", "target": { "select": "hero", "side": "own" }, "count": 1 }
      ] }
    ]
  },
  {
    "kind": "unit", "id": "eclaireur", "name": "Éclaireur",
    "cost": 2, "force": 1, "attack_cost": 1, "health": 2,
    "effects": [
      { "trigger": "on_play", "ops": [
        { "op": "draw", "target": { "select": "hero", "side": "own" }, "count": 1 }
      ] }
    ]
  }
]
```

Une unité à attaque spéciale activée : le coût est celui de l'effet, l'opération n'a pas de
`cost`, et `mode` est exigé parce que `attackers` vaut 2.

```json
[
  {
    "kind": "unit", "id": "duelliste", "name": "Duelliste",
    "cost": 4, "force": 2, "attack_cost": 2, "health": 4,
    "effects": [
      { "trigger": "activated", "cost": 3, "ops": [
        { "op": "attack", "target": { "select": "hero_or_unit", "side": "opposing" }, "amount": 3,
          "ignores_provocation": true, "scales_with_force": true,
          "attackers": 2, "mode": "simultaneous" }
      ] }
    ]
  }
]
```

Une unité aux effets continus, et un bâtiment (unité `immobile` portant l'étiquette `bâtiment`,
déclarée dans la liste fermée du jeu) qui n'a ni `force` ni `attack_cost` et aucun effet `attack` :

```json
[
  {
    "kind": "unit", "id": "etendard", "name": "Étendard",
    "cost": 3, "force": 1, "attack_cost": 2, "health": 3,
    "effects": [
      { "trigger": "continuous", "ops": [
        { "op": "modify_stat", "target": { "select": "all_units", "side": "own" }, "stat": "force", "delta": 1 },
        { "op": "grant_keyword", "target": { "select": "self" }, "keyword": "ranged" }
      ] }
    ]
  },
  {
    "kind": "unit", "id": "tour", "name": "Tour de guet",
    "cost": 3, "health": 6, "immobile": true, "tags": ["bâtiment"],
    "effects": [
      { "trigger": "activated", "cost": 2, "ops": [
        { "op": "deal_damage", "target": { "select": "hero_or_unit", "side": "opposing" }, "amount": 1 }
      ] }
    ]
  }
]
```

La polarité (`bonus`, `malus` ou `neutre`, du point de vue du propriétaire de la carte) se dérive
du signe de l'opération pour le camp affecté et de ce camp, jamais stockée ; la liste de choix d'un
effet (`expected_picks`) est dérivée de ses opérations : les attaquants d'abord, puis la cible.
