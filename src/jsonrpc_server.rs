use std::net::SocketAddr;
use std::str::FromStr;

use axum::extract::{Path, State};
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use axum_extra::response::ErasedJson;
use tasm_lib::prelude::Digest;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::{
    models::blockchain::block::block_selector::BlockSelector, rpc_server::NeptuneRPCServer,
};

/// An enum of error handlers for the REST API server.
#[derive(Debug)]
pub struct RestError(pub String);

impl IntoResponse for RestError {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Something went wrong: {}", self.0),
        )
            .into_response()
    }
}

impl From<anyhow::Error> for RestError {
    fn from(err: anyhow::Error) -> Self {
        Self(err.to_string())
    }
}

pub(crate) async fn run_rpc_server(
    rest_listener: TcpListener,
    rpcstate: NeptuneRPCServer,
) -> Result<(), anyhow::Error> {
    let cors = CorsLayer::new()
        .allow_origin(tower_http::cors::Any)
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::OPTIONS,
        ])
        .allow_headers([axum::http::header::CONTENT_TYPE]);

    let router = {
        let routes = axum::Router::new()
            .route(
                "/block/height/{height}",
                axum::routing::get(get_block_by_height),
            )
            .route(
                "/block/digest/{digest}",
                axum::routing::get(get_block_by_digest),
            )
            .route("/block/tip", axum::routing::get(get_tip))
            .route(
                "/utxo_digest/{leaf_index}",
                axum::routing::get(get_utxo_digest),
            );

        routes
            // Pass in `Rest` to make things convenient.
            .with_state(rpcstate)
            // Enable tower-http tracing.
            .layer(TraceLayer::new_for_http())
            // Enable CORS.
            .layer(cors)
    };

    axum::serve(
        rest_listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}

async fn get_block(
    rpcstate: NeptuneRPCServer,
    block_selector: BlockSelector,
) -> Result<ErasedJson, RestError> {
    let state = rpcstate.state.lock_guard().await;
    let Some(digest) = block_selector.as_digest(&state).await else {
        return Err(RestError("block not found".to_owned()));
    };
    let archival_state = state.chain.archival_state();
    let Some(block) = archival_state.get_block(digest).await? else {
        return Err(RestError("block not found".to_owned()));
    };

    Ok(ErasedJson::pretty(block.block_with_invalid_proof()))
}
async fn get_block_by_height(
    State(rpcstate): State<NeptuneRPCServer>,
    Path(height): Path<u64>,
) -> Result<ErasedJson, RestError> {
    let block_selector = BlockSelector::Height(height.into());

    get_block(rpcstate, block_selector).await
}

async fn get_block_by_digest(
    State(rpcstate): State<NeptuneRPCServer>,
    Path(digest): Path<String>,
) -> Result<ErasedJson, RestError> {
    let block_selector =
        BlockSelector::Digest(Digest::from_str(&digest).map_err(|e| RestError(e.to_string()))?);

    get_block(rpcstate, block_selector).await
}

async fn get_tip(State(rpcstate): State<NeptuneRPCServer>) -> Result<ErasedJson, RestError> {
    let block_selector = BlockSelector::Tip;

    get_block(rpcstate, block_selector).await
}

async fn get_utxo_digest(
    State(rpcstate): State<NeptuneRPCServer>,
    Path(leaf_index): Path<u64>,
) -> Result<ErasedJson, RestError> {
    let state = rpcstate.state.lock_guard().await;
    let aocl = &state.chain.archival_state().archival_mutator_set.ams().aocl;

    let digest = match leaf_index > 0 && leaf_index < aocl.num_leafs().await {
        true => Some(aocl.get_leaf_async(leaf_index).await),
        false => None,
    };

    Ok(ErasedJson::pretty(digest))
}
