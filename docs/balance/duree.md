# Durée d'une partie jouée à la main : protocole et feuille de relevé

Première moitié de la story #124 (Epic #120). Ce document dit **comment mesurer** la durée d'une
partie jouée à la main et **comment convertir** la mesure en actions ; il ne contient aucun
résultat chiffré. Les relevés sont faits par le propriétaire, la consignation et la conclusion sont
la tâche suivante de #124.

## 1. Objectif et lecture

Cible du propriétaire : **15 minutes en moyenne, humain contre humain** (une cible, pas un
plafond). Le simulateur, lui, ne mesure pas des minutes mais des **actions** : le tableau 2 du
rapport (voir `README.md` à la racine, « Format du rapport du simulateur », et
[`README.md`](README.md) de ce dossier) donne par matchup `mean actions`, `median` et `p90`, sur les
**parties terminées** seulement, et marque la moyenne `below` / `within` / `above` contre la bande
**provisoire** de 40 à 60 actions (`DURATION_TARGET_ACTIONS`).

Le but est donc de remplacer cette bande par une bande **mesurée** : combien d'actions tiennent
dans 15 minutes quand deux humains jouent, à la vitesse réelle du propriétaire. Hypothèse à tester :
40 à 60 actions, soit 15 à 22 secondes par action.

