use std::collections::{HashMap, HashSet};

use api_types::*;
use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;

fn in_period(time: DateTime<Utc>, period: &SavedFilterPeriod, now: DateTime<Utc>) -> bool {
    match period {
        SavedFilterPeriod::Absolute { start, end } => {
            start.is_none_or(|s| time >= s) && end.is_none_or(|e| time < e)
        }
        SavedFilterPeriod::Relative { seconds } => {
            time >= now - Duration::seconds(i64::from(*seconds)) && time <= now
        }
    }
}
fn related(c: &SavedFilterCorpus, r: &SavedFilterRelation) -> HashSet<Uuid> {
    use SavedFilterRelationKind::*;
    let target = r.target_issue_id;
    let mut ids = HashSet::new();
    match r.kind {
        Parent => {
            ids.extend(
                c.issues
                    .iter()
                    .find(|i| i.id == target)
                    .and_then(|i| i.parent_issue_id),
            );
        }
        Children | Descendants => {
            let mut children: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
            for i in &c.issues {
                if let Some(parent) = i.parent_issue_id {
                    children.entry(parent).or_default().push(i.id);
                }
            }
            let mut frontier = vec![target];
            let mut visited = HashSet::from([target]);
            while let Some(parent) = frontier.pop() {
                for id in children.get(&parent).into_iter().flatten() {
                    if visited.insert(*id) {
                        ids.insert(*id);
                        if r.kind == Descendants {
                            frontier.push(*id);
                        }
                    }
                }
            }
        }
        Related | Blocking | BlockedBy => {
            for edge in &c.relationships {
                match r.kind {
                    Related if edge.relationship_type == IssueRelationshipType::Related => {
                        if edge.issue_id == target {
                            ids.insert(edge.related_issue_id);
                        }
                        if edge.related_issue_id == target {
                            ids.insert(edge.issue_id);
                        }
                    }
                    Blocking
                        if edge.relationship_type == IssueRelationshipType::Blocking
                            && edge.related_issue_id == target =>
                    {
                        ids.insert(edge.issue_id);
                    }
                    BlockedBy
                        if edge.relationship_type == IssueRelationshipType::Blocking
                            && edge.issue_id == target =>
                    {
                        ids.insert(edge.related_issue_id);
                    }
                    _ => {}
                }
            }
        }
    }
    ids.remove(&target);
    ids
}
/// Evaluate the full corpus, preserving deterministic Issue-number order.
pub fn candidate_ids(c: &SavedFilterCorpus, f: &MachineFilter, now: DateTime<Utc>) -> Vec<Uuid> {
    let relation = f.relation.as_ref().map(|r| related(c, r));
    let mut tags: HashMap<Uuid, HashSet<Uuid>> = HashMap::new();
    for t in &c.issue_tags {
        tags.entry(t.issue_id).or_default().insert(t.tag_id);
    }
    let assignees: HashSet<_> = c
        .assignees
        .iter()
        .filter(|a| f.assignee_ids.contains(&a.user_id))
        .map(|a| a.issue_id)
        .collect();
    let mut issues: Vec<_> = c
        .issues
        .iter()
        .filter(|i| {
            (match f.archived {
                SavedFilterArchive::Active => !i.archived,
                SavedFilterArchive::Archived => i.archived,
                SavedFilterArchive::All => true,
            }) && db::models::text_search::matches(
                f.search.as_deref(),
                &[&i.title, i.description.as_deref().unwrap_or("")],
            ) && (f.status_ids.is_empty() || f.status_ids.contains(&i.status_id))
                && (f.priorities.is_empty()
                    || i.priority.is_some_and(|p| f.priorities.contains(&p)))
                && (f.assignee_ids.is_empty() || assignees.contains(&i.id))
                && (f.tag_ids.is_empty()
                    || match f.tag_mode {
                        SavedFilterTagMode::Any => tags
                            .get(&i.id)
                            .is_some_and(|tags| f.tag_ids.iter().any(|id| tags.contains(id))),
                        SavedFilterTagMode::All => tags
                            .get(&i.id)
                            .is_some_and(|tags| f.tag_ids.iter().all(|id| tags.contains(id))),
                    })
                && f.created_at
                    .as_ref()
                    .is_none_or(|p| in_period(i.created_at, p, now))
                && f.updated_at
                    .as_ref()
                    .is_none_or(|p| in_period(i.updated_at, p, now))
                && relation.as_ref().is_none_or(|ids| ids.contains(&i.id))
        })
        .collect();
    issues.sort_by_key(|i| (i.issue_number, i.id));
    issues.into_iter().map(|i| i.id).collect()
}
