use crate::{
    conversions::ToVecHex,
    dataframes::SortableDataFrame,
    freeze2::{ChunkDim, CollectByBlock, CollectByTransaction, ColumnData, RpcParams},
    store, with_series, with_series_binary, CollectError, ColumnType, Logs, Source, Table,
};
use ethers::prelude::*;
use ethers_core::abi::Token;
use polars::prelude::*;
use std::collections::HashMap;

#[async_trait::async_trait]
impl CollectByBlock for Logs {
    type BlockResponse = Vec<Log>;

    type BlockColumns = LogColumns;

    fn block_parameters() -> Vec<ChunkDim> {
        vec![ChunkDim::BlockNumber]
    }

    async fn fetch_by_block(
        request: RpcParams,
        source: Source,
        _schema: Table,
    ) -> Result<Self::BlockResponse, CollectError> {
        source.fetcher.get_logs(&request.ethers_log_filter()).await
    }

    fn process_block_response(
        message: Self::BlockResponse,
        columns: &mut Self::BlockColumns,
        schema: &Table,
    ) {
        process_logs(message, columns, schema)
    }
}

#[async_trait::async_trait]
impl CollectByTransaction for Logs {
    type TransactionResponse = Vec<Log>;

    type TransactionColumns = LogColumns;

    fn transaction_parameters() -> Vec<ChunkDim> {
        vec![ChunkDim::BlockNumber]
    }

    async fn fetch_by_transaction(
        request: RpcParams,
        source: Source,
        _schema: Table,
    ) -> Result<Self::TransactionResponse, CollectError> {
        let logs = source
            .fetcher
            .get_transaction_receipt(request.ethers_transaction())
            .await?
            .ok_or(CollectError::CollectError("transaction receipt not found".to_string()))?
            .logs;
        Ok(logs)
    }

    fn process_transaction_response(
        message: Self::TransactionResponse,
        columns: &mut Self::TransactionColumns,
        schema: &Table,
    ) {
        process_logs(message, columns, schema)
    }
}

/// columns for transactions
#[cryo_to_df::to_df]
#[derive(Default)]
pub struct LogColumns {
    n_rows: u64,
    block_number: Vec<u32>,
    transaction_index: Vec<u32>,
    log_index: Vec<u32>,
    transaction_hash: Vec<Vec<u8>>,
    address: Vec<Vec<u8>>,
    topic0: Vec<Option<Vec<u8>>>,
    topic1: Vec<Option<Vec<u8>>>,
    topic2: Vec<Option<Vec<u8>>>,
    topic3: Vec<Option<Vec<u8>>>,
    data: Vec<Vec<u8>>,
    event_cols: HashMap<String, Vec<Token>>,
}

/// process block into columns
pub fn process_logs(logs: Vec<Log>, columns: &mut LogColumns, schema: &Table) {
    for log in logs.iter() {
        if let (Some(bn), Some(tx), Some(ti), Some(li)) =
            (log.block_number, log.transaction_hash, log.transaction_index, log.log_index)
        {
            columns.n_rows += 1;
            store!(schema, columns, block_number, bn.as_u32());
            store!(schema, columns, transaction_index, ti.as_u32());
            store!(schema, columns, log_index, li.as_u32());
            store!(schema, columns, transaction_hash, tx.as_bytes().to_vec());
            store!(schema, columns, address, log.address.as_bytes().to_vec());
            store!(schema, columns, data, log.data.to_vec());

            // topics
            for i in 0..4 {
                let topic = if i < log.topics.len() {
                    Some(log.topics[i].as_bytes().to_vec())
                } else {
                    None
                };
                match i {
                    0 => store!(schema, columns, topic0, topic),
                    1 => store!(schema, columns, topic1, topic),
                    2 => store!(schema, columns, topic2, topic),
                    3 => store!(schema, columns, topic3, topic),
                    _ => panic!("invalid number of topics"),
                }
            }
        }
    }

    // add decoded event logs
    let decoder = schema.log_decoder.clone();
    if let Some(decoder) = decoder {
        decoder.parse_log_from_event(logs).into_iter().for_each(|(k, v)| {
            columns.event_cols.entry(k).or_insert(Vec::new()).extend(v);
        });
    }
}