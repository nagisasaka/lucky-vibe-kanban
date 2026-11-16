//! Issue-owned attachments. Legacy Workspace and Project ownership remains intact.
use axum::{
    Router,
    extract::{DefaultBodyLimit, Multipart, Path, State},
    response::{Json, Response},
    routing::get,
};
use db::models::file::File;
use deployment::Deployment;
use sqlx::SqlitePool;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{
    DeploymentImpl,
    error::ApiError,
    routes::attachments::{AttachmentResponse, serve_file_record},
};

async fn require_issue(pool: &SqlitePool, project: Uuid, issue: Uuid) -> Result<(), ApiError> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM local_issues i JOIN projects p ON p.id=i.project_id WHERE i.id=? AND i.project_id=?)")
        .bind(issue).bind(project).fetch_one(pool).await?;
    if !exists {
        return Err(ApiError::BadRequest(
            "Issue not found in this Project".into(),
        ));
    }
    Ok(())
}
async fn files(pool: &SqlitePool, project: Uuid, issue: Uuid) -> Result<Vec<File>, ApiError> {
    require_issue(pool, project, issue).await?;
    Ok(sqlx::query_as("SELECT a.* FROM attachments a JOIN issue_attachments ia ON ia.attachment_id=a.id JOIN local_issues i ON i.id=ia.issue_id WHERE ia.issue_id=? AND i.project_id=? ORDER BY ia.created_at, a.id")
        .bind(issue).bind(project).fetch_all(pool).await?)
}
async fn find(
    pool: &SqlitePool,
    project: Uuid,
    issue: Uuid,
    attachment: Uuid,
) -> Result<File, ApiError> {
    require_issue(pool, project, issue).await?;
    sqlx::query_as("SELECT a.* FROM attachments a JOIN issue_attachments ia ON ia.attachment_id=a.id JOIN local_issues i ON i.id=ia.issue_id WHERE ia.issue_id=? AND a.id=? AND i.project_id=?")
        .bind(issue).bind(attachment).bind(project).fetch_optional(pool).await?
        .ok_or_else(|| ApiError::BadRequest("Attachment not found in this Issue".into()))
}
async fn list(
    State(d): State<DeploymentImpl>,
    Path((project, issue)): Path<(Uuid, Uuid)>,
) -> Result<Json<ApiResponse<Vec<AttachmentResponse>>>, ApiError> {
    Ok(Json(ApiResponse::success(
        files(&d.db().pool, project, issue)
            .await?
            .into_iter()
            .map(AttachmentResponse::from_file)
            .collect(),
    )))
}
async fn upload(
    State(d): State<DeploymentImpl>,
    Path((project, issue)): Path<(Uuid, Uuid)>,
    mut multipart: Multipart,
) -> Result<Json<ApiResponse<AttachmentResponse>>, ApiError> {
    require_issue(&d.db().pool, project, issue).await?;
    while let Some(field) = multipart.next_field().await? {
        if field.name() != Some("image") {
            continue;
        }
        let name = field.file_name().unwrap_or("file.bin").to_owned();
        let data = field.bytes().await?;
        let file = d
            .file()
            .store_issue_file(&data, &name, project, issue)
            .await?;
        return Ok(Json(ApiResponse::success(AttachmentResponse::from_file(
            file,
        ))));
    }
    Err(ApiError::BadRequest("Missing attachment".into()))
}
async fn download(
    State(d): State<DeploymentImpl>,
    Path((project, issue, attachment)): Path<(Uuid, Uuid, Uuid)>,
) -> Result<Response, ApiError> {
    let record = find(&d.db().pool, project, issue, attachment).await?;
    serve_file_record(&d, record, "private, no-store").await
}
pub(super) fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route(
            "/projects/{project_id}/issues/{issue_id}/attachments",
            get(list)
                .post(upload)
                .layer(DefaultBodyLimit::max(20 * 1024 * 1024)),
        )
        .route(
            "/projects/{project_id}/issues/{issue_id}/attachments/{attachment_id}/file",
            get(download),
        )
}

