use sofa_registry_core::model::Scope;

/// Builder for registering a publisher with the registry.
#[derive(Debug, Clone)]
pub struct PublisherRegistration {
    pub data_id: String,
    pub group: String,
    pub instance_id: String,
    pub app_name: Option<String>,
}

impl PublisherRegistration {
    pub fn new(data_id: impl Into<String>) -> Self {
        Self {
            data_id: data_id.into(),
            group: "DEFAULT_GROUP".to_string(),
            instance_id: "DEFAULT_INSTANCE_ID".to_string(),
            app_name: None,
        }
    }

    pub fn with_group(mut self, group: impl Into<String>) -> Self {
        self.group = group.into();
        self
    }

    pub fn with_instance_id(mut self, id: impl Into<String>) -> Self {
        self.instance_id = id.into();
        self
    }

    pub fn with_app_name(mut self, name: impl Into<String>) -> Self {
        self.app_name = Some(name.into());
        self
    }

    pub fn data_info_id(&self) -> String {
        format!("{}#{}#{}", self.data_id, self.instance_id, self.group)
    }
}

/// Builder for registering a subscriber with the registry.
#[derive(Debug, Clone)]
pub struct SubscriberRegistration {
    pub data_id: String,
    pub group: String,
    pub instance_id: String,
    pub scope: Scope,
    pub app_name: Option<String>,
}

impl SubscriberRegistration {
    pub fn new(data_id: impl Into<String>) -> Self {
        Self {
            data_id: data_id.into(),
            group: "DEFAULT_GROUP".to_string(),
            instance_id: "DEFAULT_INSTANCE_ID".to_string(),
            scope: Scope::default(),
            app_name: None,
        }
    }

    pub fn with_group(mut self, group: impl Into<String>) -> Self {
        self.group = group.into();
        self
    }

    pub fn with_instance_id(mut self, id: impl Into<String>) -> Self {
        self.instance_id = id.into();
        self
    }

    pub fn with_scope(mut self, scope: Scope) -> Self {
        self.scope = scope;
        self
    }

    pub fn with_app_name(mut self, name: impl Into<String>) -> Self {
        self.app_name = Some(name.into());
        self
    }

    pub fn data_info_id(&self) -> String {
        format!("{}#{}#{}", self.data_id, self.instance_id, self.group)
    }
}
