use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MilvusMetricType {
    #[default]
    Cosine,
    Ip,
    L2,
}

impl MilvusMetricType {
    pub fn as_milvus_str(self) -> &'static str {
        match self {
            Self::Cosine => "COSINE",
            Self::Ip => "IP",
            Self::L2 => "L2",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MilvusSearchRequest {
    pub collection_name: String,
    pub anns_field: String,
    pub query_vector: Vec<f32>,
    pub top_k: usize,
    pub tenant_id: i64,
    pub dataset_id: i64,
    pub document_ids: Vec<i64>,
    pub output_fields: Vec<String>,
    pub metric_type: MilvusMetricType,
}

impl MilvusSearchRequest {
    pub fn new(
        collection_name: impl Into<String>,
        query_vector: Vec<f32>,
        top_k: usize,
        tenant_id: i64,
        dataset_id: i64,
    ) -> Self {
        Self {
            collection_name: collection_name.into().trim().to_owned(),
            anns_field: "embedding".to_owned(),
            query_vector,
            top_k,
            tenant_id,
            dataset_id,
            document_ids: vec![],
            output_fields: vec![
                "chunk_uid".to_owned(),
                "chunk_db_id".to_owned(),
                "document_id".to_owned(),
            ],
            metric_type: MilvusMetricType::Cosine,
        }
    }

    pub fn with_document_ids(mut self, document_ids: Vec<i64>) -> Self {
        self.document_ids = normalized_positive_ids(document_ids);
        self
    }

    pub fn with_output_fields<I, S>(mut self, output_fields: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let fields = output_fields
            .into_iter()
            .map(Into::into)
            .map(|field| field.trim().to_owned())
            .filter(|field| !field.is_empty())
            .collect::<Vec<_>>();
        if !fields.is_empty() {
            self.output_fields = fields;
        }
        self
    }

    pub fn with_metric_type(mut self, metric_type: MilvusMetricType) -> Self {
        self.metric_type = metric_type;
        self
    }

    pub fn filter_expression(&self) -> String {
        let mut parts = vec![
            format!("tenant_id == {}", self.tenant_id),
            format!("dataset_id == {}", self.dataset_id),
        ];
        if !self.document_ids.is_empty() {
            let ids = self
                .document_ids
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            parts.push(format!("document_id in [{ids}]"));
        }
        parts.join(" and ")
    }

    pub fn to_rest_search_body(&self) -> Value {
        json!({
            "collectionName": self.collection_name,
            "data": [self.query_vector],
            "annsField": self.anns_field,
            "filter": self.filter_expression(),
            "limit": self.top_k.max(1),
            "outputFields": self.output_fields,
            "searchParams": {
                "metric_type": self.metric_type.as_milvus_str(),
            },
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MilvusSearchHit {
    pub chunk_uid: String,
    pub score: f32,
    pub chunk_db_id: Option<i64>,
    pub document_id: Option<i64>,
    pub fields: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MilvusCreateCollectionRequest {
    pub collection_name: String,
    pub dimension: usize,
    pub metric_type: MilvusMetricType,
}

impl MilvusCreateCollectionRequest {
    pub fn new(
        collection_name: impl Into<String>,
        dimension: usize,
        metric_type: MilvusMetricType,
    ) -> Self {
        Self {
            collection_name: collection_name.into().trim().to_owned(),
            dimension,
            metric_type,
        }
    }

    pub fn to_rest_create_body(&self) -> Value {
        json!({
            "collectionName": self.collection_name,
            "schema": {
                "autoID": false,
                "enableDynamicField": false,
                "fields": [
                    {
                        "fieldName": "id",
                        "dataType": "Int64",
                        "isPrimary": true,
                    },
                    {
                        "fieldName": "chunk_db_id",
                        "dataType": "Int64",
                    },
                    {
                        "fieldName": "tenant_id",
                        "dataType": "Int64",
                    },
                    {
                        "fieldName": "dataset_id",
                        "dataType": "Int64",
                    },
                    {
                        "fieldName": "document_id",
                        "dataType": "Int64",
                    },
                    {
                        "fieldName": "chunk_uid",
                        "dataType": "VarChar",
                        "elementTypeParams": {"max_length": 255},
                    },
                    {
                        "fieldName": "chunk_index",
                        "dataType": "Int32",
                    },
                    {
                        "fieldName": "embedding",
                        "dataType": "FloatVector",
                        "elementTypeParams": {"dim": self.dimension},
                    },
                    {
                        "fieldName": "semantic_search_text",
                        "dataType": "VarChar",
                        "elementTypeParams": {"max_length": 8192},
                    },
                    {
                        "fieldName": "segment_type",
                        "dataType": "VarChar",
                        "elementTypeParams": {"max_length": 64},
                    },
                    {
                        "fieldName": "content_role",
                        "dataType": "VarChar",
                        "elementTypeParams": {"max_length": 64},
                    },
                ],
            },
            "indexParams": [
                {
                    "fieldName": "embedding",
                    "indexName": "embedding_idx",
                    "metricType": self.metric_type.as_milvus_str(),
                    "params": {
                        "index_type": "AUTOINDEX",
                    },
                },
            ],
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MilvusUpsertRow {
    pub id: i64,
    pub tenant_id: i64,
    pub dataset_id: i64,
    pub document_id: i64,
    pub chunk_uid: String,
    pub chunk_index: i32,
    pub embedding: Vec<f32>,
    pub semantic_search_text: String,
    pub segment_type: String,
    pub content_role: String,
}

impl MilvusUpsertRow {
    fn to_entity(&self) -> Value {
        json!({
            "id": self.id,
            "chunk_db_id": self.id,
            "tenant_id": self.tenant_id,
            "dataset_id": self.dataset_id,
            "document_id": self.document_id,
            "chunk_uid": self.chunk_uid,
            "chunk_index": self.chunk_index,
            "embedding": self.embedding,
            "semantic_search_text": self.semantic_search_text,
            "segment_type": self.segment_type,
            "content_role": self.content_role,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MilvusUpsertRequest {
    pub collection_name: String,
    pub rows: Vec<MilvusUpsertRow>,
}

impl MilvusUpsertRequest {
    pub fn new(collection_name: impl Into<String>, rows: Vec<MilvusUpsertRow>) -> Self {
        Self {
            collection_name: collection_name.into().trim().to_owned(),
            rows,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn to_rest_upsert_body(&self) -> Value {
        let data = self
            .rows
            .iter()
            .map(MilvusUpsertRow::to_entity)
            .collect::<Vec<_>>();
        json!({
            "collectionName": self.collection_name,
            "data": data,
        })
    }
}

pub fn parse_milvus_search_hits(response: &Value) -> Vec<MilvusSearchHit> {
    let Some(rows) = milvus_hits_container(response) else {
        return vec![];
    };

    let mut raw_hits = Vec::new();
    collect_milvus_hit_rows(rows, &mut raw_hits);
    raw_hits
        .into_iter()
        .filter_map(milvus_search_hit_from_value)
        .collect()
}

fn normalized_positive_ids(mut ids: Vec<i64>) -> Vec<i64> {
    ids.retain(|id| *id > 0);
    ids.sort_unstable();
    ids.dedup();
    ids
}

fn milvus_hits_container(response: &Value) -> Option<&Value> {
    response
        .get("data")
        .or_else(|| response.get("results"))
        .or_else(|| response.get("result"))
        .or_else(|| response.get("hits"))
}

fn collect_milvus_hit_rows<'a>(value: &'a Value, rows: &mut Vec<&'a Value>) {
    match value {
        Value::Array(items) => {
            for item in items {
                if item.is_array() {
                    collect_milvus_hit_rows(item, rows);
                } else if let Some(nested) = milvus_hits_container(item) {
                    collect_milvus_hit_rows(nested, rows);
                } else {
                    rows.push(item);
                }
            }
        }
        Value::Object(_) => {
            if let Some(nested) = milvus_hits_container(value) {
                collect_milvus_hit_rows(nested, rows);
            } else {
                rows.push(value);
            }
        }
        _ => {}
    }
}

fn milvus_search_hit_from_value(value: &Value) -> Option<MilvusSearchHit> {
    let fields = merged_milvus_hit_fields(value);
    let chunk_uid = string_field(&fields, &["chunk_uid", "chunkUid"])?
        .trim()
        .to_owned();
    if chunk_uid.is_empty() {
        return None;
    }
    let score = f32_field(&fields, &["score", "distance"])?;
    if !score.is_finite() {
        return None;
    }

    Some(MilvusSearchHit {
        chunk_uid,
        score,
        chunk_db_id: i64_field(&fields, &["chunk_db_id", "chunkDbId"]),
        document_id: i64_field(&fields, &["document_id", "documentId"]),
        fields,
    })
}

fn merged_milvus_hit_fields(value: &Value) -> Value {
    let mut fields = Map::new();
    merge_object_fields(value, &mut fields);
    if let Some(entity) = value.get("entity") {
        merge_object_fields(entity, &mut fields);
    }
    if let Some(output_fields) = value.get("fields") {
        merge_object_fields(output_fields, &mut fields);
    }
    Value::Object(fields)
}

fn merge_object_fields(value: &Value, fields: &mut Map<String, Value>) {
    let Some(object) = value.as_object() else {
        return;
    };
    for (key, value) in object {
        if matches!(key.as_str(), "entity" | "fields") {
            continue;
        }
        fields.insert(key.clone(), value.clone());
    }
}

fn string_field<'a>(value: &'a Value, names: &[&str]) -> Option<&'a str> {
    names
        .iter()
        .find_map(|name| value.get(*name).and_then(Value::as_str))
}

fn f32_field(value: &Value, names: &[&str]) -> Option<f32> {
    names.iter().find_map(|name| {
        let value = value.get(*name)?;
        if let Some(number) = value.as_f64() {
            return Some(number as f32);
        }
        value.as_str()?.parse::<f32>().ok()
    })
}

fn i64_field(value: &Value, names: &[&str]) -> Option<i64> {
    names.iter().find_map(|name| {
        let value = value.get(*name)?;
        if let Some(number) = value.as_i64() {
            return Some(number);
        }
        value.as_str()?.parse::<i64>().ok()
    })
}