**Ce qu'est une « action »** : la même chose dans le simulateur et dans le client, une action du
moteur jouée par un siège : **invoquer une carte**, **activer un effet de héros** (pioche
comprise) ou **attaquer**. Le simulateur compte une unité par `apply` d'une de ces trois actions
(il n'abandonne jamais). Ne sont **pas** des actions : la pioche déclenchée par un effet, les
dégâts, les morts, la passation ou la conservation de la main (ce sont des conséquences), ni
l'abandon.

## 2. Protocole

### Matériel et conditions

- Client : le build déployé ou `npm run build && npm run preview`, jeu de données
  `starter@1.1.0` (« Données : starter@1.1.0 » en pied d'accueil). **Noter le SHA** affiché au même
  endroit (« Version : <SHA à 7 caractères> ») dans la feuille : un relevé sans version n'est pas
  reproductible.
- Chronomètre externe (téléphone). Aucun chronomètre dans le jeu, aucune analytique.
- Une partie jouée d'un trait, sans interruption (une pause se note en remarque et la partie est
  alors écartée du calcul).

### Modes

| Mode | Accueil | Qui joue | Ce qu'il mesure |
|---|---|---|---|
| A | « Je joue les deux camps » (deux camps) | deux humains, chacun un camp, ou une personne qui joue les deux camps avec des échanges réels | la cible : humain contre humain |
| B | « Contre l'ordinateur » | un humain (siège One) contre l'ordinateur gourmand (politique de #126) | le temps de réflexion humain, face à la politique la plus forte |

**Minimum proposé : 6 parties par mode** (12 au total). C'est un minimum de lisibilité, pas un
résultat statistique : la dispersion min-max suffit, aucun écart-type n'est exigé.

### Decks, camps et graines

- Decks : `starter@1.1.0`, Ruée (`rush`) et Rempart (`bulwark`). Mode A : choisir « Deck du camp 1 »
  et « Deck du camp 2 » ; mode B : « Ton deck » (l'adversaire joue l'autre).
- **Alterner les camps d'une partie à l'autre** : mode A, Ruée au camp 1 puis au camp 2 ; mode B,
  l'humain joue Ruée puis Rempart. Sur 6 parties : 3 et 3.
- **Une graine différente par partie**, saisie dans « Graine (optionnelle) » (ou laissée au hasard :
  l'écran de fin l'affiche, « Graine : N », à recopier ; en mode B il affiche aussi la graine de
  l'ordinateur). Une graine notée permet de rejouer la même partie.

### Chronométrage

- **Mode A** : démarrer à la **première action** (la première carte invoquée, l'effet ou l'attaque,
  pas au clic sur « Lancer la partie ») et arrêter à l'apparition de l'écran de fin. Durée totale =
  durée humaine.
- **Mode B** : ne chronométrer que le **temps humain**. Mettre le chronomètre en pause pendant tout
  le tour de l'ordinateur (indicateur « L'ordinateur joue… ») et le relancer quand « À toi » revient.
  Si l'ordinateur a la main en premier (héros d'initiative supérieure), le chronomètre démarre à la
  première action **humaine**. Noter dans la feuille la **durée totale** (horloge murale, pauses
  comprises) et la **durée humaine** séparément ; seule la durée humaine sert au calcul.

### Comptage des actions

À l'écran de fin, le « Journal complet » liste toute la partie en blocs (« Ton tour », « Tour de
l'ordinateur » en mode B ; un bloc par camp en mode A, titré « Camp 1 — Ruée », « Camp 2 — Rempart »).
Une action est **une ligne dont le verbe est « invoque(s) », « utilise(s) » ou « attaque(s) avec »**
(`Tu invoques …`, `Camp 2 — Rempart utilise … (coût …)`, `L'ordinateur attaque avec …`). Toutes les
autres lignes (« pioche », « subit », « meurt », « La main passe », « garde la main », « gagne », et
la ligne « La partie commence ») sont des conséquences : **ne pas les compter**. Une attaque peut
produire cinq ou six lignes, elle ne compte que pour une.

- Mode A : compter les actions de chaque camp ; **actions humaines = total des deux camps**, actions
  ordinateur = 0.
- Mode B : actions humaines = lignes d'action des blocs « Ton tour » ; actions ordinateur = celles
  des blocs « Tour de l'ordinateur ».
- Une partie terminée par abandon (ligne « abandonne » comme raison de fin) est consignée, mais
  **écartée du calcul** : le simulateur ne joue jamais d'abandon.

## 3. Feuille de relevé

À copier et remplir (une ligne par partie ; durées en secondes, entiers ; `-` quand sans objet).
Version de données : `starter@1.1.0` ; SHA du client : `__________`.

| # | Date | Mode | Deck One / Deck Two | Graine | Durée totale (s) | Durée humaine (s) | Actions humaines | Actions ordinateur | Vainqueur | Raison de fin | Remarques |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | | A | | | | | | - | | | |
| 2 | | A | | | | | | - | | | |
| 3 | | A | | | | | | - | | | |
| 4 | | A | | | | | | - | | | |
| 5 | | A | | | | | | - | | | |
| 6 | | A | | | | | | - | | | |
| 7 | | B | | | | | | | | | |
| 8 | | B | | | | | | | | | |
| 9 | | B | | | | | | | | | |
| 10 | | B | | | | | | | | | |
| 11 | | B | | | | | | | | | |
| 12 | | B | | | | | | | | | |

Raison de fin : `hero_down` (PV du héros à 0), `exhaustion` (plus d'action possible) ou `concede`,
telle que l'écran de fin la formule. En mode A, durée totale = durée humaine.

## 4. Méthode de calcul

Entiers et moyennes seulement, sur les parties **terminées sans abandon ni pause**, **mode par
mode** (ne jamais mélanger A et B).

1. **Secondes par action humaine**, par partie : `s = durée humaine / actions humaines`. Puis, sur
   les parties du mode : moyenne `s_moy`, minimum `s_min`, maximum `s_max`.
2. **Conversion** : actions pour N minutes = `N x 60 / s`.
3. **Cible de 15 minutes en actions** (mode A) : `900 / s_moy` actions en moyenne, avec sa
   dispersion `900 / s_max` (bas) à `900 / s_min` (haut), arrondies à l'entier.
4. **Comparaison au simulateur** : même unité, **actions totales des deux camps**. Mode A : la bande
   de l'étape 3 se compare à `mean actions`, `median` et `p90` de la ligne du matchup joué
   (`rush vs bulwark`, ou un miroir) du tableau 2 du rapport, de la politique et des règles de
   référence (commande de [`reports/`](reports/), graine écrite). Mode B : `s_moy` humain, appliqué
   aux seules actions humaines, dit combien de temps de réflexion humain une partie contre
   l'ordinateur coûte ; pour comparer à la durée du simulateur, **ajouter** les actions de
   l'ordinateur (durée du rapport = humaines + ordinateur) mais pas leur temps.
5. **Réserve** : la politique du simulateur (`random`, `greedy`) n'est pas un humain : elle ne
   réfléchit pas, ne cherche pas la fin de partie la plus rapide. La comparaison porte sur
   l'**ordre de grandeur** (la moyenne est-elle dans la bande), jamais sur l'égalité.

## 5. Interprétation

- **Hypothèse confirmée** : la bande mesurée (étape 3) contient 40 à 60, ou recouvre la bande du
  rapport. La bande provisoire `DURATION_TARGET_ACTIONS` est remplacée par la bande mesurée
  (arrondie), dans la tâche de consignation.
- **Hypothèse infirmée** : écrire l'écart (bande mesurée contre 40-60, en actions et en
  pourcentage) dans « Résultats », puis nommer le levier de règle **sans l'appliquer ici** :
  - parties trop longues en actions pour tenir 15 minutes : limiter le plateau, ajouter des fins de
    partie directes ; la riposte du héros et la limite de plateau sont des options de règles
    (#127) ; le prix des opérations (#133) et le recalibrage de Ruée et Rempart (#135) agissent sur
    le rythme des decks ;
  - parties trop courtes : l'inverse (riposte du héros, plateau plus large, effets moins
    létaux), toujours via #127, #133, #135.
- **Plafond de sécurité (proposition à part, jamais appliquée)** : si les relevés montrent une
  queue très longue (une partie bien au-delà de 2 fois la durée moyenne), proposer un nombre
  d'actions maximal au-delà duquel une règle de fin forcée s'appliquerait. Ce plafond est une
  décision du propriétaire, ouverte par un ticket séparé : **aucune règle de fin forcée n'est
  introduite par cette story**.

## Résultats

À remplir par la tâche de consignation de #124.
