use sofa_registry_mcp::tools;

#[test]
fn all_tool_definitions_returns_five_tools() {
    let defs = tools::all_tool_definitions();
    assert_eq!(defs.len(), 5);
}

#[test]
fn all_tool_definitions_contains_list_services() {
    let defs = tools::all_tool_definitions();
    assert!(
        defs.iter().any(|t| t.name == "list_services"),
        "expected 'list_services' tool to be present"
    );
}

#[test]
fn all_tool_definitions_contains_get_service_info() {
    let defs = tools::all_tool_definitions();
    assert!(
        defs.iter().any(|t| t.name == "get_service_info"),
        "expected 'get_service_info' tool to be present"
    );
}

#[test]
fn all_tool_definitions_contains_search_services() {
    let defs = tools::all_tool_definitions();
    assert!(
        defs.iter().any(|t| t.name == "search_services"),
        "expected 'search_services' tool to be present"
    );
}

#[test]
fn all_tool_definitions_contains_get_cluster_health() {
    let defs = tools::all_tool_definitions();
    assert!(
        defs.iter().any(|t| t.name == "get_cluster_health"),
        "expected 'get_cluster_health' tool to be present"
    );
}

#[test]
fn all_tool_definitions_contains_get_slot_table() {
    let defs = tools::all_tool_definitions();
    assert!(
        defs.iter().any(|t| t.name == "get_slot_table"),
        "expected 'get_slot_table' tool to be present"
    );
}

#[test]
fn tool_names_are_unique() {
    let defs = tools::all_tool_definitions();
    let mut names: Vec<&str> = defs.iter().map(|t| t.name.as_str()).collect();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), defs.len(), "tool names must be unique");
}

#[test]
fn all_tools_have_non_empty_descriptions() {
    let defs = tools::all_tool_definitions();
    for tool in &defs {
        assert!(
            !tool.description.is_empty(),
            "tool '{}' should have a non-empty description",
            tool.name
        );
    }
}

#[test]
fn all_tools_have_object_input_schema() {
    let defs = tools::all_tool_definitions();
    for tool in &defs {
        let schema_type = tool.input_schema.get("type").and_then(|v| v.as_str());
        assert_eq!(
            schema_type,
            Some("object"),
            "tool '{}' input_schema type should be 'object'",
            tool.name
        );
    }
}

#[test]
fn all_tools_have_properties_field() {
    let defs = tools::all_tool_definitions();
    for tool in &defs {
        assert!(
            tool.input_schema.get("properties").is_some(),
            "tool '{}' input_schema should have 'properties'",
            tool.name
        );
    }
}

#[test]
fn all_tools_have_required_field() {
    let defs = tools::all_tool_definitions();
    for tool in &defs {
        assert!(
            tool.input_schema.get("required").is_some(),
            "tool '{}' input_schema should have 'required'",
            tool.name
        );
    }
}

// ---------------------------------------------------------------------------
// list_services schema
// ---------------------------------------------------------------------------

#[test]
fn list_services_schema_has_data_center_property() {
    let defs = tools::all_tool_definitions();
    let tool = defs.iter().find(|t| t.name == "list_services").unwrap();
    let props = tool.input_schema.get("properties").unwrap();
    assert!(
        props.get("data_center").is_some(),
        "list_services should have 'data_center' property"
    );
}

#[test]
fn list_services_has_no_required_params() {
    let defs = tools::all_tool_definitions();
    let tool = defs.iter().find(|t| t.name == "list_services").unwrap();
    let required = tool.input_schema.get("required").unwrap().as_array().unwrap();
    assert!(required.is_empty(), "list_services should have no required params");
}

// ---------------------------------------------------------------------------
// get_service_info schema
// ---------------------------------------------------------------------------

#[test]
fn get_service_info_requires_data_id() {
    let defs = tools::all_tool_definitions();
    let tool = defs.iter().find(|t| t.name == "get_service_info").unwrap();
    let required = tool.input_schema.get("required").unwrap().as_array().unwrap();
    assert!(
        required.iter().any(|v| v.as_str() == Some("data_id")),
        "get_service_info should require 'data_id'"
    );
}

