use axum::{
    Router,
    routing::{IntoMakeService, get},
};
use tower_http::{compression::CompressionLayer, validate_request::ValidateRequestHeaderLayer};

use crate::{DeploymentImpl, middleware};

pub mod agent_runs;
pub mod agent_settings;
pub mod agent_tools;
pub mod approvals;
pub mod config;
pub mod containers;
pub mod filesystem;
// pub mod github;
pub mod attachments;
pub mod events;
pub mod execution_bridge;
pub mod execution_processes;
pub mod frontend;
pub mod health;
pub mod integrations;
pub mod local_remote;
pub mod openwiki;
pub mod parallel_context;
pub mod pipelines;
pub mod preview;
pub mod project_chat;
pub mod releases;
pub mod repo;
pub mod resource_coordination;
pub mod scheduled_tasks;
pub mod scratch;
pub mod search;
pub mod sessions;
pub mod tags;
pub mod terminal;
pub mod workflows;
pub mod workspaces;

pub fn router(
    deployment: DeploymentImpl,
    access: server_access::Access,
) -> IntoMakeService<Router> {
    let remote_mcp = crate::remote_mcp::configured_router(deployment.clone(), access.clone())
        .expect("Invalid remote MCP configuration; refusing to start");
    let local_routes = Router::new()
        .route("/health", get(health::health_check))
        .merge(config::router())
        .merge(agent_settings::router())
        .merge(agent_tools::router())
        .merge(containers::router(&deployment))
        .merge(workspaces::router(&deployment))
        .merge(agent_runs::router(&deployment))
        .merge(execution_processes::router(&deployment))
        .merge(tags::router(&deployment))
        .merge(pipelines::router())
        .merge(filesystem::router())
        .merge(repo::router())
        .merge(integrations::router())
        .merge(project_chat::router())
        .merge(resource_coordination::router())
        .merge(execution_bridge::router())
        .merge(events::router(&deployment))
        .merge(approvals::router())
        .merge(scratch::router(&deployment))
        .merge(search::router(&deployment))
        .merge(preview::api_router())
        .merge(releases::router())
        .merge(sessions::router(&deployment))
        .merge(terminal::router())
        .nest("/local", local_remote::router(&deployment))
        .nest("/board", local_remote::board_api::router())
        .nest("/attachments", attachments::routes())
        .with_state(deployment.clone());

    let api_routes = Router::new()
        .merge(local_routes)
        .layer(ValidateRequestHeaderLayer::custom(
            middleware::validate_origin,
        ))
        .layer(axum::middleware::from_fn(middleware::log_server_errors))
        .with_state(deployment);

    access
        .layer(middleware::maintenance::with_maintenance(
            frontend::router(api_routes)
                .merge(remote_mcp)
                .merge(server_access::router(access.clone())),
        ))
        .layer(CompressionLayer::new())
        .into_make_service()
}
