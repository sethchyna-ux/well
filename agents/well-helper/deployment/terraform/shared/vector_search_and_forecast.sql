-- Copyright 2026 Google LLC
--
-- Licensed under the Apache License, Version 2.0 (the "License");
-- you may not use this file except in compliance with the License.
-- You may obtain a copy of the License at
--
--     https://www.apache.org/licenses/LICENSE-2.0
--
-- Unless required by applicable law or agreed to in writing, software
-- distributed under the License is distributed on an "AS IS" BASIS,
-- WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
-- See the License for the specific language governing permissions and
-- limitations under the License.

-- ====================================================================
-- BigQuery AI/ML Pipeline & Vector Search Analytics
-- ====================================================================

-- 1. Daily Aggregated Telemetry View
-- Regular time-interval projection suitable for AI.FORECAST and AI.DETECT_ANOMALIES
CREATE OR REPLACE VIEW `${project_id}.${dataset_id}.daily_telemetry_metrics` AS
SELECT
  TIMESTAMP_TRUNC(timestamp, DAY) AS metric_day,
  model,
  SUM(COALESCE(usage_input_tokens, 0) + COALESCE(usage_output_tokens, 0)) AS total_tokens,
  COUNT(DISTINCT trace) AS total_conversations,
  COUNTIF(status != 'OK') AS error_count,
  COUNT(*) AS total_messages
FROM `${project_id}.${dataset_id}.completions_view`
GROUP BY metric_day, model;

-- 2. Daily Token Consumption Forecasting (TimesFM / ARIMA_PLUS)
-- Forecast token utilization 7 days into the future
-- (Run as an ad-hoc query or scheduled analytics job)
--
-- SELECT *
-- FROM AI.FORECAST(
--   TABLE `${project_id}.${dataset_id}.daily_telemetry_metrics`,
--   data_col => 'total_tokens',
--   timestamp_col => 'metric_day',
--   horizon => 7
-- );

-- 3. Latency & Failure Anomaly Detection
-nd 2wa- Flag anomalous spikes in daily errors or invocation rates
--
-- SELECT *
-- FROM AI.DETECT_ANOMALIES(
--   TABLE `${project_id}.${dataset_id}.daily_telemetry_metrics`,
--   data_col => 'error_count',
--   timestamp_col => 'metric_day',
--   id_cols => ['model'],
--   anomaly_prob_threshold => 0.95
-- );

-- 4. Incremental Native Ingest & Embedding Generation
-- Materialize views into a partitioned native table to support VECTOR_SEARCH
CREATE TABLE IF NOT EXISTS `${project_id}.${dataset_id}.completions_native` (
  timestamp TIMESTAMP,
  insert_id STRING,
  trace STRING,
  span_id STRING,
  conversation_id STRING,
  api_call_id STRING,
  message_type STRING,
  role STRING,
  message_idx INT64,
  part_idx INT64,
  content STRING,
  part_type STRING,
  tool_name STRING,
  tool_args JSON,
  tool_response JSON,
  usage_input_tokens INT64,
  usage_output_tokens INT64,
  agent_name STRING,
  model STRING,
  finish_reasons STRING,
  status STRING,
  uri STRING,
  mime_type STRING,
  data_md5_hex STRING,
  messages_ref_uri STRING,
  content_embedding ARRAY<FLOAT64>
)
PARTITION BY DATE(timestamp)
CLUSTER BY conversation_id, model, message_type;

-- 5. Incremental Merge from completions_view into completions_native
-- Partition pruning predicate on T.timestamp ensures only recent partitions are scanned
MERGE `${project_id}.${dataset_id}.completions_native` T
USING (
  SELECT
    *,
    CAST(NULL AS ARRAY<FLOAT64>) AS content_embedding
  FROM `${project_id}.${dataset_id}.completions_view`
  WHERE timestamp >= TIMESTAMP_SUB(CURRENT_TIMESTAMP(), INTERVAL 2 DAY)
) S
ON DATE(T.timestamp) >= DATE_SUB(CURRENT_DATE(), INTERVAL 2 DAY)
  AND T.conversation_id = S.conversation_id
  AND T.insert_id = S.insert_id
  AND T.message_type = S.message_type
  AND T.message_idx = S.message_idx
  AND T.part_idx = S.part_idx
WHEN NOT MATCHED THEN
  INSERT ROW;

-- 6. Generate Embeddings for Vector Search
-- (Using Vertex AI text-embedding-005 endpoint connection)
--
-- UPDATE `${project_id}.${dataset_id}.completions_native`
-- SET content_embedding = ml_generate_embedding_result
-- FROM (
--   SELECT insert_id, message_idx, part_idx, ml_generate_embedding_result
--   FROM ML.GENERATE_EMBEDDING(
--     MODEL `${project_id}.${dataset_id}.embedding_model`,
--     (SELECT insert_id, message_idx, part_idx, content
--      FROM `${project_id}.${dataset_id}.completions_native`
--      WHERE content_embedding IS NULL AND content IS NOT NULL),
--     STRUCT(TRUE AS flatten_json_output)
--   )
-- ) emb
-- WHERE completions_native.insert_id = emb.insert_id
--   AND completions_native.message_idx = emb.message_idx
--   AND completions_native.part_idx = emb.part_idx;