#[test]
fn get_service_info_schema_has_data_id_property() {
    let defs = tools::all_tool_definitions();
    let tool = defs.iter().find(|t| t.name == "get_service_info").unwrap();
    let props = tool.input_schema.get("properties").unwrap();
    let data_id = props.get("data_id").unwrap();
    assert_eq!(
        data_id.get("type").and_then(|v| v.as_str()),
        Some("string")
    );
}

// ---------------------------------------------------------------------------
// search_services schema
// ---------------------------------------------------------------------------

#[test]
fn search_services_requires_pattern() {
    let defs = tools::all_tool_definitions();
    let tool = defs.iter().find(|t| t.name == "search_services").unwrap();
    let required = tool.input_schema.get("required").unwrap().as_array().unwrap();
    assert!(
        required.iter().any(|v| v.as_str() == Some("pattern")),
        "search_services should require 'pattern'"
    );
}

#[test]
fn search_services_schema_has_pattern_property() {
    let defs = tools::all_tool_definitions();
    let tool = defs.iter().find(|t| t.name == "search_services").unwrap();
    let props = tool.input_schema.get("properties").unwrap();
    let pattern = props.get("pattern").unwrap();
    assert_eq!(
        pattern.get("type").and_then(|v| v.as_str()),
        Some("string")
    );
}

// ---------------------------------------------------------------------------
// get_cluster_health schema
// ---------------------------------------------------------------------------

#[test]
fn get_cluster_health_has_empty_properties() {
    let defs = tools::all_tool_definitions();
    let tool = defs
        .iter()
        .find(|t| t.name == "get_cluster_health")
        .unwrap();
    let props = tool
        .input_schema
        .get("properties")
        .unwrap()
        .as_object()
        .unwrap();
    assert!(props.is_empty(), "get_cluster_health should have no properties");
}

#[test]
fn get_cluster_health_has_no_required_params() {
    let defs = tools::all_tool_definitions();
    let tool = defs
        .iter()
        .find(|t| t.name == "get_cluster_health")
        .unwrap();
    let required = tool.input_schema.get("required").unwrap().as_array().unwrap();
    assert!(required.is_empty());
}

// ---------------------------------------------------------------------------
// get_slot_table schema
// ---------------------------------------------------------------------------

#[test]
fn get_slot_table_has_empty_properties() {
    let defs = tools::all_tool_definitions();
    let tool = defs.iter().find(|t| t.name == "get_slot_table").unwrap();
    let props = tool
        .input_schema
        .get("properties")
        .unwrap()
        .as_object()
        .unwrap();
    assert!(props.is_empty(), "get_slot_table should have no properties");
}

#[test]
fn get_slot_table_has_no_required_params() {
    let defs = tools::all_tool_definitions();
    let tool = defs.iter().find(|t| t.name == "get_slot_table").unwrap();
    let required = tool.input_schema.get("required").unwrap().as_array().unwrap();
    assert!(required.is_empty());
}

// ---------------------------------------------------------------------------
// ToolDefinition serde
// ---------------------------------------------------------------------------

#[test]
fn tool_definitions_serialize_to_json() {
    let defs = tools::all_tool_definitions();
    let json = serde_json::to_string(&defs).expect("should serialize");
    assert!(json.contains("list_services"));
    assert!(json.contains("get_service_info"));
    assert!(json.contains("search_services"));
    assert!(json.contains("get_cluster_health"));
    assert!(json.contains("get_slot_table"));
}

#[test]
fn tool_definition_serializes_input_schema_as_input_schema_key() {
    let defs = tools::all_tool_definitions();
    let json = serde_json::to_string(&defs[0]).expect("should serialize");
    // The field should be serialized as "inputSchema" (camelCase) per serde rename
    assert!(
        json.contains("inputSchema"),
        "expected 'inputSchema' key in serialized JSON, got: {}",
        json
    );
}
