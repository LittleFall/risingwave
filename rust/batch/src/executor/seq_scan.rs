use std::collections::VecDeque;

use prost::Message;
use risingwave_common::array::{DataChunk, DataChunkRef};
use risingwave_common::catalog::{Field, Schema, TableId};
use risingwave_common::error::ErrorCode::ProstError;
use risingwave_common::error::{Result, RwError};
use risingwave_pb::plan::plan_node::PlanNodeType;
use risingwave_pb::plan::SeqScanNode;
use risingwave_storage::bummock::BummockResult;
use risingwave_storage::table::ScannableTableRef;

use super::{BoxedExecutor, BoxedExecutorBuilder};
use crate::executor::{Executor, ExecutorBuilder};

/// Sequential scan executor on column-oriented tables
pub(super) struct SeqScanExecutor {
    table: ScannableTableRef,
    column_ids: Vec<i32>,
    schema: Schema,
    snapshot: VecDeque<DataChunkRef>,
}

impl BoxedExecutorBuilder for SeqScanExecutor {
    fn new_boxed_executor(source: &ExecutorBuilder) -> Result<BoxedExecutor> {
        ensure!(source.plan_node().get_node_type() == PlanNodeType::SeqScan);

        let seq_scan_node = SeqScanNode::decode(&(source.plan_node()).get_body().value[..])
            .map_err(|e| RwError::from(ProstError(e)))?;

        let table_id = TableId::from(&seq_scan_node.table_ref_id);

        let table_ref = source
            .global_task_env()
            .table_manager()
            .get_table(&table_id)?;

        let column_ids = seq_scan_node.get_column_ids();

        let schema = Schema::new(
            seq_scan_node
                .get_column_type()
                .iter()
                .map(Field::try_from)
                .collect::<Result<Vec<Field>>>()?,
        );

        Ok(Box::new(Self {
            table: table_ref,
            column_ids: column_ids.to_vec(),
            schema,
            snapshot: VecDeque::new(),
        }))
    }
}

#[async_trait::async_trait]
impl Executor for SeqScanExecutor {
    async fn open(&mut self) -> Result<()> {
        info!("SeqScanExecutor opened");

        let res = self.table.get_data_by_columns(&self.column_ids).await?;

        if let BummockResult::Data(data) = res {
            self.snapshot = VecDeque::from(data);
        }
        Ok(())
    }

    async fn next(&mut self) -> Result<Option<DataChunk>> {
        Ok(self.snapshot.pop_front().map(|c| c.as_ref().clone()))
    }

    async fn close(&mut self) -> Result<()> {
        info!("SeqScanExecutor closed.");
        Ok(())
    }

    fn schema(&self) -> &Schema {
        &self.schema
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use risingwave_common::array::{Array, I64Array};
    use risingwave_common::catalog::Field;
    use risingwave_common::column_nonnull;
    use risingwave_common::types::{DataTypeKind, DecimalType, Int64Type};
    use risingwave_storage::bummock::BummockTable;
    use risingwave_storage::table::ScannableTable;
    use risingwave_storage::{Table, TableColumnDesc};

    use super::*;
    use crate::*;

    #[tokio::test]
    async fn test_seq_scan_executor() -> Result<()> {
        let table_id = TableId::default();
        let schema = Schema {
            fields: vec![Field {
                data_type: Arc::new(DecimalType::new(false, 10, 5)?),
            }],
        };
        let table_columns = schema
            .fields
            .iter()
            .enumerate()
            .map(|(i, f)| TableColumnDesc {
                data_type: f.data_type.clone(),
                column_id: i as i32, // use column index as column id
            })
            .collect();
        let table = BummockTable::new(&table_id, table_columns);

        let fields = table.schema().fields;
        let col1 = column_nonnull! { I64Array, Int64Type, [1, 3, 5, 7, 9] };
        let col2 = column_nonnull! { I64Array, Int64Type, [2, 4, 6, 8, 10] };
        let data_chunk1 = DataChunk::builder().columns(vec![col1]).build();
        let data_chunk2 = DataChunk::builder().columns(vec![col2]).build();
        table.append(data_chunk1).await?;
        table.append(data_chunk2).await?;

        let mut seq_scan_executor = SeqScanExecutor {
            table: Arc::new(table),
            column_ids: vec![0],
            schema: Schema { fields },
            snapshot: VecDeque::new(),
        };
        seq_scan_executor.open().await.unwrap();

        let fields = &seq_scan_executor.schema().fields;
        assert_eq!(fields[0].data_type.data_type_kind(), DataTypeKind::Decimal);

        seq_scan_executor.open().await.unwrap();

        let result_chunk1 = seq_scan_executor.next().await?.unwrap();
        assert_eq!(result_chunk1.dimension(), 1);
        assert_eq!(
            result_chunk1
                .column_at(0)?
                .array()
                .as_int64()
                .iter()
                .collect::<Vec<_>>(),
            vec![Some(1), Some(3), Some(5), Some(7), Some(9)]
        );

        let result_chunk2 = seq_scan_executor.next().await?.unwrap();
        assert_eq!(result_chunk2.dimension(), 1);
        assert_eq!(
            result_chunk2
                .column_at(0)?
                .array()
                .as_int64()
                .iter()
                .collect::<Vec<_>>(),
            vec![Some(2), Some(4), Some(6), Some(8), Some(10)]
        );
        seq_scan_executor.next().await.unwrap();
        seq_scan_executor.close().await.unwrap();

        Ok(())
    }
}