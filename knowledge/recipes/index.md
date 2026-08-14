# Recipes

A recipe is everything specific to one site generator: hooks, overlay files, the
builder image. The core knows none of it. Separating core from recipe is the only
abstraction v1 needs.

* [Zensical + Obsidian](zensical-obsidian.md) - the reference deployment, with the verified hook assignment
* [Navigation frontmatter convention](nav-frontmatter-convention.md) - how notes declare their place in the menu

# Candidates

Not written yet, in priority order:

* **quartz** - the largest Obsidian publishing community; "Quartz without git" is the missing piece there
* **hugo**
* **mkdocs-material**
