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
    let first = evaluate_service(
        &pool,
        None,
        project,
        definition.id,
        &SavedFilterPage::default(),
        EvaluationOptions::default(),
    )
    .await
    .unwrap();
    let page = evaluate_service(
        &pool,
        None,
        project,
        definition.id,
        &SavedFilterPage {
            offset: 500,
            limit: Some(2),
            expected_evaluation_revision: Some(first.evaluation_revision.clone()),
        },
        EvaluationOptions::default(),
    )
    .await
    .unwrap();
    assert_eq!(page.total_count, 504);
    assert_eq!(page.issue_ids, ids(&[501, 502]));
    assert_eq!(page.next_offset, Some(502));
    assert_eq!(page.state, SavedFilterEvaluationState::Complete);
    assert_eq!(page.evaluation_revision, first.evaluation_revision);
    // Same count, state and definition revision, but different membership.
    sqlx::query("UPDATE local_issues SET archived = CASE WHEN issue_number = 1 THEN 1 ELSE 0 END WHERE project_id=?").bind(project).execute(&pool).await.unwrap();
    let later = SavedFilterPage {
        offset: 500,
        limit: Some(2),
        expected_evaluation_revision: Some(first.evaluation_revision.clone()),
    };
    assert!(matches!(
        evaluate_service(
            &pool,
            None,
            project,
            definition.id,
            &later,
            EvaluationOptions::default()
        )
        .await,
        Err(ApiError::SavedFilterEvaluationChanged)
    ));
    let restarted = evaluate_service(
        &pool,
        None,
        project,
        definition.id,
        &SavedFilterPage::default(),
        EvaluationOptions::default(),
    )
    .await
    .unwrap();
    assert_eq!(restarted.total_count, first.total_count);
    assert_eq!(restarted.definition_revision, first.definition_revision);
    assert_ne!(restarted.evaluation_revision, first.evaluation_revision);
    assert!(matches!(
        evaluate_service(
            &pool,
            None,
            project,
            definition.id,
            &SavedFilterPage {
                offset: 1,
                ..Default::default()
            },
            EvaluationOptions::default()
        )
        .await,
        Err(ApiError::BadRequest(_))
    ));
    // Restore the full-corpus fixture for adapter assertions below.
    sqlx::query("UPDATE local_issues SET archived = (issue_number = 505) WHERE project_id=?")
        .bind(project)
        .execute(&pool)
        .await
        .unwrap();
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

#[test]
fn saved_filter_unassigned_or_users_and_exact_simple_id() {
    let mut c = corpus();
    for (issue_id, user_id) in [(1, 50), (3, 51)] {
        c.assignees.push(IssueAssignee {
            id: Uuid::new_v4(),
            issue_id: Uuid::from_u128(issue_id),
            user_id: Uuid::from_u128(user_id),
            assigned_at: Utc::now(),
        });
    }
    let old: MachineFilter = serde_json::from_value(json!({"assignee_ids":ids(&[50])})).unwrap();
    assert!(!old.include_unassigned);
    assert!(old.simple_id.is_none());
    assert_eq!(machine::candidate_ids(&c, &old, Utc::now()), ids(&[1]));
    let mut f = MachineFilter {
        include_unassigned: true,
        ..Default::default()
    };
    assert_eq!(machine::candidate_ids(&c, &f, Utc::now()), ids(&[2]));
    f.assignee_ids = ids(&[50]);
    assert_eq!(machine::candidate_ids(&c, &f, Utc::now()), ids(&[1, 2]));
    f.simple_id = Some("local-2".into());
    assert_eq!(machine::candidate_ids(&c, &f, Utc::now()), ids(&[2]));
    f.simple_id = Some("2".into());
    assert!(machine::candidate_ids(&c, &f, Utc::now()).is_empty());
    f.simple_id = None;
    f.status_ids = ids(&[999]);
    assert!(machine::candidate_ids(&c, &f, Utc::now()).is_empty());
}