#[cfg(test)]
#[tokio::test]
#[ignore = "run alone in a fresh lvk-issue-attachments-* local instance"]
async fn isolated_issue_attachment_http() -> anyhow::Result<()> {
    use anyhow::ensure;
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;
    let instance = std::path::PathBuf::from(std::env::var("LVK_INSTANCE_DIR")?);
    ensure!(
        instance.is_absolute()
            && instance
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("lvk-issue-attachments-")
    );
    ensure!(!instance.join("data/db.v2.sqlite").exists());
    ensure!(std::env::var("LVK_ACCESS_DEPLOYMENT").as_deref() == Ok("local"));
    ensure!(std::env::var("LVK_AUTH_MODE").as_deref() == Ok("local"));
    ensure!(
        std::env::var("LVK_PUBLIC_URL")
            .unwrap_or_default()
            .is_empty()
    );
    let shutdown = tokio_util::sync::CancellationToken::new();
    let d = DeploymentImpl::new(shutdown.clone()).await?;
    let result = async {
        let pool=&d.db().pool;
        let project=Uuid::new_v4(); let other_project=Uuid::new_v4(); let issue=Uuid::new_v4(); let other_issue=Uuid::new_v4(); let status=Uuid::new_v4();
        for (id,key) in [(project,"ATT"),(other_project,"OTHER")] {
            sqlx::query("INSERT INTO projects(id,name,key) VALUES(?,'attachment fixture',?)").bind(id).bind(key).execute(pool).await?;
        }
        sqlx::query("INSERT INTO local_project_statuses(id,project_id,name,color,sort_order) VALUES(?,?,'Todo','blue',0)").bind(status).bind(project).execute(pool).await?;
        for (id,number) in [(issue,1),(other_issue,2)] {
            sqlx::query("INSERT INTO local_issues(id,project_id,issue_number,simple_id,status_id,title,sort_order) VALUES(?,?,?,?,?,'attachment fixture',0)").bind(id).bind(project).bind(number).bind(format!("ATT-{number}")).bind(status).execute(pool).await?;
        }
        let app=Router::new().merge(router()).nest("/attachments",crate::routes::attachments::routes()).with_state(d.clone());
        let base=format!("/projects/{project}/issues/{issue}/attachments");
        let multipart="--fixture\r\nContent-Disposition: form-data; name=\"image\"; filename=\"evidence.txt\"\r\nContent-Type: text/plain\r\n\r\nissue evidence\r\n--fixture--\r\n";
        let response=app.clone().oneshot(Request::post(&base).header("content-type","multipart/form-data; boundary=fixture").body(Body::from(multipart))?).await?;
        ensure!(response.status()==StatusCode::OK);
        let payload:serde_json::Value=serde_json::from_slice(&to_bytes(response.into_body(),1024*1024).await?)?;
        let attachment=payload["data"]["id"].as_str().unwrap();
        d.file().delete_orphaned_files().await?;
        for uri in [format!("{base}/{attachment}/file"),base.clone()] {
            let response=app.clone().oneshot(Request::get(uri).body(Body::empty())?).await?;
            ensure!(response.status()==StatusCode::OK);
            let bytes=to_bytes(response.into_body(),1024*1024).await?;
            ensure!(!bytes.is_empty());
        }
        let response=app.clone().oneshot(Request::get(format!("{base}/{attachment}/file")).body(Body::empty())?).await?;
        ensure!(response.headers()["cache-control"]=="private, no-store");
        ensure!(to_bytes(response.into_body(),1024).await?.as_ref()==b"issue evidence");
        for uri in [format!("/projects/{project}/issues/{other_issue}/attachments/{attachment}/file"),format!("/projects/{other_project}/issues/{issue}/attachments/{attachment}/file"),format!("/attachments/{attachment}/file")] {
            let response=app.clone().oneshot(Request::get(uri).body(Body::empty())?).await?;
            ensure!(!response.status().is_success());
        }
        let response=app.oneshot(Request::get(format!("/projects/{project}/issues/{other_issue}/attachments")).body(Body::empty())?).await?;
        let payload:serde_json::Value=serde_json::from_slice(&to_bytes(response.into_body(),1024).await?)?;
        ensure!(payload["data"].as_array().unwrap().is_empty());
        anyhow::Ok(())
    }.await;
    shutdown.cancel();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn issue_parent_and_attachment_membership_are_required() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::raw_sql("CREATE TABLE projects(id BLOB PRIMARY KEY); CREATE TABLE local_issues(id BLOB PRIMARY KEY,project_id BLOB); CREATE TABLE attachments(id BLOB PRIMARY KEY,file_path TEXT,original_name TEXT,mime_type TEXT,size_bytes INTEGER,hash TEXT,created_at TEXT,updated_at TEXT); CREATE TABLE issue_attachments(issue_id BLOB,attachment_id BLOB,created_at TEXT DEFAULT '2026-11-17');").execute(&pool).await.unwrap();
        let project = Uuid::new_v4();
        let other_project = Uuid::new_v4();
        let issue = Uuid::new_v4();
        let other_issue = Uuid::new_v4();
        let attachment = Uuid::new_v4();
        sqlx::query("INSERT INTO projects VALUES(?),(?)")
            .bind(project)
            .bind(other_project)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO local_issues VALUES(?,?),(?,?)")
            .bind(issue)
            .bind(project)
            .bind(other_issue)
            .bind(project)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO attachments VALUES(?,'a','a.txt','text/plain',1,'hash','2026-11-17T00:00:00Z','2026-11-17T00:00:00Z')").bind(attachment).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO issue_attachments(issue_id,attachment_id) VALUES(?,?)")
            .bind(issue)
            .bind(attachment)
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(files(&pool, project, issue).await.unwrap().len(), 1);
        assert!(files(&pool, project, other_issue).await.unwrap().is_empty());
        assert!(find(&pool, project, issue, attachment).await.is_ok());
        assert!(find(&pool, project, other_issue, attachment).await.is_err());
        assert!(find(&pool, other_project, issue, attachment).await.is_err());
        assert!(files(&pool, other_project, issue).await.is_err());
    }
}
