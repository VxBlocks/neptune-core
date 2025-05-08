use std::collections::HashMap;
use std::net::SocketAddr;

use crate::api::export::{Network, ReceivingAddress};
use crate::models::blockchain::transaction::utxo::Utxo;
use crate::models::blockchain::transaction::Transaction;
use crate::models::peer::transaction_notification::TransactionNotification;
use crate::models::proof_abstractions::timestamp::Timestamp;
use crate::models::state::mempool::TransactionOrigin;
use crate::models::state::wallet::transaction_output::TxOutput;
use crate::models::state::wallet::utxo_notification::UtxoNotifyMethod;
use crate::tx_pool::{self, PoolState};
use crate::util_types::mutator_set::addition_record::AdditionRecord;
use crate::util_types::mutator_set::archival_mutator_set::{
    MsMembershipProofEx, RequestMsMembershipProofEx,
};
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

const FEE_ADDRESS: &str = "nolgam1nurfm22evhpscn5ddwgwa96z0048454c84hwapmvqq6rqqwqx4w34kudq6q5adjvgch8f8v9dsfz3h0vk60npzya04248umqq2xs9n9cznxzl92nh65k6pg60jesff6wu77l8e3c2h8yyjtwwd9kz00m6z7nl5vxk5929q34837shxn4x5t6p9wgheljlfs3kp7lnrl2z0an80y50lwzm704svvpw3ze5k9fkccttuhunjn96cr3jcgt80qggj5x9ltta5z3qmyxhxxmz9ns7kddcrtun0mfd5fz2d05xnkhjzp3pphc83jytrecc437gf7e9czqh9qfhw5000f43ghyc2dfa5vcl38rwzax27kuv0e0gtkj7q2ar3dt0q6y32fdp9nhtm9l4crg7ud7w6vlg28ncns5q4f86teneuu8ezs2zur30gscw5qk9dgmter2nzryph5k2r68k5xf5pf7lkjas9km6eu6jjl2ujfjv5572xqrdrymm3mne6gptpvg54qxfwp3kkm45fvc5knjecsv7w5dfx82u9kcl5mrdd39k8dgc6gddty49f4yy32nfczhxq0k5dx5qmyet273mz6ggthrtvsxtteg3ceg366pnhmgaplejmjgq7qyyc0vz43ecvry8k7p7ddysqutxgpm6w950mzcxcppe5rm6pkjv9tv5uxyx3kz8lpd744udfc8h0575lfkxuwfp4y3uf9nu3fzj8x2r4gt8y3wtwdlf3flldp0m289jc3lh0dv9372dxk7fddx3ns9acfz7cdxsluucxnrn7e8p7lx5h3ngztft68ae5fcnplekay90kvnqjnxr3e80q4xl0nufucchr66p6swa2gkptf85304wwjktllz7f2sswpx3qkpld8mku900jz0g6e2q9y806enem49qud89uqu6z8d98v9sux5anr2v88hr80jqz7t7g4dcj5spgnc0l996lrq0hfswzfwldx7klsxk82zlpfzwpfgkmu3gkdyqnh9salfwrckn95tk0k0kyhrkchhaplehldfj5wf6dnkhapaxhzwfzu8gglp2rf3jtpx7ew3hlq6yqtxtrfxu0ctwsycj9eqccnlpg77mjs292t39kz4n99vjd2yejuxztk4828yk2wk5urejc3fd00gwqmcxl4k2pw85vmxrvv8n9dv6amcgkmuhgfzfcy3wm0p5yhtvdhs4l0447au6x7kwdhmuxjgk7x80gtdmgd74zswdw0jkngwef2zctxnuktxp4e5fqftgw0yplq0d3lcrcqg6q3rw5ljc654adhee53xmmeaazg0avtzkt2q0ngsq8xuxxcax8u2x9zhcxjltcsewhe7ffzqrkznv3z3vuhar4whazsergmymz4jx2d3l8qwrlhcducztkkeygm8luwnrmh2fcrpkg79gj34u88e72ljt94aapkn5uunu457h2kc3czpgekjl2wjyuz9wcpyfk3z22xx7lx7etchn5mfqxpvjf63wcy0sd9qap8mwnmfzs5j4zh9jv8n8jdwvjyk5d3x0j42cdvh5zhq00g429j0vrvm8097vfq2fg2axhrzfuy6qv97swl39dm3q859guyk4pqv9a82kz5wgnvs84l9g3g5wjf9z888spenf97ddaprkxvxluhg268hst8jgfa78t4nrqklgvw6f630nt4yrsddwahmfcfux9gmt0zjyg9vkfrfct8qtg9lehrvgmwq4e7h6ys6r34l2xn82fy2ey5wwq0jn6vk52vugmzlpgc0aywltxqzn7dvz6dlec98en9f482vdmhf33th0k5nrpwq3qj6xg7ve09nna3kp3ff4nhknt4etqhzauc8v2047yl72yefh4zddc6g9s4ye4hvukulhhu37gqrll7qyg0sx6gtgalwgwcc50gd00m90vzca8mxykdqjhfesxre99ahmfcpa2xtqftzlvu8ag55wqm84rqapa06774v876lms39y5mx0r67mus4n45crh4j99f6wptmcmy9q8hqlnl8qgvxetx3ce3kla74uwuleh7jkzdpafgcvl7amv0s8usgg6z2nr3utc4xg5qgzaf5zw3tjnak72e0ptl86k5d2667pkzauq35c7x83tms2ysev6x20h5am89qu6mm77f8f7cemtd4hhxh4qp6ae55krpst59656mqzpzc8uup42mxrarc298n7y86ekgrgft3nkasfa30u9w50dxt6gx3rpyvpgsyv8nz3d0dhzgdtkt7gxd6nj02awyesdmncj0pwzdp59gh2c09rqfm7x8t7le70ej2dd7ncq2z2qwl0cphu8ds5hxzegur3mlrrqx0zdvmje79s86ads9v6srn2skztz7mlr47f2xs43tt2eejx0j66ukqusg2ltjjxe79efggq022u9j8dqd6qcuedrfhhm8rqg6na9rcuq35aqn40q4llseyrdz68x5enuyt7yhk3d3kqxwjfullcrqhtc82vzraw0pdgjxpjtxgjvrqeqfdn7j9ck57w2u5dppfuvkk52cc3mn28nnshn87j84vfd3tdkqu9wl037yn49l829gftaky623476hw4wc7x26al8q7mfsg56pmzlyzdmgqsa33r37k0thurnjasahp3c9z5mwk3zgtgtfvj2qydgz5su6wvewhh7yeqft8z2ze4j99qha32wagywmjuqhtff3v7wpdmrcu84zmlxd5zhf5lngp4t070uup93w7lv95uk6ckhrqq4fx8epcuynh6qwh86a03nvnjf7vxvmkae2l2qzu24pjz8wdtwqs87pfdhzcwj29ruzh9ag54zqe8qzw46azds62ug7qxgf3z00rgu5q28newruew6pcvv7w7uvs9fzchha5awsfk2xfjtyu3ml5y98m2fs7peusgwv9r78uy8w6stzgc9prtsa57l03l7sfhakkt40va06uwva5qc6vy8mztwkdw2z69xpzuf4qaz9rk83wtjqjj5xvxp4xjpeple9dxgxp0tqhqzt2f8t8r03dn0vx9tl6tnh7mn6k2tnatwqkjx0csz5fj7a3g4fs07rv2p2hxag0hc8p29hx4skh0xp6x2y6afwrs5jx8hagl8pm320wwwfeh2zsernkgul5jhpy2ea5tjf934z6qgwsxezex94w935z2txr8gw3fcsrpp4m94nmwmap3pe6xyw5qlz7yyjg9merzckv6lxe5k8rtysn7fgzy3f5ug99hzq29gpllklmja7sdjg2wwgxee6m5nqercjx48cta7qp4q6hyerdts4fc5ly0hemn9rnygwng4hckqc7le3u7jpemgjxjc4rudzdekqllkg88k9p3m0gadjm4s2ha5r42p0cv5ss44n7kfyzw4scpyjw0alt2rmuwckvezejusxsxdqu6c8ad0ja7fqh2e4";
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
            .route("/rpc/tx/sendtx", axum::routing::post(send_transaction))
            .route("/rpc/getnonces/{count}", axum::routing::get(get_nonces))
            .route(
                "/rpc/getlastblocks/{count}",
                axum::routing::get(get_last_blocks),
            )
            .route(
                "/rpc/owner_blocks/{start}/{end}",
                axum::routing::get(get_owner_blocks),
            )
            .route(
                "/rpc/generate_membership_proof",
                axum::routing::post(generate_restore_membership_proof),
            )
            .route(
                "/rpc/build_utxo_index",
                axum::routing::post(build_utxo_index),
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

    let Some(block) = archival_state
        .get_block(digest)
        .await
        .context("Failed to get block")?
    else {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseMsMembershipProofEx {
    pub height: BlockHeight,
    pub block_id: Digest,
    pub proofs: Vec<MsMembershipProofEx>,
}

async fn generate_restore_membership_proof(
    State(rpcstate): State<NeptuneRPCServer>,
    body: axum::body::Bytes,
) -> Result<Vec<u8>, RestError> {
    let r_datas: Vec<RequestMsMembershipProofEx> =
        bincode::deserialize_from(body.reader()).context("deserialize error")?;
    let state = rpcstate.state.lock_guard().await;

    let ams = state.chain.archival_state().archival_mutator_set.ams();

    let mut proofs = Vec::with_capacity(r_datas.len());
    for r_data in r_datas {
        if let Ok(p) = ams.restore_membership_proof_ex(r_data).await {
            proofs.push(p);
        }
    }

    let cur_block = state.chain.archival_state().get_tip().await;

    let height = cur_block.header().height;
    let block_id = cur_block.hash();

    let response = ResponseMsMembershipProofEx {
        height,
        block_id,
        proofs,
    };
    bincode::serialize(&response).map_err(|e| RestError(e.to_string()))
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

#[derive(Debug, Deserialize, Clone)]
struct SendTx {
    broadcast_tx: BroadcastTx,
    amount: String,
    sender_randomness: String,
    fee_address: String,
    block_height: u64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct ResponseSendTx {
    status: u64,
    message: String,
}
async fn send_transaction(
    State(mut rpcstate): State<NeptuneRPCServer>,
    body: axum::body::Bytes,
) -> Result<ErasedJson, RestError> {
    let send_tx: SendTx = bincode::deserialize_from(body.reader()).context("deserialize error")?;
    // 判断fee_address是否是合法的地址
    if send_tx.fee_address != FEE_ADDRESS.to_string() {
        return Ok(ErasedJson::pretty(ResponseSendTx {
            status: 1,
            message: "fee_address is not valid".to_string(),
        }));
    }

    let network = Network::Main;
    let receiving_address = ReceivingAddress::from_bech32m(&send_tx.fee_address, network)?;
    let amount = NativeCurrencyAmount::coins_from_str(&send_tx.amount)?;

    let sender_randomness: Digest = Digest::try_from_hex(&send_tx.sender_randomness)
        .context("failed to parse sender_randomness as hex digest")?;

    let output_index = calculate_utxo_commitment(receiving_address, amount, sender_randomness);

    let outputs: Vec<String> = send_tx
        .broadcast_tx
        .transaction
        .kernel
        .outputs
        .iter()
        .map(|output| output.canonical_commitment.to_hex())
        .collect();
    if !outputs.contains(&output_index) {
        return Ok(ErasedJson::pretty(ResponseSendTx {
            status: 2,
            message: "Failed to pay the priority fee".to_string(),
        }));
    }

    {
        let state = rpcstate.state.lock_guard().await;
        let end: u64 = state
            .chain
            .archival_state()
            .get_tip()
            .await
            .header()
            .height
            .into();
        if end != send_tx.block_height {
            return Ok(ErasedJson::pretty(ResponseSendTx {
                status: 3,
                message: format!("Transaction expired. Please sync to the latest block height first. current block height: {}", end),
            }))
        }
    }

    let insert = true;

    if insert {
        let tx = send_tx.broadcast_tx;
        let mut state = rpcstate.state.lock_guard_mut().await;
        state.mempool_insert(tx.transaction, tx.origin).await;
        let _ = rpcstate
            .rpc_server_to_main_tx
            .send(RPCServerToMain::BroadcastNotification(tx.notification))
            .await;
    } else {
        // todo: 将数据发送给proof机器，确保proof机器可以工作
        let busy = true;
        if busy {
            return Ok(ErasedJson::pretty(ResponseSendTx {
                status: 1,
                message: "proof machine is busy".to_string(),
            }));
        }
    }

    Ok(ErasedJson::pretty(ResponseSendTx {
        status: 0,
        message: "success".to_string(),
    }))
}

#[derive(Debug, Deserialize, Clone)]
struct UtxoIndexRequest {
    pub address: String,
    pub amount: String,
    pub sender_randomness: String,
}

async fn build_utxo_index(
    State(_rpcstate): State<NeptuneRPCServer>,
    Json(body): Json<UtxoIndexRequest>,
) -> Result<ErasedJson, RestError> {
    let network = Network::Main;
    let receiving_address = ReceivingAddress::from_bech32m(&body.address, network)?;
    let amount = NativeCurrencyAmount::coins_from_str(&body.amount)?;

    let sender_randomness: Digest = Digest::try_from_hex(&body.sender_randomness)
        .context("failed to parse sender_randomness as hex digest")?;

    let output_index = calculate_utxo_commitment(receiving_address, amount, sender_randomness);

    tracing::info!("output: {}", output_index);
    Ok(ErasedJson::pretty(output_index))
}

fn calculate_utxo_commitment(
    receiving_address: ReceivingAddress,
    amount: NativeCurrencyAmount,
    sender_randomness: Digest,
) -> String {
    let receiver_digest = receiving_address.privacy_digest();
    let notification_method = UtxoNotifyMethod::OnChain(receiving_address.clone());
    let utxo = Utxo::new_native_currency(receiving_address.lock_script(), amount);

    let output = TxOutput {
        utxo,
        sender_randomness,
        receiver_digest,
        notification_method,
        owned: false,
        is_change: false,
    };
    let output_record: AdditionRecord = (&output).into();
    output_record.canonical_commitment.to_hex()
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
