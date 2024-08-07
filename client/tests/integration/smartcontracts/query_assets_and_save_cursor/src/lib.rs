//! Smart contract which executes [`FindAllAssets`] and saves cursor to the owner's metadata.

#![no_std]

#[cfg(not(test))]
extern crate panic_halt;

extern crate alloc;

use dlmalloc::GlobalDlmalloc;
use iroha_smart_contract::{data_model::query::cursor::ForwardCursor, prelude::*};
use nonzero_ext::nonzero;
use parity_scale_codec::{Decode, DecodeAll, Encode};

#[global_allocator]
static ALLOC: GlobalDlmalloc = GlobalDlmalloc;

getrandom::register_custom_getrandom!(iroha_smart_contract::stub_getrandom);

#[derive(Debug, Decode)]
struct QueryOutputCursor {
    _batch: alloc::vec::Vec<Asset>,
    cursor: ForwardCursor,
}

/// Execute [`FindAllAssets`] and save cursor to the owner's metadata.
/// NOTE: DON'T TAKE THIS AS AN EXAMPLE, THIS IS ONLY FOR TESTING INTERNALS OF IROHA
#[iroha_smart_contract::main]
fn main(owner: AccountId) {
    // NOTE: QueryOutputCursor fields are private therefore
    // we guess the layout by encoding and then decoding
    let asset_cursor = QueryOutputCursor::decode_all(
        &mut &FindAllAssets
            .fetch_size(FetchSize::new(Some(nonzero!(1_u32))))
            .execute()
            .dbg_unwrap()
            .encode()[..],
    )
    .dbg_unwrap();

    SetKeyValue::account(
        owner,
        "cursor".parse().unwrap(),
        JsonString::new(asset_cursor.cursor),
    )
    .execute()
    .dbg_expect("Failed to save cursor to the owner's metadata");
}
