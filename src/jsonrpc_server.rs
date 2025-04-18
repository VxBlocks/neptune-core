use std::collections::HashMap;
use std::net::SocketAddr;

use crate::models::blockchain::transaction::Transaction;
use crate::models::peer::transaction_notification::TransactionNotification;
use crate::models::proof_abstractions::timestamp::Timestamp;
use crate::models::state::mempool::TransactionOrigin;
use crate::tx_pool::{self, PoolState};
use crate::RPCServerToMain;
use anyhow::Context;
use axum::body::Body;
use axum::extract::{DefaultBodyLimit, Path, Request, State};
use axum::Json;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use axum_extra::response::ErasedJson;
use block_selector::BlockSelectorExtended;
use bytes::Buf;
use itertools::Itertools;
use num_traits::Zero;
use serde::{Deserialize, Serialize};
use tasm_lib::prelude::Digest;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;

use crate::models::blockchain::block::block_height::BlockHeight;
use crate::models::blockchain::block::block_info::BlockInfo;
use crate::models::blockchain::type_scripts::native_currency_amount::NativeCurrencyAmount;
use crate::rpc_server::MempoolTransactionInfo;
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
    pool_state: PoolState,
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
                "/rpc/block/{*block_selector}",
                axum::routing::get(get_block),
            )
            .route(
                "/rpc/batch_block/{height}/{batch_size}",
                axum::routing::get(get_batch_block),
            )
            .route(
                "/rpc/block_info/{*block_selector}",
                axum::routing::get(get_block_info),
            )
            .route(
                "/rpc/utxo_digest/{leaf_index}",
                axum::routing::get(get_utxo_digest),
            )
            .route(
                "/rpc/mempool/{start_index}/{number}",
                axum::routing::get(get_mempool),
            )
            .route(
                "/rpc/blocks_time/{start}/{end}",
                axum::routing::get(get_blocks_time),
            )
            .route(
                "/rpc/tx/submit_tx",
                axum::routing::post(tx_pool::router::submit_transaction)
                    .with_state(pool_state.clone()),
            )
            .route(
                "/rpc/get_tx_job",
                axum::routing::get(tx_pool::router::get_transaction).with_state(pool_state.clone()),
            )
            .route(
                "/rpc/tx_job_status/{id}",
                axum::routing::get(tx_pool::router::get_transaction_status).with_state(pool_state),
            )
            .route(
                "/rpc/tx/broadcast",
                axum::routing::post(broadcast_transaction),
            )
            .route("/rpc/getnonces/{count}", axum::routing::get(get_nonces))
            .route(
                "/rpc/getlastblocks/{count}",
                axum::routing::get(get_last_blocks),
            )
            .route(
                "/rpc/owner_blocks/{start}/{end}",
                axum::routing::get(get_owner_blocks),
            );

        routes
            // Pass in `Rest` to make things convenient.
            .with_state(rpcstate)
            // Enable tower-http tracing.
            .layer(TraceLayer::new_for_http())
            .layer(DefaultBodyLimit::disable())
            // .layer(RequestBodyLimitLayer::new(200 * 1000 * 1000))
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
    State(rpcstate): State<NeptuneRPCServer>,
    Path(block_selector): Path<BlockSelectorExtended>,
) -> Result<ErasedJson, RestError> {
    let block_selector = BlockSelector::from(block_selector);
    let state = rpcstate.state.lock_guard().await;
    let Some(digest) = block_selector.as_digest(&state).await else {
        return Ok(ErasedJson::pretty(Option::<crate::Block>::None));
    };
    let archival_state = state.chain.archival_state();
    let Some(block) = archival_state.get_block(digest).await? else {
        return Ok(ErasedJson::pretty(Option::<crate::Block>::None));
    };

    Ok(ErasedJson::pretty(block.block_with_invalid_proof()))
}

