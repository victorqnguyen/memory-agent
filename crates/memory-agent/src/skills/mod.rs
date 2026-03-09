pub mod templates;

use memory_core::types::{Memory, SourceType};
use memory_core::{SaveParams, SearchResult, Store};

const PROCEDURAL_TAG: &str = "procedural";

fn skill_tag(name: &str) -> String {
    format!("skill:{name}")
}

fn preference_key(name: &str) -> String {
    format!("preference/{name}")
}

fn procedural_key(name: &str) -> String {
    format!("skill/{name}/procedural")
}

pub struct SkillStartResult {
    /// Procedural memories tagged with `skill:{name}` and `procedural`
    pub procedural_memories: Vec<SearchResult>,
    /// Observed memories at `preference/{skill_name}` scope/key
    pub overrides: Vec<SearchResult>,
    /// General context memories
    pub context_memories: Vec<Memory>,
}

pub fn on_skill_start(
    store: &Store,
    skill_name: &str,
    scope: Option<&str>,
) -> anyhow::Result<SkillStartResult> {
    let tag = skill_tag(skill_name);
    let tag_refs: Vec<&str> = vec![tag.as_str(), PROCEDURAL_TAG];
    let procedural_memories = store.search_by_tags(&tag_refs, scope, 10)?;

    let pref_key = preference_key(skill_name);
    let overrides = store
        .search_by_tags(&[tag.as_str()], scope, 20)?
        .into_iter()
        .filter(|r| r.key.starts_with(&pref_key) || r.source_type == SourceType::Observed)
        .collect();

    let context_memories = store.context(scope, Some(10))?;

    Ok(SkillStartResult {
        procedural_memories,
        overrides,
        context_memories,
    })
}

pub fn on_skill_end(
    store: &Store,
    skill_name: &str,
    scope: Option<&str>,
    outcome: &str,
    files_changed: &[String],
) -> anyhow::Result<Option<i64>> {
    let content = match templates::extract_procedural_memory(skill_name, outcome, files_changed) {
        Some(c) => c,
        None => return Ok(None),
    };

    let key = procedural_key(skill_name);
    let tags = vec![skill_tag(skill_name), PROCEDURAL_TAG.to_string()];

    let action = store.save(SaveParams {
        key,
        value: content,
        scope: scope.map(str::to_string),
        source_type: Some(SourceType::Procedural),
        source_ref: None,
        source_commit: None,
        tags: Some(tags),
    })?;

    Ok(Some(action.id()))
}
