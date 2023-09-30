use crate::{
    conversions::ToVecHex, dataframes::SortableDataFrame, store, with_series, with_series_binary,
    CollectByBlock, CollectByTransaction, CollectError, ColumnData, ColumnType, Datatype,
    NativeTransfers, Params, Schemas, Source, Table, ToVecU8,
};
use ethers::prelude::*;
use polars::prelude::*;
use std::collections::HashMap;

type Result<T> = ::core::result::Result<T, CollectError>;

#[async_trait::async_trait]
impl CollectByBlock for NativeTransfers {
    type Response = Vec<Trace>;

    type Columns = NativeTransferColumns;

    async fn extract(request: Params, source: Source, _schemas: Schemas) -> Result<Self::Response> {
        source.fetcher.trace_block(request.block_number().into()).await
    }

    fn transform(response: Self::Response, columns: &mut Self::Columns, schemas: &Schemas) {
        let schema = schemas.get(&Datatype::Traces).expect("schema not provided");
        process_native_transfers(response, columns, schema)
    }
}

#[async_trait::async_trait]
impl CollectByTransaction for NativeTransfers {
    type Response = Vec<Trace>;

    type Columns = NativeTransferColumns;

    async fn extract(request: Params, source: Source, _schemas: Schemas) -> Result<Self::Response> {
        source.fetcher.trace_transaction(request.ethers_transaction_hash()).await
    }

    fn transform(response: Self::Response, columns: &mut Self::Columns, schemas: &Schemas) {
        let schema = schemas.get(&Datatype::Traces).expect("schema not provided");
        process_native_transfers(response, columns, schema)
    }
}

/// columns for transactions
#[cryo_to_df::to_df(Datatype::Traces)]
#[derive(Default)]
pub struct NativeTransferColumns {
    n_rows: u64,
    block_number: Vec<u32>,
    transaction_index: Vec<Option<u32>>,
    transfer_index: Vec<u32>,
    transaction_hash: Vec<Option<Vec<u8>>>,
    from_address: Vec<Vec<u8>>,
    to_address: Vec<Vec<u8>>,
    value: Vec<Vec<u8>>,
    chain_id: Vec<u64>,
}

/// process block into columns
pub fn process_native_transfers(
    traces: Vec<Trace>,
    columns: &mut NativeTransferColumns,
    schema: &Table,
) {
    for (transfer_index, trace) in traces.iter().enumerate() {
        columns.n_rows += 1;
        store!(schema, columns, block_number, trace.block_number as u32);
        store!(schema, columns, transaction_index, trace.transaction_position.map(|x| x as u32));
        store!(schema, columns, transfer_index, transfer_index as u32);
        store!(
            schema,
            columns,
            transaction_hash,
            trace.transaction_hash.map(|x| x.as_bytes().to_vec())
        );

        match &trace.action {
            Action::Call(action) => {
                store!(schema, columns, from_address, action.from.as_bytes().to_vec());
                store!(schema, columns, to_address, action.to.as_bytes().to_vec());
                store!(schema, columns, value, action.value.to_vec_u8());
            }
            Action::Create(action) => {
                store!(schema, columns, from_address, action.from.as_bytes().to_vec());
                match &trace.result.as_ref().expect("missing trace result") {
                    Res::Create(res) => store!(schema, columns, to_address, res.address.0.into()),
                    _ => panic!("missing create result"),
                }
                store!(schema, columns, value, action.value.to_vec_u8());
            }
            Action::Suicide(action) => {
                store!(schema, columns, from_address, action.address.as_bytes().to_vec());
                store!(schema, columns, to_address, action.refund_address.as_bytes().to_vec());
                store!(schema, columns, value, action.balance.to_vec_u8());
            }
            Action::Reward(action) => {
                store!(schema, columns, from_address, vec![0; 20]);
                store!(schema, columns, to_address, action.author.as_bytes().to_vec());
                store!(schema, columns, value, action.value.to_vec_u8());
            }
        }
    }
}
