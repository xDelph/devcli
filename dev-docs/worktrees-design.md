# devcli + Git worktrees — conception et plan d'implémentation

Statut : **conception, aucun code écrit**.
Auteur : herdr · Date : 2026-10-08

---

## 1. Le problème

Les worktrees git se créent à la volée, typiquement par des agents qui travaillent
sur plusieurs branches en parallèle. Aujourd'hui, lancer la même app depuis N
worktrees impose N entrées dupliquées dans `~/.devcli/config.json`, et les
process collide sur les ports.

Modèle actuel :

```
config.json :  projects → apps → { path, commands, … }
identité      :  process_id = "project.app.env"  →  state store "project.app.env.json"
```

Une app porte déjà son `path`, donc **rien n'empêche techniquement** deux worktrees
de coexister. Ce qui manque, c'est (a) de ne pas dupliquer la config et (b) un
identifiant par worktree.

---

## 2. Faits git (vérifiés expérimentalement)

Toutes ces données viennent d'expériences locales réelles, pas de mémoire.

| Question | Réponse vérifiée |
|---|---|
| Format de listing | `git worktree list --porcelain` → blocs `worktree <path>`, `HEAD <sha>`, `branch refs/heads/<b>` ou `detached` |
| Deux worktrees, même branche, sans `--force` | **Refusé** par git (« already checked out ») |
| Deux worktrees, même branche, **avec `--force`** | **Autorisé** — et `branch` est **identique** pour les deux |
| Identifiant unique et stable par worktree | **`.git/worktrees/<nom>`** — un répertoire admin par worktree, y compris en `--force` |
| Comment obtenir l'admin dir | `git rev-parse --git-dir` depuis le worktree → `…/repo/.git/worktrees/wt-feat` |
| Racine commune à tous les worktrees | `git rev-parse --git-common-dir` |
| Nommage de l'admin dir | Dérivé du **nom du dossier** du worktree (`wt-deep`), **pas** de la branche |
| Branche contenant `/` | Aucun impact : `feature/deep/nested` est bien affichée en `refs/heads/feature/deep/nested` |
| Worktree supprimé via `git worktree remove` | **Disparaît immédiatement** du listing |
| Worktree supprimé via `rm -rf` (cas agent) | **Reste listé comme stale** ; `git worktree prune` le retire (« gitdir file points to non-existent location ») |

### Conséquence pour le naming

Avec `--force`, deux worktrees partagent la même branche → **la branche ne peut
pas servir de clé d'unicité**. L'identifiant fiable est l'admin dir
(`.git/worktrees/<slug>`), unique par construction.

Stratégie de slug retenue :

```
slug = nom du dossier du worktree, slugifié
ex.  …/.worktrees/feature-login-a  →  feature-login-a
```

Uniqueness : le slug dérive du dossier, donc unique dans un même parent. En cas de
collision (deux dossiers de même nom, p. ex. worktrees dispersés), suffixer
`-2`, `-3`… **par ordre déterministe** (tri des chemins). C'est déterministe et
reproductible tant que l'ensemble des worktrees ne change pas.

---

## 3. Décision d'architecture : dériver, ne rien persister

L'état des worktrees est exactement `git worktree list`. Il n'a donc **aucune
raison d'être stocké**.

**On ne touche pas à `config.json`.** Un état dérivé est recalculé à chaque
commande :

```
config.json déclare :   projects: { "api": { apps: { api: { path: "~/code/api" } } } }

à l'exécution, git dit qu'il y a 3 worktrees :
  ~/code/api              → projet "api"          (déclaré)
  ~/code/.worktrees/a     → projet "api@a"       (dérivé, hérite de "api")
  ~/code/.worktrees/b     → projet "api@b"       (dérivé)
```

Bénéfices :

- **Aucun fichier supplémentaire** : pas de churn, pas de diff, pas de commit.
- **L'auto-nettoyage est gratuit** : un worktree supprimé n'apparaît plus au
  calcul suivant. Rien à nettoyer côté config.
- **Héritage implicite** : un projet dérivé hérite de la config du projet de base,
  en ne changeant que son `path`.
- **Pas de collision d'identité** : `api@a.api.local` est unique, le state store
  `project.app.env.json` reste sûr.
- **Zéro régression** : le comportement actuel est strictement préservé.

### Ce qu'on refuse explicitement

Écrire les worktrees dans `config.json`. Cela mélangerait état rédigé (humain) et
état dérivé (machine), ferait churner le fichier à chaque création/suppression, et
obligerait devcli à supprimer des lignes dans un fichier que l'utilisateur
écrit aussi.

---

## 4. Identité runtime et ports (le vrai bloqueur)

Si la même app tourne dans 3 worktrees, les trois se battent sur `:3000`. Sans
désambiguïsation, les worktrees sont inutilisables en parallèle — le cas d'usage
principal.

Solution : devcli injecte des variables d'environnement par worktree, déjà
interpolables par `env-flow` (qui expose un `interpolator` et
`EnvFlow::interpolate`) :

