use anyhow::Context;
use axum::{
    body::Body,
    extract::{Request, State},
    http::StatusCode,
    response::Response,
};
use axum_extra::response::ErasedJson;
use bytes::Buf;
use serde::Deserialize;
use serde_json::json;

use crate::{
    jsonrpc_server::RestError,
    models::{
        blockchain::transaction::Transaction,
        peer::{
            transaction_notification::TransactionNotification,
            transfer_transaction::TransactionProofQuality,
        },
        state::mempool::TransactionOrigin,
    },
};

use super::PoolState;

pub async fn get_transaction(State(state): State<PoolState>, req: Request) -> Response {
    let transaction = state.get_most_worth_transaction().unwrap();
    let body = bincode::serialize(&transaction).unwrap();

    let body = Body::from(body);

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/octet-stream")
        .body(body)
        .unwrap()
}

#[derive(Debug, Deserialize, Clone)]
struct BroadcastTx {
    transaction: Transaction,
    height: u64,
    origin: TransactionOrigin,
    notification: TransactionNotification,
}

pub async fn submit_transaction(
    State(state): State<PoolState>,
    body: axum::body::Bytes,
) -> Result<ErasedJson, RestError> {
    let tx: BroadcastTx = bincode::deserialize_from(body.reader()).context("deserialize error")?;
    let id = tx.transaction.kernel.txid().to_string();
    let transaction = bincode::serialize(&tx.transaction).unwrap();
    let fee = tx.transaction.kernel.fee.to_nau();
    state.add_transaction(&id, &transaction, fee)?;

    Ok(ErasedJson::pretty(json!({
        "id": id,
    })))
}

pub async fn submit_single_proof_transaction(
    State(state): State<PoolState>,
    body: axum::body::Bytes,
) -> Result<ErasedJson, RestError> {
    let tx: BroadcastTx = bincode::deserialize_from(body.reader()).context("deserialize error")?;
    let id = tx.transaction.kernel.txid().to_string();
    if tx.transaction.proof.proof_quality()? != TransactionProofQuality::SingleProof {
        return Err(RestError("proof quality is not single proof".to_string()));
    }

    //TODO: broadcast transaction

    state.finish_transaction(&id)?;

    Ok(ErasedJson::pretty(json!({
        "status": "broadcasted"
    })))
}

pub async fn get_transaction_status(
    State(state): State<PoolState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<ErasedJson, RestError> {
    if let Some((_transaction, created, finished)) = state.get_executing_transaction(&id)? {
        if finished > 0 {
            return Ok(ErasedJson::pretty(json!({
                "status": "success",
                "created_at": created,
                "finished_at": finished,
            })));
        }
        return Ok(ErasedJson::pretty(json!({
            "status": "executing",
            "created_at": created,
        })));
    };

    if let Some(_transaction) = state.get_pending_transaction(&id)? {
        return Ok(ErasedJson::pretty(json!({
            "status": "pending"
        })));
    }

    Ok(ErasedJson::pretty(json!({
        "status": "outdated"
    })))
}
