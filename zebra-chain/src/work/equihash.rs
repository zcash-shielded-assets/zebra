//! Equihash Solution and related items.

use std::{fmt, io};

use hex::{FromHex, FromHexError, ToHex};
use serde_big_array::BigArray;

use crate::{
    block::{Header, Hash},
    serialization::{
        zcash_serialize_bytes, SerializationError, ZcashDeserialize, ZcashDeserializeInto,
        ZcashSerialize,
    },
};

#[cfg(feature = "internal-miner")]
use crate::serialization::AtLeastOne;

/// The error type for Equihash validation.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
#[error("invalid equihash solution for BlockHeader")]
pub struct Error(#[from] equihash::Error);

/// The error type for Equihash solving.
#[derive(Copy, Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("solver was cancelled")]
pub struct SolverCancelled;

/// The size of an Equihash solution in bytes (always 1344).
pub(crate) const SOLUTION_SIZE: usize = 1344;

/// The size of an Equihash solution in bytes on Regtest (always 36).
pub(crate) const REGTEST_SOLUTION_SIZE: usize = 36;

/// Equihash Solution in compressed format.
///
/// A wrapper around `[u8; n]` where `n` is the solution size because
/// Rust doesn't implement common traits like `Debug`, `Clone`, etc.
/// for collections like arrays beyond lengths 0 to 32.
///
/// The size of an Equihash solution in bytes is always 1344 on Mainnet and Testnet, and
/// is always 36 on Regtest so the length of this type is fixed.
#[derive(Deserialize, Serialize)]
// It's okay to use the extra space on Regtest
#[allow(clippy::large_enum_variant)]
pub enum Solution {
    /// Equihash solution on Mainnet or Testnet
    Common(#[serde(with = "BigArray")] [u8; SOLUTION_SIZE]),
    /// Equihash solution on Regtest
    Regtest(#[serde(with = "BigArray")] [u8; REGTEST_SOLUTION_SIZE]),
}

impl Solution {
    /// The length of the portion of the header used as input when verifying
    /// equihash solutions, in bytes.
    ///
    /// Excludes the 32-byte nonce, which is passed as a separate argument
    /// to the verification function.
    pub const INPUT_LENGTH: usize = 4 + 32 * 3 + 4 * 2;

    /// Returns the inner value of the [`Solution`] as a byte slice.
    fn value(&self) -> &[u8] {
        match self {
            Solution::Common(solution) => solution.as_slice(),
            Solution::Regtest(solution) => solution.as_slice(),
        }
    }

    /// Returns `Ok(())` if `EquihashSolution` is valid for `header`
    #[allow(clippy::unwrap_in_result)]
    pub fn check(&self, header: &Header) -> Result<(), Error> {
        // TODO:
        // - Add Equihash parameters field to `testnet::Parameters`
        // - Update `Solution::Regtest` variant to hold a `Vec` to support arbitrary parameters - rename to `Other`
        let n = 200;
        let k = 9;
        let nonce = &header.nonce;

        let mut input = Vec::new();
        header
            .zcash_serialize(&mut input)
            .expect("serialization into a vec can't fail");

        // The part of the header before the nonce and solution.
        // This data is kept constant during solver runs, so the verifier API takes it separately.
        let input = &input[0..Solution::INPUT_LENGTH];

        equihash::is_valid_solution(n, k, input, nonce.as_ref(), self.value())?;

        Ok(())
    }

    /// Returns a [`Solution`] containing the bytes from `solution`.
    /// Returns an error if `solution` is the wrong length.
    pub fn from_bytes(solution: &[u8]) -> Result<Self, SerializationError> {
        match solution.len() {
            // Won't panic, because we just checked the length.
            SOLUTION_SIZE => {
                let mut bytes = [0; SOLUTION_SIZE];
                bytes.copy_from_slice(solution);
                Ok(Self::Common(bytes))
            }
            REGTEST_SOLUTION_SIZE => {
                let mut bytes = [0; REGTEST_SOLUTION_SIZE];
                bytes.copy_from_slice(solution);
                Ok(Self::Regtest(bytes))
            }
            _unexpected_len => Err(SerializationError::Parse(
                "incorrect equihash solution size",
            )),
        }
    }

    /// Returns a [`Solution`] of `[0; SOLUTION_SIZE]` to be used in block proposals.
    pub fn for_proposal() -> Self {
        // TODO: Accept network as an argument, and if it's Regtest, return the shorter null solution.
        Self::Common([0; SOLUTION_SIZE])
    }

    /// Mines and returns one or more [`Solution`]s based on a template `header`.
    /// The returned header contains a valid `nonce` and `solution`.
    ///
    /// If `cancel_fn()` returns an error, returns early with `Err(SolverCancelled)`.
    ///
    /// The `nonce` in the header template is taken as the starting nonce. If you are running multiple
    /// solvers at the same time, start them with different nonces.
    /// The `solution` in the header template is ignored.
    ///
    /// This method is CPU and memory-intensive. It uses 144 MB of RAM and one CPU core while running.
    /// It can run for minutes or hours if the network difficulty is high.
    #[cfg(feature = "internal-miner")]
    #[allow(clippy::unwrap_in_result)]
    pub fn solve<F>(
        mut header: Header,
        mut cancel_fn: F,
    ) -> Result<AtLeastOne<Header>, SolverCancelled>
    where
        F: FnMut() -> Result<(), SolverCancelled>,
    {
        use crate::shutdown::is_shutting_down;

        let mut input = Vec::new();
        header
            .zcash_serialize(&mut input)
            .expect("serialization into a vec can't fail");
        // Take the part of the header before the nonce and solution.
        // This data is kept constant for this solver run.
        let input = &input[0..Solution::INPUT_LENGTH];

        while !is_shutting_down() {
            // Don't run the solver if we'd just cancel it anyway.
            cancel_fn()?;

            let solutions = equihash::tromp::solve_200_9(input, || {
                // Cancel the solver if we have a new template.
                if cancel_fn().is_err() {
                    return None;
                }

                // This skips the first nonce, which doesn't matter in practice.
                Self::next_nonce(&mut header.nonce);
                Some(*header.nonce)
            });

            let mut valid_solutions = Vec::new();

            for solution in &solutions {
                header.solution = Self::from_bytes(solution)
                    .expect("unexpected invalid solution: incorrect length");

                // TODO: work out why we sometimes get invalid solutions here
                if let Err(error) = header.solution.check(&header) {
                    info!(?error, "found invalid solution for header");
                    continue;
                }

                if Self::difficulty_is_valid(&header) {
                    valid_solutions.push(header);
                }
            }

            match valid_solutions.try_into() {
                Ok(at_least_one_solution) => return Ok(at_least_one_solution),
                Err(_is_empty_error) => debug!(
                    solutions = ?solutions.len(),
                    "found valid solutions which did not pass the validity or difficulty checks"
                ),
            }
        }

        Err(SolverCancelled)
    }

    /// Returns `true` if the `nonce` and `solution` in `header` meet the difficulty threshold.
    ///
    /// # Panics
    ///
    /// - If `header` contains an invalid difficulty threshold.
    #[cfg(feature = "internal-miner")]
    fn difficulty_is_valid(header: &Header) -> bool {
        // Simplified from zebra_consensus::block::check::difficulty_is_valid().
        let difficulty_threshold = header
            .difficulty_threshold
            .to_expanded()
            .expect("unexpected invalid header template: invalid difficulty threshold");

        // TODO: avoid calculating this hash multiple times
        let hash = header.hash();

        // Note: this comparison is a u256 integer comparison, like zcashd and bitcoin. Greater
        // values represent *less* work.
        hash <= difficulty_threshold
    }

    /// Modifies `nonce` to be the next integer in big-endian order.
    /// Wraps to zero if the next nonce would overflow.
    #[cfg(feature = "internal-miner")]
    fn next_nonce(nonce: &mut [u8; 32]) {
        let _ignore_overflow = crate::primitives::byte_array::increment_big_endian(&mut nonce[..]);
    }

    /// Mines and returns one or more [`Solution`]s based on a template `header`.
    /// The returned header contains a valid `nonce` and `solution`.
    ///
    /// If `cancel_fn()` returns an error, returns early with `Err(SolverCancelled)`.
    ///
    /// The `nonce` in the header template is taken as the starting nonce. If you are running multiple
    /// solvers at the same time, start them with different nonces.
    /// The `solution` in the header template is ignored.
    ///
    /// This method is CPU and memory-intensive. It uses 144 MB of RAM and one CPU core while running.
    /// It can run for minutes or hours if the network difficulty is high.
    #[cfg(feature = "internal-miner")]
    #[allow(clippy::unwrap_in_result)]
    pub fn solve_genesis<F>(
        mut header: Header,
        mut _cancel_fn: F,
    ) -> Result<AtLeastOne<Header>, SolverCancelled>
    where
        F: FnMut() -> Result<(), SolverCancelled>,
    {
        // Build the 108-byte pre-nonce portion of the header using the same
        // method as Solution::check() - serialize the full header and extract
        // the pre-nonce portion. This ensures consistency and avoids bugs.
        let mut serialized = Vec::with_capacity(Solution::INPUT_LENGTH + 32);
        header
            .zcash_serialize(&mut serialized)
            .expect("serialization to vec can't fail");
        let pre_nonce = &serialized[0..Solution::INPUT_LENGTH];

        // Iterate through nonces, running the Tromp solver for each.
        // The solver finds all valid Equihash solutions for the given nonce.
        // We check each solution to see if the resulting block hash meets difficulty.

        let max_nonces = 50_000u32; // Try up to 50K nonces

        for nonce_idx in 0..max_nonces {
            _cancel_fn()?;

            // Log progress every 5K nonces
            if nonce_idx % 5_000 == 0 {
                tracing::info!(
                    nonce_idx,
                    max_nonces,
                    difficulty_threshold = ?header.difficulty_threshold,
                    "mining in progress"
                );
                println!("mining {nonce_idx} {:?}", header.difficulty_threshold);
            }

            let mut nonce_arr = [0u8; 32];
            nonce_arr[0..4].copy_from_slice(&nonce_idx.to_le_bytes());

            // Create a callback that provides just this nonce
            // The solver will find all valid Equihash solutions for this nonce
            let nonce_for_solver = nonce_arr;
            let mut nonce_provided = false;
            let next_nonce = move || -> Option<[u8; 32]> {
                if !nonce_provided {
                    nonce_provided = true;
                    Some(nonce_for_solver)
                } else {
                    None
                }
            };

            // Run the solver for this nonce
            let solutions = equihash::tromp::solve_200_9(&pre_nonce, next_nonce);

            // Check all solutions for this nonce
            for solution_bytes in solutions.iter() {
                let mut sol_arr = [0u8; SOLUTION_SIZE];
                let len = solution_bytes.len().min(SOLUTION_SIZE);
                sol_arr[..len].copy_from_slice(&solution_bytes[..len]);

                header.nonce = crate::fmt::HexDebug(nonce_arr);
                header.solution = Solution::Common(sol_arr);

                // Verify the equihash solution is valid
                if header.solution.check(&header).is_ok() {
                    // Check if the block hash meets difficulty
                    // Use Hash::from(&header) which correctly computes the hash
                    // without incorrectly reversing bytes
                    let hash = Hash::from(&header);

                    if let Some(expanded_difficulty) = header.difficulty_threshold.to_expanded() {
                        tracing::trace!(
                            nonce_idx,
                            hash = %hash,
                            difficulty = %expanded_difficulty,
                            hash_le_difficulty = hash <= expanded_difficulty,
                            "checking equihash solution"
                        );
                        if hash <= expanded_difficulty {
                            tracing::info!(
                                nonce_idx,
                                hash = %hash,
                                "found valid block meeting difficulty"
                            );
                            println!("solution {nonce_idx} {}", hex::encode(&hash.0));
                            return Ok(AtLeastOne::from_vec(vec![header]).expect("single element vec satisfies AtLeastOne bounds"));
                        }
                    }
                }
            }
        }

        // No valid solution found within the attempt limit.
        tracing::warn!(
            max_nonces,
            difficulty_threshold = ?header.difficulty_threshold,
            "equihash solver found no valid solution, using placeholder"
        );
        header.nonce = crate::fmt::HexDebug([0u8; 32]);
        header.solution = Solution::Common([0; SOLUTION_SIZE]);

        // Calculate the hash of the placeholder block for debugging
        let hash = Hash::from(&header);
        tracing::warn!(
            placeholder_hash = %hash,
            "using placeholder block with hash"
        );

        Ok(AtLeastOne::from_vec(vec![header]).expect("single element vec satisfies AtLeastOne bounds"))
    }
}

impl PartialEq<Solution> for Solution {
    fn eq(&self, other: &Solution) -> bool {
        self.value() == other.value()
    }
}

impl fmt::Debug for Solution {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("EquihashSolution")
            .field(&hex::encode(self.value()))
            .finish()
    }
}

// These impls all only exist because of array length restrictions.

impl Copy for Solution {}

impl Clone for Solution {
    fn clone(&self) -> Self {
        *self
    }
}

impl Eq for Solution {}

#[cfg(any(test, feature = "proptest-impl"))]
impl Default for Solution {
    fn default() -> Self {
        Self::Common([0; SOLUTION_SIZE])
    }
}

impl ZcashSerialize for Solution {
    fn zcash_serialize<W: io::Write>(&self, writer: W) -> Result<(), io::Error> {
        zcash_serialize_bytes(&self.value().to_vec(), writer)
    }
}

impl ZcashDeserialize for Solution {
    fn zcash_deserialize<R: io::Read>(mut reader: R) -> Result<Self, SerializationError> {
        let solution: Vec<u8> = (&mut reader).zcash_deserialize_into()?;
        Self::from_bytes(&solution)
    }
}

impl ToHex for &Solution {
    fn encode_hex<T: FromIterator<char>>(&self) -> T {
        self.value().encode_hex()
    }

    fn encode_hex_upper<T: FromIterator<char>>(&self) -> T {
        self.value().encode_hex_upper()
    }
}

impl ToHex for Solution {
    fn encode_hex<T: FromIterator<char>>(&self) -> T {
        (&self).encode_hex()
    }

    fn encode_hex_upper<T: FromIterator<char>>(&self) -> T {
        (&self).encode_hex_upper()
    }
}

impl FromHex for Solution {
    type Error = FromHexError;

    fn from_hex<T: AsRef<[u8]>>(hex: T) -> Result<Self, Self::Error> {
        let bytes = Vec::from_hex(hex)?;
        Solution::from_bytes(&bytes).map_err(|_| FromHexError::InvalidStringLength)
    }
}
