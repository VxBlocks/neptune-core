//! BlockInfo is a concise summary of a block intended for human
//! consumption/reporting in block explorers, cli, dashboard, etc.

use itertools::Itertools;
use serde::Deserialize;
use serde::Serialize;
use tasm_lib::prelude::Tip5;
use twenty_first::math::digest::Digest;

use super::difficulty_control::Difficulty;
use super::difficulty_control::ProofOfWork;
use crate::models::blockchain::block::block_height::BlockHeight;
use crate::models::blockchain::block::Block;
use crate::models::blockchain::type_scripts::native_currency_amount::NativeCurrencyAmount;
use crate::models::proof_abstractions::timestamp::Timestamp;
use crate::models::state::transaction_kernel_id::TransactionKernelId;
use crate::prelude::twenty_first;

/// Provides summary information about a Block
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlockInfo {
    pub height: BlockHeight,

    /// Block size in number of [`BFieldElement`](twenty_first::math::b_field_element::BFieldElement)s
    pub size: usize,
    pub digest: Digest,
    pub nonce: Digest,
    pub prev_block_digest: Digest,
    pub timestamp: Timestamp,
    pub cumulative_proof_of_work: ProofOfWork,
    pub difficulty: Difficulty,
    pub num_inputs: usize,
    pub inputs: Vec<String>,
    pub num_outputs: usize,
    pub outputs: Vec<String>,
    pub num_public_announcements: usize,
    pub coinbase_amount: NativeCurrencyAmount,
    pub fee: NativeCurrencyAmount,
    pub is_genesis: bool,
    pub is_tip: bool,
    pub is_canonical: bool,
    pub sibling_blocks: Vec<Digest>, // blocks at same height
    // pub block_kernel2: BlockKernel,
    pub txid: TransactionKernelId,
}

// note: this is used by neptune-cli block-info command.
impl std::fmt::Display for BlockInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let buf = String::new()
            + &format!("height: {}\n", self.height)
            + &format!("digest: {}\n", self.digest.to_hex())
            + &format!("size: {}\n", self.size)
            + &format!("nonce: {}\n", self.nonce.to_hex())
            + &format!("prev_block_digest: {}\n", self.prev_block_digest.to_hex())
            + &format!("timestamp: {}\n", self.timestamp.standard_format())
            + &format!(
                "cumulative_proof_of_work: {}\n",
                self.cumulative_proof_of_work
            )
            + &format!("difficulty: {}\n", self.difficulty)
            + &format!("num_inputs: {}\n", self.num_inputs)
            + &format!("inputs: {:#?}\n", self.inputs)
            + &format!("num_outputs: {}\n", self.num_outputs)
            + &format!(
                "num_public_announcements: {}\n",
                self.num_public_announcements
            )
            + &format!("outputs: {:#?}\n", self.outputs)
            + &format!("coinbase_amount: {}\n", self.coinbase_amount)
            + &format!("fee: {}\n", self.fee)
            + &format!("is_genesis: {}\n", self.is_genesis)
            + &format!("is_tip: {}\n", self.is_tip)
            + &format!("is_canonical: {}\n", self.is_canonical)
            + &format!(
                "sibling_blocks: {}\n",
                self.sibling_blocks.iter().map(|d| d.to_hex()).join(",")
            )
            // + &format!(
            //     "block_kernel2: {:?}\n",
            //     self.block_kernel2
            // )
            + &format!(
                "txid: {:?}\n",
                self.txid,
            );

        write!(f, "{}", buf)
    }
}

impl BlockInfo {
    pub fn new(
        block: &Block,
        genesis_digest: Digest,
        tip_digest: Digest,
        sibling_blocks: Vec<Digest>, // other blocks at same height
        is_canonical: bool,
    ) -> Self {
        let body = block.body();
        let header = block.header();
        let digest = block.hash();
        Self {
            digest,
            nonce: header.nonce,
            prev_block_digest: header.prev_block_digest,
            height: header.height,
            size: block.size(),
            timestamp: header.timestamp,
            difficulty: header.difficulty,
            cumulative_proof_of_work: header.cumulative_proof_of_work,
            num_inputs: body.transaction_kernel.inputs.len(),
            inputs: body
                .transaction_kernel
                .inputs
                .iter()
                .map(|input| Tip5::hash(&input.absolute_indices).to_hex())
                .collect(),
            num_outputs: body.transaction_kernel.outputs.len(),
            outputs: body
                .transaction_kernel
                .outputs
                .iter()
                .map(|output| output.canonical_commitment.to_hex())
                .collect(),
            num_public_announcements: body.transaction_kernel.public_announcements.len(),
            fee: body.transaction_kernel.fee,
            coinbase_amount: block.coinbase_amount(),
            is_genesis: digest == genesis_digest,
            is_tip: digest == tip_digest,
            is_canonical,
            sibling_blocks,
            // block_kernel2: block.kernel.clone(),
            txid: block.kernel.body.transaction_kernel.txid(),
        }
    }

    /// Returns expected (calculated) coinbase amount for this block's height.
    ///
    /// note that this calculated value may be more than the coinbase_amount
    /// field because a miner may choose to reward themself less than the
    /// calculated reward amount.
    pub fn expected_coinbase_amount(&self) -> NativeCurrencyAmount {
        Block::block_subsidy(self.height)
    }
}
