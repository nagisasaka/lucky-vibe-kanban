//! Native repository skill discovery. Roots are supplied by Workspace ownership,
//! never by recursively registering arbitrary directories below a working folder.
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

/// A native discovery cache belongs to exactly one set of extra repositories.
pub fn discovery_cache_key(base: String, roots: &[PathBuf]) -> String {
    if roots.is_empty() {
        return base;
    }
    let mut scope = roots.to_vec();
    scope.sort();
    scope.dedup();
    format!("{base}:repository-roots:{scope:?}")
}

/// Creation UI discovers source checkouts before worktrees exist. Explicit
/// repository skill selections must execute the corresponding worktree version.
pub fn retarget_selected_skills(
    skills: &mut [crate::actions::SelectedSkill],
    mappings: &[(PathBuf, PathBuf)],
) {
    for skill in skills {
        if let Some((source, target)) = mappings
            .iter()
            .filter(|(source, _)| skill.path.starts_with(source))
            .max_by_key(|(source, _)| source.components().count())
        {
            let relative = skill.path.strip_prefix(source).expect("matched prefix");
            if !relative
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
            {
                skill.path = target.join(relative);
            }
        }
    }
}

pub fn codex_roots(repos: &[PathBuf]) -> Vec<PathBuf> {
    repos
        .iter()
        .map(|r| r.join(".agents/skills"))
        .filter(|p| p.is_dir())
        .collect()
}

pub fn claude_skills(repos: &[PathBuf]) -> BTreeMap<String, Vec<PathBuf>> {
    let mut result: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
    for repo in repos {
        for (folder, skill) in [("skills", true), ("commands", false)] {
            let base = repo.join(".claude").join(folder);
            if let Ok(entries) = std::fs::read_dir(&base) {
                for entry in entries.flatten() {
                    let path = if skill {
                        entry.path().join("SKILL.md")
                    } else {
                        entry.path()
                    };
                    if !path.is_file() || path.extension().is_none_or(|e| e != "md") {
                        continue;
                    }
                    let name = if skill {
                        entry.file_name().to_string_lossy().into_owned()
                    } else {
                        path.file_stem()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned()
                    };
                    let name = if skill {
                        read_skill_name(&path).unwrap_or(name)
                    } else {
                        name
                    };
                    result.entry(name).or_default().push(path);
                }
            }
        }
    }
    result
}

fn read_skill_name(path: &Path) -> Option<String> {
    use std::io::{BufRead, Read};
    let file = std::fs::File::open(path).ok()?;
    let mut lines = std::io::BufReader::new(file.take(64 * 1024)).lines();
    if lines.next()?.ok()?.trim() != "---" {
        return None;
    }
    for line in lines.take(128) {
        let line = line.ok()?;
        if line.trim() == "---" {
            break;
        }
        if let Some(name) = line.strip_prefix("name:") {
            let name = name.trim();
            let name = if name.starts_with(['\"', '\'']) {
                name.trim_matches(['\"', '\''])
            } else {
                name.split(" #").next()?.trim()
            };
            return (!name.is_empty()).then(|| name.to_owned());
        }
    }
    None
}

pub fn ambiguous_claude_names(repos: &[PathBuf]) -> Vec<String> {
    let mut counts = BTreeMap::<String, usize>::new();
    // Count the actual .claude directory per root, not ancestor path prefixes:
    // source repositories may be nested before their worktrees are created.
    let unique: std::collections::BTreeSet<_> = repos.iter().collect();
    for repo in unique {
        for name in claude_skills(std::slice::from_ref(repo)).into_keys() {
            *counts.entry(name).or_default() += 1;
        }
    }
    counts
        .into_iter()
        .filter_map(|(name, count)| (count > 1).then_some(name))
        .collect()
}

