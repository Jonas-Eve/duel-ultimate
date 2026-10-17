# Maquette des cartes (Epic #86)

Copie versionnée de la maquette produite avec le propriétaire, pour ne pas dépendre de la page privée où elle a été dessinée. Les stories qui s'y réfèrent : #87 (apparence des cartes compactes), #90 (détail au toucher et actions depuis le détail), #88 et #89 (illustrations).

## Fichiers

| Fichier | Contenu |
|---|---|
| `Main.dc.html` | Plateau à 390 px : cartes compactes (main, plateau propre, plateau adverse) |
| `DetailRuee.dc.html` | Détail d'une carte en main, avec « Jouer (N temps) » |
| `DetailRempart.dc.html` | Détail d'une carte adverse, en lecture seule |
| `DetailFutur.dc.html` | **Exemple futur, pas une carte du jeu** : créature à plusieurs effets actifs |
| `canvas.json` | Disposition des feuilles et notes de légende |

Ce sont des sources d'un outil de maquettes (balises `<x-dc>`, `sc-for`, `sc-if`, script `support.js` non fourni) : elles ne s'ouvrent pas telles quelles dans un navigateur. Elles se lisent comme du HTML et du CSS annotés ; les valeurs (couleurs, tailles) y sont exactes. Ce fichier résume ce qu'il faut en retenir.

## Règles de la maquette

- **Trois valeurs sur toute carte**, toujours affichées, même à 0 : attaque (étoile), riposte (flèche de retour), vie (cœur), en une rangée en bas. Chaque valeur a une icône de forme différente et un chiffre : aucune information par la couleur seule.
- **Carte compacte, taille fixe**, quels que soient le contenu et l'image (illustration rognée, nom tronqué). Maquette : 66 × 112 px ; #87 retient 64 px de large pour tenir cinq cartes en main à 360 px. Chiffres d'au moins 10 px.
- **Carte en main** : le coût d'invocation (petite pastille avec sablier) est à côté du nom. **Carte sur le terrain** : pas de coût, seulement le nom, les valeurs et les effets permanents (étiquette « Provocation » sous la carte).
- **Cadre commun** à toutes les cartes (fond parchemin `#f6edd9`/`#e9dcbd`, bordure brune `#4a3a22`, filet doré `#c9a24a`, mêmes coins). Les decks se distinguent par un **symbole** : éclair pour Ruée, créneaux pour Rempart, en pastille sur l'illustration (carte compacte) et dans l'angle en haut à droite (détail).
- **Détail** (#90) : toucher une carte l'ouvre. Carte de hauteur fixe (520 px), illustration de hauteur fixe (230 px) avec ou sans actions, zone d'effets défilante. Effets actifs en une ligne « Nom (⏱ coût) : inflige N dégâts », effets passifs expliqués ; en bas, attaque et riposte à gauche, vie à droite. Carte adverse : lecture seule. Carte en main : « Jouer (N temps) » et « Fermer ». Créature du plateau : toucher la ligne d'un effet actif, puis choix de la cible.
- **Modèle** : l'attaque est un effet actif parmi d'autres ; la riposte est une valeur ; les effets passifs sont permanents et sans coût. Ce qui dépasse l'état du moteur (attaque rapide, dégâts fixes, « Ne peut pas attaquer », attaque à 0) relève de la tranche « effets de créature » de l'Epic #5 et n'apparaît que dans `DetailFutur`, étiqueté comme exemple.