```
DEVCLI_WORKTREE      = slug          ("a")
DEVCLI_WORKTREE_IDX  = index         (0, 1, 2 — tri déterministe)
DEVCLI_WORKTREE_PORT = base + idx    (3000, 3001, 3002)
```

Config d'app (inchangée pour l'utilisateur non concerné) :

```jsonc
"api": {
  "path": "~/code/api",
  "commands": { "local": { "dev": "npm run dev -- --port ${DEVCLI_WORKTREE_PORT}" } }
}
```

---

## 5. Sécurité : worktree supprimé pendant que les process tournent

Un worktree supprimé laisse des process orphelins, avec un cwd disparu, qui
continuent de tourner et de tenir des ports. Règle :

- si un process suivi a un **cwd disparu** → devcli l'**arrête** et le signale ;
- jamais de kill « par similarité de nom » : uniquement par pid **et**
  vérification que le cwd est gone.

Détection d'un worktree stale : `git worktree list --porcelain` le liste encore,
mais son dossier n'existe plus. On ne fait pas confiance au listing seul — on
`stat` le path. Éventuellement `git worktree prune` pour nettoyer l'entrée git.

---

## 6. Résolution d'ambiguïté des commandes

Avec 3 worktrees, `devcli start api` est ambigu. Règles :

| Commande | Effet |
|---|---|
| `api` | projet de base seul |
| `api@a` | worktree slug `a` |
| `api@*` | tous les worktrees (pratique : tester N branches en parallèle) |
| chemin explicite (`--path`) | ciblage direct, sans ambiguïté |

---

## 7. Plan de tâches

Légende : **[P]** = prérequis bloquant · **[I]** = indépendant, parallélisable ·
**D** = dépendance.

Critère de parallélisation : chaque tâche est décrite par ses entrées/sorties,
de sorte que deux tâches [I] ne se recouvrent pas et ne modifient pas le même
fichier sans contrat explicite.

### Phase 0 — Socle de détection (aucune modification de comportement)

| # | Tâche | Fichiers | Dep | Parallèle |
|---|---|---|---|---|
| 0.1 | Prototype CLI : lister worktrees d'un repo, slugifier, déterministe | *(prototype hors prod)* | — | ✔ |
| 0.2 | Figer le format de sortie (contrat testable : slug, idx, path, branch, detached) | — | 0.1 | ✔ |
| 0.3 | Décider base port (config ? convention ? defaut 3000) | — | — | ✔ |

### Phase 1 — Lecture seule (aucune écriture de config)

| # | Tâche | Fichiers | Dep | Parallèle |
|---|---|---|---|---|
| 1.1 | `worktrees::discover(common_dir)` : parse `--porcelain` → structs | nouveau `devcli-core/src/worktrees.rs` | 0.2 | ✔ |
| 1.2 | Slug + collision suffixing déterministe | `worktrees.rs` | 1.1 | ✔ |
| 1.3 | Détection stale (path existe ?) | `worktrees.rs` | 1.1 | ✔ |
| 1.4 | Tests unitaires : slug, collisions, detached, stale | `worktrees.rs` | 1.2,1.3 | ✔ |

### Phase 2 — Résolution de projets dérivés (lecture seule)

| # | Tâche | Fichiers | Dep | Parallèle |
|---|---|---|---|---|
| 2.1 | Résolution « projet courant » : base + dérivés, sans écriture | `worktrees.rs` | 1.4 | — |
| 2.2 | Groupement dans `status` (affichage) | `commands/status.rs` | 2.1 | ✔ |
| 2.3 | `DEVCLI_WORKTREE*` dans l'env du process | `commands/start/executor.rs` | 2.1 | ✔ |
| 2.4 | Test : pas de régression quand aucun worktree | — | 2.1 | ✔ |

### Phase 3 — Écriture/arrêt ciblés

| # | Tâche | Fichiers | Dep | Parallèle |
|---|---|---|---|---|
| 3.1 | Résolution `api@a` et `api@*` | `commands/start/resolver.rs` | 2.1 | ✔ |
| 3.2 | `stop` / `restart` ciblent le bon worktree | `commands/stop.rs`, `restart.rs` | 3.1 | ✔ |
| 3.3 | Arrêt auto des process à cwd disparu + signalement | `process_manager_support.rs` | 1.3 | ✔ |

### Phase 4 — Documentation

| # | Tâche | Fichiers | Dep | Parallèle |
|---|---|---|---|---|
| 4.1 | README : section worktrees | `README.md` | 2.2 | ✔ |
| 4.2 | `AGENTS.md` : commandes `api@a`, `api@*` | `AGENTS.md` | 3.2 | ✔ |

**Ordre de commit** : une phase = un commit. Phases 1 et 2 sont non-breaking
(aucune écriture) et livrables indépendamment.

---

## 8. Décisions en attente

1. **Base des ports** : où vit-elle (config, convention, défaut) ?
2. **Commande de listage** : un `devcli worktrees` explicite, ou seulement
   l'affichage dans `status` ?
3. **`git worktree prune`** : devcli le déclenche-t-il, ou seulement avertit ?