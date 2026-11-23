use chrono::Duration;
use serde_json::json;

use super::*;
fn issue(n: u128) -> Issue {
    serde_json::from_value(json!({"id":Uuid::from_u128(n),"project_id":Uuid::from_u128(1000),"status_id":Uuid::from_u128(10),"issue_number":n as i32,"simple_id":format!("LOCAL-{n}"),"title":"Release","description":"A literal  phrase","priority":"high","sort_order":n as f64,"extension_metadata":{},"created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-02T00:00:00Z"})).unwrap()
}
fn corpus() -> SavedFilterCorpus {
    SavedFilterCorpus {
        issues: vec![issue(1), issue(2), issue(3)],
        statuses: vec![],
        tags: vec![],
        issue_tags: vec![],
        assignees: vec![],
        relationships: vec![],
    }
}
fn ids(ns: &[u128]) -> Vec<Uuid> {
    ns.iter().map(|n| Uuid::from_u128(*n)).collect()
}
#[test]
fn saved_filter_combines_fields_tags_archive_and_literal_search() {
    let mut c = corpus();
    c.issues[2].archived = true;
    for (i, t) in [(1, 20), (1, 21), (2, 20), (3, 21)] {
        c.issue_tags.push(IssueTag {
            id: Uuid::new_v4(),
            issue_id: Uuid::from_u128(i),
            tag_id: Uuid::from_u128(t),
        });
    }
    c.assignees.push(IssueAssignee {
        id: Uuid::new_v4(),
        issue_id: Uuid::from_u128(1),
        user_id: Uuid::from_u128(50),
        assigned_at: Utc::now(),
    });
    let mut f = MachineFilter {
        tag_ids: ids(&[20, 21]),
        ..Default::default()
    };
    let now = Utc::now();
    assert_eq!(machine::candidate_ids(&c, &f, now), ids(&[1, 2]));
    f.tag_mode = SavedFilterTagMode::All;
    assert_eq!(machine::candidate_ids(&c, &f, now), ids(&[1]));
    f.assignee_ids = ids(&[50, 51]);
    f.status_ids = ids(&[10, 11]);
    f.priorities = vec![IssuePriority::High, IssuePriority::Low];
    f.search = Some("LITERAL  PHRASE".into());
    assert_eq!(machine::candidate_ids(&c, &f, now), ids(&[1]));
    f.search = Some("literal phrase".into());
    assert!(machine::candidate_ids(&c, &f, now).is_empty());
    f = MachineFilter {
        archived: SavedFilterArchive::Archived,
        ..Default::default()
    };
    assert_eq!(machine::candidate_ids(&c, &f, now), ids(&[3]));
    f.archived = SavedFilterArchive::All;
    assert_eq!(machine::candidate_ids(&c, &f, now), ids(&[1, 2, 3]));
}
#[test]
fn saved_filter_period_boundaries_and_time_only_changes() {
    let mut c = corpus();
    let now = c.issues[0].created_at;
    c.issues[1].created_at = now - Duration::seconds(60);
    c.issues[2].created_at = now - Duration::seconds(61);
    let mut f = MachineFilter {
        created_at: Some(SavedFilterPeriod::Relative { seconds: 60 }),
        ..Default::default()
    };
    assert_eq!(machine::candidate_ids(&c, &f, now), ids(&[1, 2]));
    assert_eq!(
        machine::candidate_ids(&c, &f, now + Duration::seconds(1)),
        ids(&[1])
    );
    f.created_at = Some(SavedFilterPeriod::Absolute {
        start: Some(now - Duration::seconds(60)),
        end: Some(now),
    });
    assert_eq!(machine::candidate_ids(&c, &f, now), ids(&[2]));
}
#[test]
fn saved_filter_relation_direction_reach_and_cycle_termination() {
    let mut c = corpus();
    c.issues[1].parent_issue_id = Some(c.issues[0].id);
    c.issues[2].parent_issue_id = Some(c.issues[1].id);
    let mut f = MachineFilter {
        relation: Some(SavedFilterRelation {
            kind: SavedFilterRelationKind::Children,
            target_issue_id: c.issues[0].id,
        }),
        ..Default::default()
    };
    assert_eq!(machine::candidate_ids(&c, &f, Utc::now()), ids(&[2]));
    f.relation.as_mut().unwrap().kind = SavedFilterRelationKind::Descendants;
    assert_eq!(machine::candidate_ids(&c, &f, Utc::now()), ids(&[2, 3]));
    c.issues[0].parent_issue_id = Some(c.issues[2].id);
    assert_eq!(machine::candidate_ids(&c, &f, Utc::now()), ids(&[2, 3]));
    f.relation.as_mut().unwrap().kind = SavedFilterRelationKind::Parent;
    assert_eq!(machine::candidate_ids(&c, &f, Utc::now()), ids(&[3]));
    c.relationships.push(IssueRelationship {
        id: Uuid::new_v4(),
        issue_id: c.issues[0].id,
        related_issue_id: c.issues[1].id,
        relationship_type: IssueRelationshipType::Blocking,
        created_at: Utc::now(),
    });
    f.relation.as_mut().unwrap().kind = SavedFilterRelationKind::BlockedBy;
    assert_eq!(machine::candidate_ids(&c, &f, Utc::now()), ids(&[2]));
    f.relation.as_mut().unwrap().kind = SavedFilterRelationKind::Blocking;
    assert!(machine::candidate_ids(&c, &f, Utc::now()).is_empty());
    f.relation.as_mut().unwrap().target_issue_id = c.issues[1].id;
    assert_eq!(machine::candidate_ids(&c, &f, Utc::now()), ids(&[1]));
    c.relationships[0].relationship_type = IssueRelationshipType::Related;
    f.relation.as_mut().unwrap().kind = SavedFilterRelationKind::Related;
    assert_eq!(machine::candidate_ids(&c, &f, Utc::now()), ids(&[1]));
}

