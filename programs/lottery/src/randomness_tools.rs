// use anchor_lang::solana_program::keccak;
// use std::convert::TryInto;

// //https://docs.chain.link/docs/chainlink-vrf-best-practices/#getting-multiple-random-number
// pub fn expand(randomness: Vec<u8>, n: u32) -> u32 {
//     let mut hasher = keccak::Hasher::default();
//     hasher.hash(&randomness);
//     hasher.hash(&n.to_le_bytes());

//     u32::from_le_bytes(
//         hasher.result().to_bytes()[0..4]
//             .try_into()
//             .expect("slice with incorrect length"),
//     )
// }

use sha2::{Sha256, Digest};

pub struct HashStruct {
    pub nonce : u64,
    pub initial_seed : u64
}

unsafe fn any_as_u8_slice<T: Sized>(p: &T) -> &[u8] {
    ::std::slice::from_raw_parts(
        (p as *const T) as *const u8,
        ::std::mem::size_of::<T>(),
    )
}

pub fn get_sha256_hashed_random(seed: u64, nonce: u64) -> u64 {

    let hashstruct = HashStruct {nonce : nonce, initial_seed : seed};
    let vec_to_hash = unsafe{any_as_u8_slice(&hashstruct)};
    let hash= &(Sha256::new()
    .chain_update(vec_to_hash)
    .finalize()[..32]);

    // hash is a vector of 32 8bit numbers.  We can take slices of this to generate our 4 random u64s
    let mut hashed_randoms : [u64; 4] = [0; 4];
    for i in 0..1 {
        let hash_slice = &hash[i*8..(i+1)*8];
        hashed_randoms[i] = u64::from_le_bytes(hash_slice.try_into().expect("slice with incorrect length"));
    }

    return hashed_randoms[0];
    
}