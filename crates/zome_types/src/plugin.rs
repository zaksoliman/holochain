use holochain_serialized_bytes::prelude::*;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, SerializedBytes)]
pub struct PluginCall {
    pub lib: String,
    pub func: String,
    pub payload: SerializedBytes,
}
