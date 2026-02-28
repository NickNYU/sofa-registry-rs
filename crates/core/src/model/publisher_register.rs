use serde::{Deserialize, Serialize};

use super::base_register::BaseRegister;
use super::data_box::DataBox;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PublisherRegister {
    #[serde(flatten)]
    pub base: BaseRegister,

    #[serde(default, rename = "dataList")]
    pub data_list: Vec<DataBox>,
}
