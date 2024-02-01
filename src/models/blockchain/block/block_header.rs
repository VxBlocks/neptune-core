use crate::prelude::twenty_first;

use crate::models::consensus::mast_hash::HasDiscriminant;
use crate::models::consensus::mast_hash::MastHash;
use crate::util_types::mutator_set::mutator_set_accumulator::MutatorSetAccumulator;
use crate::util_types::mutator_set::mutator_set_trait::MutatorSet;
use crate::Hash;
use get_size::GetSize;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use twenty_first::shared_math::bfield_codec::BFieldCodec;
use twenty_first::shared_math::digest::Digest;

use twenty_first::amount::u32s::U32s;
use twenty_first::shared_math::b_field_element::BFieldElement;

use super::block_height::BlockHeight;

pub const TARGET_DIFFICULTY_U32_SIZE: usize = 5;
pub const PROOF_OF_WORK_COUNT_U32_SIZE: usize = 5;
pub const TARGET_BLOCK_INTERVAL: u64 = 588000; // 9.8 minutes in milliseconds
pub const MINIMUM_DIFFICULTY: u32 = 2;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, BFieldCodec, GetSize)]
pub struct BlockHeader {
    pub version: BFieldElement,
    pub height: BlockHeight,
    pub mutator_set_hash: Digest,
    pub prev_block_digest: Digest,

    // TODO: Reject blocks that are more than 10 seconds into the future
    // number of milliseconds since unix epoch
    pub timestamp: BFieldElement,

    // TODO: Consider making a type for `nonce`
    pub nonce: [BFieldElement; 3],
    pub max_block_size: u32,

    // use to compare two forks of different height
    pub proof_of_work_line: U32s<PROOF_OF_WORK_COUNT_U32_SIZE>,

    // use to compare two forks of the same height
    pub proof_of_work_family: U32s<PROOF_OF_WORK_COUNT_U32_SIZE>,

    // This is the difficulty for the *next* block. Unit: expected # hashes
    pub difficulty: U32s<TARGET_DIFFICULTY_U32_SIZE>,
    pub block_body_merkle_root: Digest,
}

impl Display for BlockHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let string = format!(
            "Height: {}\n\
            Timestamp: {}\n\
            Prev. Digest: {}\n\
            Proof-of-work-line: {}\n\
            Proof-of-work-family: {}",
            self.height,
            self.timestamp,
            self.prev_block_digest,
            self.proof_of_work_line,
            self.proof_of_work_family
        );

        write!(f, "{}", string)
    }
}

pub enum BlockHeaderField {
    Version,
    Height,
    MutatorSetHash,
    PrevBlockDigest,
    Timestamp,
    Nonce,
    MaxBlockSize,
    ProofOfWorkLine,
    ProofOfWorkFamily,
    Difficulty,
    BlockBodyMerkleRoot,
}

impl HasDiscriminant for BlockHeaderField {
    fn discriminant(&self) -> usize {
        match self {
            BlockHeaderField::Version => 0,
            BlockHeaderField::Height => 1,
            BlockHeaderField::MutatorSetHash => 2,
            BlockHeaderField::PrevBlockDigest => 3,
            BlockHeaderField::Timestamp => 4,
            BlockHeaderField::Nonce => 5,
            BlockHeaderField::MaxBlockSize => 6,
            BlockHeaderField::ProofOfWorkLine => 7,
            BlockHeaderField::ProofOfWorkFamily => 8,
            BlockHeaderField::Difficulty => 9,
            BlockHeaderField::BlockBodyMerkleRoot => 10,
        }
    }
}

impl MastHash for BlockHeader {
    type FieldEnum = BlockHeaderField;

    fn mast_sequences(&self) -> Vec<Vec<BFieldElement>> {
        vec![
            self.version.encode(),
            self.height.encode(),
            self.mutator_set_hash.encode(),
            self.prev_block_digest.encode(),
            self.timestamp.encode(),
            self.nonce.encode(),
            self.max_block_size.encode(),
            self.proof_of_work_line.encode(),
            self.proof_of_work_family.encode(),
            self.difficulty.encode(),
            self.block_body_merkle_root.encode(),
        ]
    }
}

impl BlockHeader {
    pub fn empty_header() -> BlockHeader {
        Self {
            version: BFieldElement::new(0),
            height: 0.into(),
            mutator_set_hash: MutatorSetAccumulator::<Hash>::new().hash(),
            prev_block_digest: Digest::default(),
            timestamp: BFieldElement::new(0),
            nonce: [
                BFieldElement::new(0),
                BFieldElement::new(0),
                BFieldElement::new(0),
            ],
            max_block_size: 0,
            proof_of_work_line: 0.into(),
            proof_of_work_family: 0.into(),
            difficulty: 0.into(),
            block_body_merkle_root: Digest::default(),
        }
    }
}

#[cfg(test)]
mod block_header_tests {
    use rand::{thread_rng, Rng};

    use super::*;

    pub fn random_block_header() -> BlockHeader {
        let mut rng = thread_rng();
        BlockHeader {
            version: rng.gen(),
            height: BlockHeight::from(rng.gen::<u64>()),
            mutator_set_hash: rng.gen(),
            prev_block_digest: rng.gen(),
            timestamp: rng.gen(),
            nonce: rng.gen(),
            max_block_size: rng.gen(),
            proof_of_work_line: rng.gen(),
            proof_of_work_family: rng.gen(),
            difficulty: rng.gen(),
            block_body_merkle_root: rng.gen(),
        }
    }
    #[test]
    pub fn test_block_header_decode() {
        let block_header = random_block_header();
        let encoded = block_header.encode();
        let decoded = *BlockHeader::decode(&encoded).unwrap();
        assert_eq!(block_header, decoded);
    }
}