pub fn mentions_skill(prompt: &str, name: &str) -> bool {
    prompt.split_whitespace().any(|word| {
        let word = word.trim_matches(|c: char| {
            matches!(
                c,
                '`' | '\"' | '\'' | ',' | '.' | ':' | ';' | '(' | ')' | '[' | ']'
            )
        });
        word == format!("${name}") || word == format!("/{name}")
    })
}

pub fn workspace_repos(root: &Path, names: &[String]) -> Vec<PathBuf> {
    names
        .iter()
        .filter(|n| {
            matches!(
                (
                    Path::new(n).components().next(),
                    Path::new(n).components().nth(1)
                ),
                (Some(std::path::Component::Normal(_)), None)
            )
        })
        .map(|n| root.join(n))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn discovery_scopes_never_share_workspace_catalogs() {
        let a = PathBuf::from("/workspace-a/repo");
        let b = PathBuf::from("/workspace-b/repo");
        assert_ne!(
            discovery_cache_key("profile".into(), std::slice::from_ref(&a)),
            discovery_cache_key("profile".into(), std::slice::from_ref(&b))
        );
        assert_eq!(
            discovery_cache_key("profile".into(), &[a.clone(), b.clone()]),
            discovery_cache_key("profile".into(), &[b, a])
        );
    }

    #[test]
    fn explicit_source_skills_follow_worktree_versions_without_changing_global_skills() {
        use crate::actions::SelectedSkill;
        let mut skills = vec![
            SelectedSkill {
                name: "same".into(),
                path: "/source/a/.agents/skills/check/SKILL.md".into(),
            },
            SelectedSkill {
                name: "same".into(),
                path: "/source/a/nested/.agents/skills/check/SKILL.md".into(),
            },
            SelectedSkill {
                name: "global".into(),
                path: "/user/skills/global/SKILL.md".into(),
            },
        ];
        retarget_selected_skills(
            &mut skills,
            &[
                ("/source/a".into(), "/work/a".into()),
                ("/source/a/nested".into(), "/work/b".into()),
            ],
        );
        assert_eq!(
            skills[0].path,
            PathBuf::from("/work/a/.agents/skills/check/SKILL.md")
        );
        assert_eq!(
            skills[1].path,
            PathBuf::from("/work/b/.agents/skills/check/SKILL.md")
        );
        assert_eq!(
            skills[2].path,
            PathBuf::from("/user/skills/global/SKILL.md")
        );
    }

    #[test]
    fn codex_roots_are_owned_skill_containers_not_repository_roots() {
        let tmp = tempfile::tempdir().unwrap();
        let names = vec!["a".into(), "b".into(), "../outside".into()];
        let repos = workspace_repos(tmp.path(), &names);
        assert_eq!(repos, vec![tmp.path().join("a"), tmp.path().join("b")]);
        std::fs::create_dir_all(repos[0].join(".agents/skills")).unwrap();
        assert_eq!(codex_roots(&repos), vec![repos[0].join(".agents/skills")]);
        assert!(codex_roots(&workspace_repos(tmp.path(), &["other".into()])).is_empty());
    }

    #[test]
    fn duplicate_names_keep_both_paths_and_are_not_silently_invoked() {
        let tmp = tempfile::tempdir().unwrap();
        let roots: Vec<_> = ["a", "b"].iter().map(|r| tmp.path().join(r)).collect();
        for r in &roots {
            let p = r.join(".claude/skills/check");
            std::fs::create_dir_all(&p).unwrap();
            std::fs::write(p.join("SKILL.md"), "---\nname: inspect\n---\nScoped skill").unwrap();
        }
        assert_eq!(claude_skills(&roots)["inspect"].len(), 2);
        assert_eq!(ambiguous_claude_names(&roots), vec!["inspect"]);
        assert!(mentions_skill("Use $inspect here", "inspect"));
        assert!(!mentions_skill("Use $inspection", "inspect"));
        assert!(mentions_skill("Use `/inspect`, please", "inspect"));
    }
}
