use crate::{
    conversions::ToVecHex, dataframes::SortableDataFrame, store, with_series, with_series_binary,
    CodeDiffs, CollectByBlock, CollectByTransaction, CollectError, ColumnData, ColumnType,
    Datatype, RpcParams, Source, Table,
};
use ethers::prelude::*;
use polars::prelude::*;
use std::collections::HashMap;

#[async_trait::async_trait]
impl CollectByBlock for CodeDiffs {
    type BlockResponse = (Option<u32>, Option<Vec<u8>>, Vec<ethers::types::BlockTrace>);

    type BlockColumns = CodeDiffColumns;

    async fn extract_by_block(
        request: RpcParams,
        source: Source,
        _schemas: HashMap<Datatype, Table>,
    ) -> Result<Self::BlockResponse, CollectError> {
        source.fetcher.trace_block_state_diffs(request.block_number() as u32).await
    }

    fn transform_by_block(
        response: Self::BlockResponse,
        columns: &mut Self::BlockColumns,
        schemas: &HashMap<Datatype, Table>,
    ) {
        if let Some(schema) = schemas.get(&Datatype::CodeDiffs) {
            process_code_diffs(&response, columns, schema)
        } else {
            panic!("missing schema")
        }
    }
}

#[async_trait::async_trait]
impl CollectByTransaction for CodeDiffs {
    type TransactionResponse = (Option<u32>, Option<Vec<u8>>, Vec<ethers::types::BlockTrace>);

    type TransactionColumns = CodeDiffColumns;

    async fn extract_by_transaction(
        request: RpcParams,
        source: Source,
        _schemas: HashMap<Datatype, Table>,
    ) -> Result<Self::TransactionResponse, CollectError> {
        source.fetcher.trace_transaction_state_diffs(request.transaction_hash()).await
    }

    fn transform_by_transaction(
        response: Self::TransactionResponse,
        columns: &mut Self::TransactionColumns,
        schemas: &HashMap<Datatype, Table>,
    ) {
        if let Some(schema) = schemas.get(&Datatype::CodeDiffs) {
            process_code_diffs(&response, columns, schema)
        } else {
            panic!("missing schema")
        }
    }
}

/// columns for transactions
#[cryo_to_df::to_df(Datatype::CodeDiffs)]
#[derive(Default)]
pub struct CodeDiffColumns {
    n_rows: u64,
    block_number: Vec<Option<u32>>,
    transaction_index: Vec<Option<u64>>,
    transaction_hash: Vec<Option<Vec<u8>>>,
    address: Vec<Vec<u8>>,
    from_value: Vec<Vec<u8>>,
    to_value: Vec<Vec<u8>>,
}

pub(crate) fn process_code_diffs(
    response: &(Option<u32>, Option<Vec<u8>>, Vec<ethers::types::BlockTrace>),
    columns: &mut CodeDiffColumns,
    schema: &Table,
) {
    let (block_number, tx, traces) = response;
    for (index, trace) in traces.iter().enumerate() {
        if let Some(ethers::types::StateDiff(state_diffs)) = &trace.state_diff {
            for (addr, diff) in state_diffs.iter() {
                process_code_diff(addr, &diff.code, block_number, tx, index, columns, schema);
            }
        }
    }
}

pub(crate) fn process_code_diff(
    addr: &H160,
    diff: &Diff<Bytes>,
    block_number: &Option<u32>,
    transaction_hash: &Option<Vec<u8>>,
    transaction_index: usize,
    columns: &mut CodeDiffColumns,
    schema: &Table,
) {
    columns.n_rows += 1;
    let (from, to) = match diff {
        Diff::Same => (H256::zero().as_bytes().to_vec(), H256::zero().as_bytes().to_vec()),
        Diff::Born(value) => (H256::zero().as_bytes().to_vec(), value.to_vec()),
        Diff::Died(value) => (value.to_vec(), H256::zero().as_bytes().to_vec()),
        Diff::Changed(ChangedType { from, to }) => (from.to_vec(), to.to_vec()),
    };
    store!(schema, columns, block_number, *block_number);
    store!(schema, columns, transaction_index, Some(transaction_index as u64));
    store!(schema, columns, transaction_hash, transaction_hash.clone());
    store!(schema, columns, address, addr.as_bytes().to_vec());
    store!(schema, columns, from_value, from);
    store!(schema, columns, to_value, to);
}