#[tokio::test]
async fn saved_filter_full_corpus_adapter_and_paging_after_evaluation() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    db::run_migrations(&pool).await.unwrap();
    let project = Uuid::new_v4();
    let status = Uuid::new_v4();
    sqlx::query("INSERT INTO projects(id,name,key) VALUES(?,'many Issues','MANY')")
        .bind(project)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO local_project_statuses(id,project_id,name,color,sort_order) VALUES(?,?,'Todo','blue',0)").bind(status).bind(project).execute(&pool).await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    for n in 1..=505 {
        sqlx::query("INSERT INTO local_issues(id,project_id,issue_number,simple_id,status_id,title,description,sort_order,archived) VALUES(?,?,?,?,?,'Release','full body',0,?)").bind(Uuid::from_u128(n)).bind(project).bind(n as i32).bind(format!("MANY-{n}")).bind(status).bind(n==505).execute(&mut *tx).await.unwrap();
    }
    tx.commit().await.unwrap();
    let value = store::write(
        &pool,
        project,
        "test",
        SavedFilterWrite::Create(CreateSavedFilterRequest {
            request_id: Uuid::new_v4(),
            definition: SavedFilterDefinition {
                name: "many".into(),
                machine: MachineFilter::default(),
                natural_language: None,
                context: Some("Release context".into()),
            },
        }),
    )
    .await
    .unwrap();
    let definition: SavedFilter = serde_json::from_value(value).unwrap();
    let page = evaluate_service(
        &pool,
        None,
        project,
        definition.id,
        &SavedFilterPage {
            offset: 500,
            limit: Some(2),
        },
        EvaluationOptions::default(),
    )
    .await
    .unwrap();
    assert_eq!(page.total_count, 504);
    assert_eq!(page.issue_ids, ids(&[501, 502]));
    assert_eq!(page.next_offset, Some(502));
    assert_eq!(page.state, SavedFilterEvaluationState::Complete);
    let corpus = load_corpus(&pool, project).await.unwrap();
    assert_eq!(corpus.issues.len(), 505);
    let request = semantic_request(&definition, &corpus, ids(&[501, 502]));
    assert_eq!(
        request.issues.len(),
        505,
        "corpus includes archived and machine-excluded Issues"
    );
    assert_eq!(request.context["definition_context"], "Release context");
    assert_eq!(request.context["statuses"][0]["name"], "Todo");
    assert_eq!(
        request.issues[504].evidence["issue"]["description"],
        "full body"
    );
    assert_eq!(request.issues[504].evidence["issue"]["archived"], true);
    let changed = semantic_request(&definition, &corpus, ids(&[1]));
    assert_eq!(
        serde_json::to_value(request.issues).unwrap(),
        serde_json::to_value(changed.issues).unwrap(),
        "time-only candidates do not change semantic evidence"
    );
}
