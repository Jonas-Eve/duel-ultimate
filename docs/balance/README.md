# Équilibrage : mesures et protocoles

Index des documents de mesure du simulateur (`duel-simulator`, Epic #120).

**Règle : une mesure = commande + graine + commit + date.** Un nombre sans ces quatre éléments
n'est pas une mesure : il ne se compare à rien et ne se reproduit pas.

- [`throughput.md`](throughput.md) : le débit du simulateur (parties par seconde), son protocole,
  la règle de comparaison entre machines, le seuil de régression et le tableau daté des mesures
  (#123).
- `reports/` : rapports d'équilibrage consignés (à venir).

Aucune mesure de débit ne tourne en CI, et aucun seuil n'est épinglé dans un test.