async fn get_batch_block(
    State(rpcstate): State<NeptuneRPCServer>,
    Path((height, batch_size)): Path<(u64, u64)>,
) -> Result<Vec<u8>, RestError> {
    let mut blocks = Vec::with_capacity(batch_size as usize);
    for cur_height in height..height + batch_size {
        let block_selector = BlockSelector::Height(cur_height.into());
        let state = rpcstate.state.lock_guard().await;
        let Some(digest) = block_selector.as_digest(&state).await else {
            break;
        };
        let archival_state = state.chain.archival_state();
        let Some(block) = archival_state.get_block(digest).await? else {
            break;
        };

        blocks.push(block.block_with_invalid_proof());
    }

    bincode::serialize(&blocks).map_err(|e| RestError(e.to_string()))
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
async fn get_block_info(
    State(rpcstate): State<NeptuneRPCServer>,
    Path(block_selector): Path<BlockSelectorExtended>,
) -> Result<ErasedJson, RestError> {
    let block_selector = BlockSelector::from(block_selector);
    let state = rpcstate.state.lock_guard().await;
    let Some(digest) = block_selector.as_digest(&state).await else {
        return Ok(ErasedJson::pretty(Option::<BlockInfo>::None));
    };
    let tip_digest = state.chain.light_state().hash();
    let archival_state = state.chain.archival_state();

    let Some(block) = archival_state.get_block(digest).await.unwrap() else {
        return Ok(ErasedJson::pretty(Option::<BlockInfo>::None));
    };
    let is_canonical = archival_state
        .block_belongs_to_canonical_chain(digest)
        .await;

    // sibling blocks are those at the same height, with different digest
    let sibling_blocks = archival_state
        .block_height_to_block_digests(block.header().height)
        .await
        .into_iter()
        .filter(|d| *d != digest)
        .collect();

    let block_info = BlockInfo::new(
        &block,
        archival_state.genesis_block().hash(),
        tip_digest,
        sibling_blocks,
        is_canonical,
    );

    Ok(ErasedJson::pretty(block_info))
}

async fn get_mempool(
    State(rpcstate): State<NeptuneRPCServer>,
    Path((start_index, number)): Path<(usize, usize)>,
) -> Result<ErasedJson, RestError> {
    let global_state = rpcstate.state.lock_guard().await;
    let mempool_txkids = global_state
        .mempool
        .get_sorted_iter()
        .skip(start_index)
        .take(number)
        .map(|(txkid, _)| txkid)
        .collect_vec();

    let (incoming, outgoing): (HashMap<_, _>, HashMap<_, _>) = {
        let (incoming_iter, outgoing_iter) = global_state.wallet_state.mempool_balance_updates();
        (incoming_iter.collect(), outgoing_iter.collect())
    };

    let tip_msah = global_state
        .chain
        .light_state()
        .mutator_set_accumulator_after()
        .hash();

    let mempool_transactions = mempool_txkids
        .iter()
        .filter_map(|id| {
            let mut mptxi = global_state
                .mempool
                .get(*id)
                .map(|tx| (MempoolTransactionInfo::from(tx), tx.kernel.mutator_set_hash))
                .map(|(mptxi, tx_msah)| {
                    if tx_msah == tip_msah {
                        mptxi.synced()
                    } else {
                        mptxi
                    }
                });
            if mptxi.is_some() {
                if let Some(pos_effect) = incoming.get(id) {
                    mptxi = Some(mptxi.unwrap().with_positive_effect_on_balance(*pos_effect));
                }
                if let Some(neg_effect) = outgoing.get(id) {
                    mptxi = Some(mptxi.unwrap().with_negative_effect_on_balance(*neg_effect));
                }
            }

            mptxi
        })
        .collect_vec();

    Ok(ErasedJson::pretty(mempool_transactions))
}

#[derive(Debug, Serialize, Clone, Copy)]
struct BlockTime {
    height: u64,
    time: u64,
}

async fn get_blocks_time(
    State(rpcstate): State<NeptuneRPCServer>,
    Path((start, end)): Path<(u64, u64)>,
) -> Result<ErasedJson, RestError> {
    let mut block_time_list = Vec::with_capacity((end - start + 1) as usize);
    let state = rpcstate.state.lock_guard().await;
    for cur_height in start..=end {
        let block_selector = BlockSelector::Height(cur_height.into());
        let Some(digest) = block_selector.as_digest(&state).await else {
            break;
        };
        let archival_state = state.chain.archival_state();
        let Some(block) = archival_state.get_block(digest).await? else {
            break;
        };

        block_time_list.push(BlockTime {
            height: block.header().height.into(),
            time: block.header().timestamp.to_millis() / 1000,
        });
    }

    Ok(ErasedJson::pretty(block_time_list))
}

async fn get_nonces(
    State(rpcstate): State<NeptuneRPCServer>,
    Path(count): Path<u64>,
) -> Result<ErasedJson, RestError> {
    let state = rpcstate.state.lock_guard().await;
    let end: u64 = state
        .chain
        .archival_state()
        .get_tip()
        .await
        .header()
        .height
        .into();
    let start = end - count + 1;

    let mut block_time_list = Vec::with_capacity((end - start + 1) as usize);
    let mut count = 0;
    for cur_height in start..=end {
        let block_selector = BlockSelector::Height(cur_height.into());
        let Some(digest) = block_selector.as_digest(&state).await else {
            break;
        };
        let archival_state = state.chain.archival_state();
        let Some(block) = archival_state.get_block(digest).await? else {
            break;
        };

        block_time_list.push(block.header().nonce.to_hex());
        if block
            .header()
            .nonce
            .to_hex()
            .starts_with("0000000000000000")
        {
            count += 1;
        }
    }

    let aaa = (
        format!(
            "block ({}-{}), {}/{} = {}%",
            start,
            end,
            count,
            end - start + 1,
            (count * 100) / (end - start + 1)
        ),
        block_time_list,
    );

    Ok(ErasedJson::pretty(aaa))
}

#[derive(Debug, Serialize, Clone)]
struct SimpleBlock {
    height: u64,
    hash: String,
    fee: String,
    timestamp: u64,
}

async fn get_last_blocks(
    State(rpcstate): State<NeptuneRPCServer>,
    Path(count): Path<u64>,
) -> Result<ErasedJson, RestError> {
    let state = rpcstate.state.lock_guard().await;
    let end: u64 = state
        .chain
        .archival_state()
        .get_tip()
        .await
        .header()
        .height
        .into();
    let start = end - count + 1;

    let mut block_time_list = Vec::with_capacity((end - start + 1) as usize);
    for cur_height in start..=end {
        let block_selector = BlockSelector::Height(cur_height.into());
        let Some(digest) = block_selector.as_digest(&state).await else {
            break;
        };
        let archival_state = state.chain.archival_state();
        let Some(block) = archival_state.get_block(digest).await? else {
            break;
        };

        block_time_list.push(SimpleBlock {
            height: block.header().height.into(),
            hash: block.hash().to_hex(),
            fee: block.body().transaction_kernel.fee.to_string(),
            timestamp: block.header().timestamp.to_millis(),
        });
    }

    Ok(ErasedJson::pretty(block_time_list))
}

async fn get_owner_blocks(
    State(rpcstate): State<NeptuneRPCServer>,
    Path((start, end)): Path<(u64, u64)>,
) -> Result<ErasedJson, RestError> {
    let state = rpcstate.state.lock_guard().await;

    let mut owner_block_list = Vec::new();
    let mut reward = NativeCurrencyAmount::zero();
    for cur_height in start..=end {
        let block_selector = BlockSelector::Height(cur_height.into());
        let Some(digest) = block_selector.as_digest(&state).await else {
            break;
        };
        let archival_state = state.chain.archival_state();
        let Some(block) = archival_state.get_block(digest).await? else {
            break;
        };

        let guesser_digest = state
            .wallet_state
            .wallet_entropy
            .guesser_spending_key(block.header().prev_block_digest)
            .after_image();

        if guesser_digest == block.header().guesser_digest {
            reward = reward + block.body().transaction_kernel.fee;
            owner_block_list.push(RewardCard {
                block_id: block.hash(),
                block_height: block.header().height,
                timestamp: block.header().timestamp,
                amount: block.body().transaction_kernel.fee.to_string(),
            });
        }
    }

    let guess_reward = GuessReward {
        start: start.into(),
        end: end.into(),
        reward: reward.to_string(),
        records: owner_block_list,
    };

    Ok(ErasedJson::pretty(guess_reward))
}

#[derive(Debug, Serialize, Clone)]
struct GuessReward {
    start: BlockHeight,
    end: BlockHeight,
    reward: String,
    records: Vec<RewardCard>,
}

#[derive(Debug, Serialize, Clone)]
struct RewardCard {
    block_id: Digest,
    block_height: BlockHeight,
    timestamp: Timestamp,
    amount: String,
}

#[derive(Debug, Deserialize, Clone)]
struct BroadcastTx {
    transaction: Transaction,
    origin: TransactionOrigin,
    notification: TransactionNotification,
}
async fn broadcast_transaction(
    State(mut rpcstate): State<NeptuneRPCServer>,
    body: axum::body::Bytes,
) -> Result<ErasedJson, RestError> {
    let tx: BroadcastTx = bincode::deserialize_from(body.reader()).context("deserialize error")?;
    let tx_id = tx.transaction.kernel.txid();
    let mut state = rpcstate.state.lock_guard_mut().await;
    state.mempool_insert(tx.transaction, tx.origin).await;
    let _ = rpcstate
        .rpc_server_to_main_tx
        .send(RPCServerToMain::BroadcastNotification(tx.notification))
        .await;

    Ok(ErasedJson::pretty(tx_id.to_string()))
}

mod block_selector {
    use std::str::FromStr;

    use serde::de::Error;
    use serde::{Deserialize, Deserializer};

    use crate::models::blockchain::block::block_selector::{
        BlockSelector, BlockSelectorParseError,
    };

    use height_or_digest::HeightOrDigest;

    /// newtype for `BlockSelector` that provides ability to parse `height_or_digest/value`.
    ///
    /// This is useful for HTML form(s) that allow user to enter either height or
    /// digest into the same text input field.
    ///
    /// In particular it is necessary to support javascript-free website with such
    /// an html form.
    #[derive(Debug, Clone, Copy)]
    pub struct BlockSelectorExtended(BlockSelector);

    impl std::fmt::Display for BlockSelectorExtended {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    impl FromStr for BlockSelectorExtended {
        type Err = BlockSelectorParseError;

        // note: this parses BlockSelector, plus height_or_digest/<value>
        fn from_str(s: &str) -> Result<Self, Self::Err> {
            match BlockSelector::from_str(s) {
                Ok(bs) => Ok(Self::from(bs)),
                Err(e) => {
                    let parts: Vec<_> = s.split('/').collect();
                    if parts.len() == 2 && parts[0] == "height_or_digest" {
                        Ok(Self::from(HeightOrDigest::from_str(parts[1])?))
                    } else {
                        Err(e)
                    }
                }
            }
        }
    }

    // note: axum uses serde Deserialize for Path elements.
    impl<'de> Deserialize<'de> for BlockSelectorExtended {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            let s = String::deserialize(deserializer)?;
            Self::from_str(&s).map_err(D::Error::custom)
        }
    }

    impl From<HeightOrDigest> for BlockSelectorExtended {
        fn from(hd: HeightOrDigest) -> Self {
            Self(hd.into())
        }
    }

    impl From<BlockSelector> for BlockSelectorExtended {
        fn from(v: BlockSelector) -> Self {
            Self(v)
        }
    }

    impl From<BlockSelectorExtended> for BlockSelector {
        fn from(v: BlockSelectorExtended) -> Self {
            v.0
        }
    }

    mod height_or_digest {
        use crate::models::blockchain::block::block_height::BlockHeight;
        use crate::models::blockchain::block::block_selector::BlockSelector;
        use crate::models::blockchain::block::block_selector::BlockSelectorParseError;
        use crate::prelude::tasm_lib::prelude::Digest;
        use serde::{Deserialize, Serialize};
        use std::str::FromStr;

        /// represents either a block-height or a block digest
        #[derive(Debug, Clone, Copy, Serialize, Deserialize)]
        pub enum HeightOrDigest {
            /// Identifies block by Digest (hash)
            Digest(Digest),
            /// Identifies block by Height (count from genesis)
            Height(BlockHeight),
        }

        impl std::fmt::Display for HeightOrDigest {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    Self::Digest(d) => write!(f, "{}", d),
                    Self::Height(h) => write!(f, "{}", h),
                }
            }
        }

        impl FromStr for HeightOrDigest {
            type Err = BlockSelectorParseError;

            // note: this parses the output of impl Display for HeightOrDigest
            // note: this is used by clap parser in neptune-cli for block-info command
            //       and probably future commands as well.
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok(match s.parse::<u64>() {
                    Ok(h) => Self::Height(h.into()),
                    Err(_) => Self::Digest(Digest::try_from_hex(s)?),
                })
            }
        }

        impl From<HeightOrDigest> for BlockSelector {
            fn from(hd: HeightOrDigest) -> Self {
                match hd {
                    HeightOrDigest::Height(h) => Self::Height(h),
                    HeightOrDigest::Digest(d) => Self::Digest(d),
                }
            }
        }
    }
}
