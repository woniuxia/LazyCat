use serde_json::Value;

mod config;
mod csv;
mod java_bean;
mod markup;
mod sql_entity;

#[cfg(test)]
use sql_entity::needs_table_field;

const ACTIONS: &[&str] = &[
    "json_to_xml",
    "xml_to_json",
    "json_to_yaml",
    "csv_to_json",
    "csv_read_file",
    "java_bean_to_json",
    "json_to_js_object",
    "java_bean_to_js_object",
    "config_convert",
    "yaml_validate",
    "yaml_format",
    "sql_to_entity",
];

#[cfg(test)]
pub(crate) fn supported_actions() -> &'static [&'static str] {
    ACTIONS
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    if !ACTIONS.contains(&action) {
        return Err(format!("unsupported convert action: {action}"));
    }
    match action {
        "json_to_xml" => markup::json_to_xml(payload),
        "xml_to_json" => markup::xml_to_json(payload),
        "json_to_yaml" => markup::json_to_yaml(payload),
        "csv_to_json" => csv::csv_to_json(payload),
        "csv_read_file" => csv::csv_read_file(payload),
        "java_bean_to_json" => java_bean::java_bean_to_json(payload),
        "json_to_js_object" => java_bean::json_to_js_object(payload),
        "java_bean_to_js_object" => java_bean::java_bean_to_js_object(payload),
        "config_convert" => config::config_convert(payload),
        "yaml_validate" => config::yaml_validate(payload),
        "yaml_format" => config::yaml_format(payload),
        "sql_to_entity" => sql_entity::sql_to_entity(payload),
        _ => Err(format!("unsupported convert action: {action}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;

    #[test]
    fn json_xml_yaml_conversions_should_work() {
        let input = r#"{"name":"lazycat","age":1,"active":true}"#;
        let xml = execute("json_to_xml", &json!({ "input": input, "rootTag": "user" }))
            .expect("json_to_xml");
        let xml_text = xml.as_str().expect("xml string");
        assert!(xml_text.contains("<user>"));
        assert!(xml_text.contains("<name>lazycat</name>"));

        let xml_input = "<root><name>lazycat</name><age>1</age></root>";
        let json_out = execute("xml_to_json", &json!({ "input": xml_input })).expect("xml_to_json");
        let json_text = json_out.as_str().expect("json string");
        assert!(json_text.contains("\"name\""));

        let yaml = execute("json_to_yaml", &json!({ "input": input })).expect("json_to_yaml");
        let yaml_text = yaml.as_str().expect("yaml string");
        assert!(yaml_text.contains("name: lazycat"));
    }

    #[test]
    fn csv_to_json_should_support_header_and_selected_columns() {
        let csv = "name,age,city\nalice,18,sz\nbob,20,sh";
        let out = execute(
            "csv_to_json",
            &json!({
                "input": csv,
                "delimiter": ",",
                "hasHeader": true,
                "selectedColumns": [0, 2]
            }),
        )
        .expect("csv_to_json");
        let text = out.as_str().expect("json text");
        assert!(text.contains("\"name\""));
        assert!(text.contains("\"city\""));
        assert!(!text.contains("\"age\""));
    }

    #[test]
    fn csv_to_json_without_header_should_generate_col_names() {
        let csv = "a,1\nb,2";
        let out = execute(
            "csv_to_json",
            &json!({
                "input": csv,
                "delimiter": ",",
                "hasHeader": false
            }),
        )
        .expect("csv_to_json");
        let text = out.as_str().expect("json text");
        assert!(text.contains("\"col1\""));
        assert!(text.contains("\"col2\""));
    }

    #[test]
    fn csv_read_file_should_read_utf8_and_fail_for_missing_file() {
        let path = std::env::temp_dir().join(format!("lazycat-convert-{}.csv", std::process::id()));
        fs::write(&path, "姓名,年龄\n猫,2".as_bytes()).expect("write temp csv");
        let out = execute(
            "csv_read_file",
            &json!({ "path": path.to_string_lossy().to_string() }),
        )
        .expect("csv_read_file");
        assert!(out.as_str().unwrap_or_default().contains("姓名"));
        let _ = fs::remove_file(&path);

        let err = execute("csv_read_file", &json!({ "path": "__not_found__.csv" }))
            .expect_err("should fail");
        assert!(err.contains("read csv file failed"));
    }

    #[test]
    fn java_bean_and_json_to_js_object_should_work() {
        let bean = r#"
            public class User {
              @JsonProperty("user_name")
              private String name;
              private Integer age;
              private List<String> tags;
            }
        "#;
        let out =
            execute("java_bean_to_json", &json!({ "bean": bean })).expect("java_bean_to_json");
        let obj = out.as_object().expect("object");
        let json_text = obj
            .get("json")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        assert!(json_text.contains("\"user_name\""));
        assert!(json_text.contains("\"age\""));
        assert!(json_text.contains("\"tags\""));

        let js = execute(
            "json_to_js_object",
            &json!({
                "json": r#"{"a":1,"b":"x"}"#,
                "quoteStyle": "double"
            }),
        )
        .expect("json_to_js_object");
        let js_text = js["jsObject"].as_str().unwrap_or_default();
        assert!(js_text.contains("const payload ="));

        let bean_js = execute(
            "java_bean_to_js_object",
            &json!({
                "bean": bean,
                "quoteStyle": "single"
            }),
        )
        .expect("java_bean_to_js_object");
        assert!(bean_js["jsObject"]
            .as_str()
            .unwrap_or_default()
            .contains("const payload ="));
    }

    #[test]
    fn convert_invalid_inputs_should_fail() {
        let err =
            execute("json_to_xml", &json!({ "input": "{bad json}" })).expect_err("invalid json");
        assert!(err.contains("invalid json"));

        let err = execute("xml_to_json", &json!({ "input": "<root>" })).expect_err("invalid xml");
        assert!(err.contains("invalid xml"));

        let err = execute("java_bean_to_json", &json!({ "bean": "" })).expect_err("empty bean");
        assert!(err.contains("bean is empty"));

        let err = execute("json_to_js_object", &json!({ "json": "" })).expect_err("empty json");
        assert!(err.contains("json is empty"));
    }

    #[test]
    fn config_convert_properties_to_yaml() {
        let r = execute(
            "config_convert",
            &json!({
                "input": "server.port=8080\nserver.host=localhost",
                "from": "properties",
                "to": "yaml"
            }),
        )
        .unwrap();
        let output = r["output"].as_str().unwrap();
        assert!(output.contains("server"));
        assert!(output.contains("8080"));
    }

    #[test]
    fn config_convert_yaml_to_toml() {
        let r = execute(
            "config_convert",
            &json!({
                "input": "server:\n  port: 8080",
                "from": "yaml",
                "to": "toml"
            }),
        )
        .unwrap();
        let output = r["output"].as_str().unwrap();
        assert!(output.contains("[server]"));
        assert!(output.contains("8080"));
    }

    #[test]
    fn config_convert_env_to_properties() {
        let r = execute(
            "config_convert",
            &json!({
                "input": "DB_HOST=localhost\nDB_PORT=5432",
                "from": "env",
                "to": "properties"
            }),
        )
        .unwrap();
        let output = r["output"].as_str().unwrap();
        assert!(output.contains("DB_HOST=localhost"));
    }

    #[test]
    fn yaml_validate_valid() {
        let r = execute(
            "yaml_validate",
            &json!({"input": "key: value\nlist:\n  - a\n  - b"}),
        )
        .unwrap();
        assert_eq!(r["valid"], true);
        assert!(r["error"].is_null());
    }

    #[test]
    fn yaml_validate_invalid() {
        let r = execute("yaml_validate", &json!({"input": "key: [unclosed"})).unwrap();
        assert_eq!(r["valid"], false);
        assert!(r["error"]["message"].as_str().unwrap().len() > 0);
    }

    #[test]
    fn yaml_format_indent() {
        let r = execute(
            "yaml_format",
            &json!({"input": "key:   value\nlist:\n    - a", "indent": 2}),
        )
        .unwrap();
        let output = r["output"].as_str().unwrap();
        assert!(output.contains("key:"));
    }

    #[test]
    fn yaml_format_preserves_scalar_and_nested_value_types() {
        let input = r#"
integer: 42
decimal: 1.5
enabled: true
missing: null
items:
  - 1
  - false
  - null
nested:
  name: lazycat
"#;
        let result = execute("yaml_format", &json!({"input": input})).expect("format YAML");
        let output = result["output"].as_str().expect("formatted YAML");
        let value: Value = serde_norway::from_str(output).expect("parse formatted YAML");

        assert_eq!(
            value,
            json!({
                "integer": 42,
                "decimal": 1.5,
                "enabled": true,
                "missing": null,
                "items": [1, false, null],
                "nested": { "name": "lazycat" }
            })
        );
    }

    #[test]
    fn yaml_validate_rejects_multiple_documents_and_invalid_tags() {
        for input in [
            "first: document\n---\nsecond: document\n",
            "value: !<tag:example.com,2026:unterminated tagged\n",
        ] {
            let result =
                execute("yaml_validate", &json!({"input": input})).expect("validation result");
            assert_eq!(result["valid"], false, "input must be rejected: {input}");
            assert!(result["error"]["message"]
                .as_str()
                .is_some_and(|message| !message.is_empty()));
        }
    }

    #[test]
    fn sql_to_entity_java_basic() {
        let sql = r#"
            CREATE TABLE t_user (
                id BIGINT NOT NULL AUTO_INCREMENT COMMENT 'primary key',
                user_name VARCHAR(100) NOT NULL COMMENT 'user name',
                age INT DEFAULT 0,
                created_at DATETIME NOT NULL,
                PRIMARY KEY (id)
            );
        "#;
        let r = execute(
            "sql_to_entity",
            &json!({
                "sql": sql,
                "language": "java",
                "options": { "comments": true, "naming": "camelCase" }
            }),
        )
        .unwrap();
        let code = r["code"].as_str().unwrap();
        assert!(code.contains("public class User"));
        assert!(code.contains("private Long id;"));
        assert!(code.contains("private String userName;"));
        assert!(code.contains("private Integer age;"));
        assert!(code.contains("private LocalDateTime createdAt;"));
        assert!(code.contains("/** primary key */"));
        assert!(code.contains("@Data"));
        assert!(code.contains("import lombok.Data;"));
        assert!(!code.contains("getId()"));
        assert!(!code.contains("setUserName("));
        assert!(!code.contains("com.baomidou.mybatisplus.annotation"));
        assert!(!code.contains("@TableName"));
        assert!(!code.contains("@TableField"));
        assert!(!code.contains("@TableId"));

        let tables = r["tables"].as_array().unwrap();
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0]["name"], "t_user");
        let cols = tables[0]["columns"].as_array().unwrap();
        assert_eq!(cols.len(), 4);
        assert_eq!(cols[0]["name"], "id");
        assert_eq!(cols[0]["nullable"], false);
    }

    #[test]
    fn table_field_mapping_uses_default_camel_case() {
        assert!(!needs_table_field("email", "email"));
        assert!(!needs_table_field("created_at", "createdAt"));
        assert!(!needs_table_field("user_name", "userName"));
        assert!(needs_table_field("legacy_code", "legacyCodeValue"));
    }

    #[test]
    fn sql_to_entity_java_mybatis_plus_annotations() {
        let sql = r#"
            CREATE TABLE t_user (
                id BIGINT NOT NULL AUTO_INCREMENT COMMENT 'primary key',
                user_name VARCHAR(100) NOT NULL COMMENT 'user name',
                email VARCHAR(200),
                created_at DATETIME NOT NULL,
                PRIMARY KEY (id)
            );
        "#;
        let r = execute(
            "sql_to_entity",
            &json!({
                "sql": sql,
                "language": "java",
                "options": {
                    "comments": true,
                    "naming": "camelCase",
                    "mybatisPlus": true
                }
            }),
        )
        .unwrap();
        let code = r["code"].as_str().unwrap();

        assert!(code.contains("import com.baomidou.mybatisplus.annotation.IdType;"));
        assert!(!code.contains("import com.baomidou.mybatisplus.annotation.TableField;"));
        assert!(code.contains("import com.baomidou.mybatisplus.annotation.TableId;"));
        assert!(code.contains("import com.baomidou.mybatisplus.annotation.TableName;"));
        assert!(code.contains("@TableName(\"t_user\")"));
        assert!(code.contains("@TableId(type = IdType.AUTO)\n    private Long id;"));
        assert!(!code.contains("@TableField"));
        assert!(code.contains("private String userName;"));
        assert!(code.contains("private LocalDateTime createdAt;"));
    }

    #[test]
    fn sql_to_entity_java_mybatis_plus_inline_primary_key() {
        let sql = "CREATE TABLE account (user_id BIGINT NOT NULL PRIMARY KEY, display_name VARCHAR(100));";
        let r = execute(
            "sql_to_entity",
            &json!({
                "sql": sql,
                "language": "java",
                "options": {
                    "comments": false,
                    "naming": "camelCase",
                    "mybatisPlus": true
                }
            }),
        )
        .unwrap();
        let code = r["code"].as_str().unwrap();

        assert!(code.contains("@TableId(\"user_id\")\n    private Long userId;"));
        assert!(!code.contains("@TableField(\"user_id\")"));
        assert!(!code.contains("import com.baomidou.mybatisplus.annotation.IdType;"));
    }

    #[test]
    fn sql_to_entity_java_mybatis_plus_composite_primary_key() {
        let sql = r#"
            CREATE TABLE order_item (
                order_id BIGINT NOT NULL,
                item_id BIGINT NOT NULL,
                quantity INT,
                PRIMARY KEY (order_id, item_id)
            );
        "#;
        let r = execute(
            "sql_to_entity",
            &json!({
                "sql": sql,
                "language": "java",
                "options": {
                    "comments": false,
                    "naming": "camelCase",
                    "mybatisPlus": true
                }
            }),
        )
        .unwrap();
        let code = r["code"].as_str().unwrap();

        assert!(!code.contains("@TableId"));
        assert!(!code.contains("import com.baomidou.mybatisplus.annotation.TableId;"));
        assert!(!code.contains("import com.baomidou.mybatisplus.annotation.TableField;"));
        assert!(!code.contains("@TableField"));
        assert!(code.contains("private Long orderId;"));
        assert!(code.contains("private Long itemId;"));
    }

    #[test]
    fn sql_to_entity_java_excludes_selected_base_fields_and_extends_parent() {
        let result = execute(
            "sql_to_entity",
            &json!({
                "sql": "CREATE TABLE t_user (id BIGINT NOT NULL AUTO_INCREMENT, tenant_id BIGINT, created_at DATETIME, name VARCHAR(100), PRIMARY KEY (id));",
                "language": "java",
                "options": {
                    "comments": false,
                    "naming": "camelCase",
                    "mybatisPlus": true,
                    "baseClasses": [
                        { "id": 1, "alias": "基础", "qualifiedName": "com.example.BaseEntity", "fields": ["id", "createdAt"] },
                        { "id": 2, "alias": "租户", "qualifiedName": "com.example.TenantFields", "fields": ["tenantId"] }
                    ],
                    "parentBaseClassId": 1
                }
            }),
        )
        .unwrap();
        let code = result["code"].as_str().unwrap();
        assert!(code.contains("import com.example.BaseEntity;"));
        assert!(code.contains("public class User extends BaseEntity"));
        assert!(code.contains("private String name;"));
        assert!(!code.contains("private Long id;"));
        assert!(!code.contains("private Long tenantId;"));
        assert!(!code.contains("private LocalDateTime createdAt;"));
        assert!(!code.contains("TableId"));
        assert!(!code.contains("IdType"));
        assert!(!code.contains("java.time.LocalDateTime"));
        assert!(!code.contains("TenantFields"));
    }

    #[test]
    fn sql_to_entity_java_rejects_parent_outside_selection() {
        let error = execute(
            "sql_to_entity",
            &json!({
                "sql": "CREATE TABLE users (id BIGINT);",
                "language": "java",
                "options": {
                    "baseClasses": [{ "id": 1, "alias": "基础", "qualifiedName": "BaseEntity", "fields": ["id"] }],
                    "parentBaseClassId": 2
                }
            }),
        )
        .unwrap_err();
        assert!(error.contains("实际父类必须属于已选基类"));
    }

    #[test]
    fn sql_to_entity_java_rejects_invalid_base_class_snapshot() {
        let error = execute(
            "sql_to_entity",
            &json!({
                "sql": "CREATE TABLE users (id BIGINT);",
                "language": "java",
                "options": {
                    "baseClasses": [{ "id": 1, "alias": "非法", "qualifiedName": "com.example.1Base", "fields": ["created-at"] }],
                    "parentBaseClassId": 1
                }
            }),
        )
        .unwrap_err();
        assert!(error.contains("非法 Java 标识符"));
    }

    #[test]
    fn sql_to_entity_typescript() {
        let sql = "CREATE TABLE orders (id INT NOT NULL, total DECIMAL(10,2), status VARCHAR(20));";
        let r = execute(
            "sql_to_entity",
            &json!({
                "sql": sql,
                "language": "typescript",
                "options": { "comments": false, "naming": "camelCase" }
            }),
        )
        .unwrap();
        let code = r["code"].as_str().unwrap();
        assert!(code.contains("export interface Orders"));
        assert!(code.contains("id: number;"));
        assert!(code.contains("total?: number;"));
        assert!(code.contains("status?: string;"));
    }

    #[test]
    fn sql_to_entity_go() {
        let sql = "CREATE TABLE `product` (id BIGINT NOT NULL, name VARCHAR(255), price FLOAT);";
        let r = execute(
            "sql_to_entity",
            &json!({
                "sql": sql,
                "language": "go",
                "options": { "comments": false, "naming": "camelCase" }
            }),
        )
        .unwrap();
        let code = r["code"].as_str().unwrap();
        assert!(code.contains("type Product struct"));
        assert!(code.contains("Id int64"));
        assert!(code.contains("*string"));
        assert!(code.contains("json:\"name\""));
    }

    #[test]
    fn sql_to_entity_python() {
        let sql =
            "CREATE TABLE user_profile (user_id INT NOT NULL, bio TEXT, created DATE NOT NULL);";
        let r = execute(
            "sql_to_entity",
            &json!({
                "sql": sql,
                "language": "python",
                "options": { "comments": true, "naming": "snake_case" }
            }),
        )
        .unwrap();
        let code = r["code"].as_str().unwrap();
        assert!(code.contains("@dataclass"));
        assert!(code.contains("class UserProfile:"));
        assert!(code.contains("user_id: int"));
        assert!(code.contains("bio: Optional[str]"));
        assert!(code.contains("from datetime import"));
    }

    #[test]
    fn sql_to_entity_kotlin() {
        let sql =
            "CREATE TABLE IF NOT EXISTS config (id INT NOT NULL, value TEXT, active BOOLEAN);";
        let r = execute(
            "sql_to_entity",
            &json!({
                "sql": sql,
                "language": "kotlin",
                "options": { "comments": false, "naming": "camelCase" }
            }),
        )
        .unwrap();
        let code = r["code"].as_str().unwrap();
        assert!(code.contains("data class Config("));
        assert!(code.contains("val id: Int"));
        assert!(code.contains("val value: String?"));
        assert!(code.contains("val active: Boolean?"));
    }

    #[test]
    fn sql_to_entity_csharp() {
        let sql = "CREATE TABLE [order_items] (id INT NOT NULL, order_id BIGINT NOT NULL, quantity INT DEFAULT 1);";
        let r = execute(
            "sql_to_entity",
            &json!({
                "sql": sql,
                "language": "csharp",
                "options": { "comments": false, "naming": "camelCase" }
            }),
        )
        .unwrap();
        let code = r["code"].as_str().unwrap();
        assert!(code.contains("public class OrderItems"));
        assert!(code.contains("public int Id { get; set; }"));
        assert!(code.contains("public long OrderId { get; set; }"));
        assert!(code.contains("public int? Quantity { get; set; }"));
    }

    #[test]
    fn sql_to_entity_multiple_tables() {
        let sql = r#"
            CREATE TABLE users (id INT NOT NULL, name VARCHAR(100));
            CREATE TABLE roles (id INT NOT NULL, role_name VARCHAR(50) NOT NULL);
        "#;
        let r = execute(
            "sql_to_entity",
            &json!({
                "sql": sql,
                "language": "java",
                "options": { "comments": false, "naming": "camelCase" }
            }),
        )
        .unwrap();
        let tables = r["tables"].as_array().unwrap();
        assert_eq!(tables.len(), 2);
        let code = r["code"].as_str().unwrap();
        assert!(code.contains("public class Users"));
        assert!(code.contains("public class Roles"));
    }

    #[test]
    fn sql_to_entity_empty_input() {
        let err = execute(
            "sql_to_entity",
            &json!({
                "sql": "",
                "language": "java",
                "options": {}
            }),
        )
        .expect_err("empty sql");
        assert!(err.contains("SQL is empty"));
    }

    #[test]
    fn sql_to_entity_no_create_table() {
        let err = execute(
            "sql_to_entity",
            &json!({
                "sql": "SELECT * FROM users;",
                "language": "java",
                "options": {}
            }),
        )
        .expect_err("no create table");
        assert!(err.contains("No CREATE TABLE"));
    }

    #[test]
    fn sql_to_entity_tinyint1_is_boolean() {
        let sql =
            "CREATE TABLE flags (id INT NOT NULL, active TINYINT(1) NOT NULL, count TINYINT(4));";
        let r = execute(
            "sql_to_entity",
            &json!({
                "sql": sql,
                "language": "java",
                "options": { "comments": false, "naming": "camelCase" }
            }),
        )
        .unwrap();
        let code = r["code"].as_str().unwrap();
        assert!(code.contains("private Boolean active;"));
        assert!(code.contains("private Integer count;"));
    }

    #[test]
    fn sql_to_entity_snake_case_naming() {
        let sql = "CREATE TABLE user_settings (user_id INT NOT NULL, config_value TEXT);";
        let r = execute(
            "sql_to_entity",
            &json!({
                "sql": sql,
                "language": "java",
                "options": { "comments": false, "naming": "snake_case" }
            }),
        )
        .unwrap();
        let code = r["code"].as_str().unwrap();
        assert!(code.contains("private Integer user_id;"));
        assert!(code.contains("private String config_value;"));
    }
}
