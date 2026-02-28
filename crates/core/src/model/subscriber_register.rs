use serde::{Deserialize, Serialize};

use super::base_register::BaseRegister;
use super::scope::Scope;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SubscriberRegister {
    #[serde(flatten)]
    pub base: BaseRegister,

    #[serde(default)]
    pub scope: Scope,

    #[serde(default, rename = "acceptEncoding")]
    pub accept_encoding: Option<String>,

    #[serde(default, rename = "acceptMulti")]
    pub accept_multi: bool,
}