#[tokio::test]
async fn saved_filter_relative_time_invalidates_result_pages_without_definition_changes() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    db::run_migrations(&pool).await.unwrap();
    let project = Uuid::new_v4();
    let status = Uuid::new_v4();
    sqlx::query("INSERT INTO projects(id,name,key) VALUES(?,'time','TIME')")
        .bind(project)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO local_project_statuses(id,project_id,name,color,sort_order) VALUES(?,?,'Todo','blue',0)").bind(status).bind(project).execute(&pool).await.unwrap();
    let now = issue(1).created_at;
    // One expires exactly as another enters: total_count stays two.
    for (n, seconds) in [(1, -60), (2, 0), (3, 1)] {
        sqlx::query("INSERT INTO local_issues(id,project_id,issue_number,simple_id,status_id,title,sort_order,created_at) VALUES(?,?,?,?,?,'Time',0,?)").bind(Uuid::from_u128(n)).bind(project).bind(n as i32).bind(format!("TIME-{n}")).bind(status).bind(now+Duration::seconds(seconds)).execute(&pool).await.unwrap();
    }
    let value = store::write(
        &pool,
        project,
        "test",
        SavedFilterWrite::Create(CreateSavedFilterRequest {
            request_id: Uuid::new_v4(),
            definition: SavedFilterDefinition {
                name: "time".into(),
                machine: MachineFilter {
                    created_at: Some(SavedFilterPeriod::Relative { seconds: 60 }),
                    ..Default::default()
                },
                natural_language: None,
                context: None,
            },
        }),
    )
    .await
    .unwrap();
    let filter: SavedFilter = serde_json::from_value(value).unwrap();
    let first = evaluate_service(
        &pool,
        None,
        project,
        filter.id,
        &SavedFilterPage {
            limit: Some(1),
            ..Default::default()
        },
        EvaluationOptions {
            now,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(first.issue_ids, ids(&[1]));
    let later = SavedFilterPage {
        offset: 1,
        limit: Some(1),
        expected_evaluation_revision: Some(first.evaluation_revision.clone()),
    };
    assert!(
        evaluate_service(
            &pool,
            None,
            project,
            filter.id,
            &later,
            EvaluationOptions {
                now,
                ..Default::default()
            }
        )
        .await
        .is_ok()
    );
    assert!(matches!(
        evaluate_service(
            &pool,
            None,
            project,
            filter.id,
            &later,
            EvaluationOptions {
                now: now + Duration::seconds(1),
                ..Default::default()
            }
        )
        .await,
        Err(ApiError::SavedFilterEvaluationChanged)
    ));
    let next = evaluate_service(
        &pool,
        None,
        project,
        filter.id,
        &SavedFilterPage::default(),
        EvaluationOptions {
            now: now + Duration::seconds(1),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(next.issue_ids, ids(&[2, 3]));
    assert_eq!(next.total_count, first.total_count);
    assert_eq!(next.definition_revision, first.definition_revision);
    assert_ne!(next.evaluation_revision, first.evaluation_revision);
}

/// Production HTTP auth/router coverage in a task-owned disposable instance.
#[tokio::test]
#[ignore = "run alone with fresh local466-http-* LVK_INSTANCE_DIR and local auth"]
async fn isolated_saved_filter_http() -> anyhow::Result<()> {
    use anyhow::ensure;
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;
    let root = std::path::PathBuf::from(std::env::var("LVK_INSTANCE_DIR")?);
    ensure!(
        root.is_absolute()
            && root
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("local466-http-")
    );
    ensure!(!root.join("data/db.v2.sqlite").exists());
    ensure!(std::env::var("LVK_ACCESS_DEPLOYMENT").as_deref() == Ok("local"));
    ensure!(std::env::var("LVK_AUTH_MODE").as_deref() == Ok("local"));
    let shutdown = tokio_util::sync::CancellationToken::new();
    let d = DeploymentImpl::new(shutdown.clone()).await?;
    let result=async {
        let project=Uuid::new_v4();let other=Uuid::new_v4();
        for (id,key) in [(project,"FIRST"),(other,"SECOND")] {sqlx::query("INSERT INTO projects(id,name,key) VALUES(?,'HTTP fixture',?)").bind(id).bind(key).execute(&d.db().pool).await?;}
        let access=server_access::Access::from_env(d.db().pool.clone(),utils::assets::asset_dir())?;
        let app=access.layer(axum::Router::new().nest("/api",super::router()).with_state(d.clone()));
        let request=|method:&str,path:String,value:serde_json::Value,csrf:bool| {
            let mut request=Request::builder().method(method).uri(path).header("host","localhost").header("content-type","application/json");
            if csrf {request=request.header("x-lvk-csrf","local");}
            request.body(Body::from(value.to_string())).unwrap()
        };
        let path=format!("/api/projects/{project}/saved-filters");
        let input=json!({"request_id":Uuid::new_v4(),"definition":{"name":"HTTP","machine":{},"natural_language":null,"context":"Current release"}});
        ensure!(app.clone().oneshot(request("POST",path.clone(),input.clone(),false)).await?.status()==StatusCode::FORBIDDEN);
        let response=app.clone().oneshot(request("POST",path.clone(),input.clone(),true)).await?;
        ensure!(response.status()==StatusCode::OK);
        let saved:serde_json::Value=serde_json::from_slice(&to_bytes(response.into_body(),65536).await?)?;
        let id:Uuid=serde_json::from_value(saved["data"]["id"].clone())?;
        let replay=app.clone().oneshot(request("POST",path.clone(),input,true)).await?;
        let replay:serde_json::Value=serde_json::from_slice(&to_bytes(replay.into_body(),65536).await?)?;
        ensure!(saved==replay);
        ensure!(app.clone().oneshot(request("GET",format!("/api/projects/{other}/saved-filters/{id}"),json!(null),false)).await?.status()==StatusCode::NOT_FOUND);
        let response=app.clone().oneshot(request("POST",format!("{path}/{id}/evaluate"),json!({}),true)).await?;
        ensure!(response.status()==StatusCode::OK);
        let evaluation:serde_json::Value=serde_json::from_slice(&to_bytes(response.into_body(),65536).await?)?;
        ensure!(evaluation["data"]["state"]=="complete" && evaluation["data"]["total_count"]==0 && evaluation["data"]["stale"]==false);
        let bad_page=app.clone().oneshot(request("GET",format!("{path}/{id}/results?offset=1&expected_evaluation_revision=old"),json!(null),false)).await?;
        ensure!(bad_page.status()==StatusCode::CONFLICT);
        let missing_token=app.clone().oneshot(request("GET",format!("{path}/{id}/results?offset=1"),json!(null),false)).await?;
        ensure!(missing_token.status()==StatusCode::BAD_REQUEST);
        let changed=json!({"request_id":Uuid::new_v4(),"expected_revision":1,"definition":{"name":"Renamed","machine":{},"natural_language":null,"context":null}});
        ensure!(app.clone().oneshot(request("PUT",format!("{path}/{id}"),changed,true)).await?.status()==StatusCode::OK);
        let stale=json!({"request_id":Uuid::new_v4(),"expected_revision":1});
        ensure!(app.clone().oneshot(request("DELETE",format!("{path}/{id}"),stale,true)).await?.status()==StatusCode::CONFLICT);
        let response=app.clone().oneshot(request("GET",format!("{path}/{id}/link"),json!(null),false)).await?;
        let link:serde_json::Value=serde_json::from_slice(&to_bytes(response.into_body(),65536).await?)?;
        ensure!(link["data"]["url"]==format!("/projects/{project}?savedFilter={id}"));
        ensure!(app.clone().oneshot(request("DELETE",format!("{path}/{id}"),json!({"request_id":Uuid::new_v4(),"expected_revision":2}),true)).await?.status()==StatusCode::OK);
        ensure!(app.oneshot(request("GET",format!("{path}/{id}/link"),json!(null),false)).await?.status()==StatusCode::NOT_FOUND);
        ensure!(sqlx::query_scalar::<_,i64>("SELECT COUNT(*) FROM agent_runs").fetch_one(&d.db().pool).await?==0);
        println!("Saved-filter production HTTP routes: CSRF, CRUD/replay, revisions, Project isolation, machine-only evaluation and stable/deleted links passed");
        anyhow::Ok(())
    }.await;
    shutdown.cancel();
    result
}
