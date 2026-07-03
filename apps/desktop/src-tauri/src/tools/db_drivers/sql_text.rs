//! SQL 文本纯函数：语句拆分、只读/危险分类、无 WHERE 检测、标识符转义。
//!
//! 前端 `utils/dbSqlClassify.ts` 维护同一套规则与测试向量，两端行为必须一致：
//! 前端用于提示，后端用于强制拦截（后端不信任前端结论）。

/// SQL 方言。影响 `#` 行注释（MySQL 专属；PG 中 `#>` 等是 JSONB 操作符）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlDialect {
    MySql,
    Pg,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StatementInfo {
    /// 只读形态：SELECT / WITH…SELECT / VALUES / SHOW / EXPLAIN / DESC(RIBE) / 括号包裹 SELECT
    pub readonly: bool,
    /// 数据变更：INSERT / UPDATE / DELETE / REPLACE / MERGE / TRUNCATE
    pub dml: bool,
    /// 结构变更：CREATE / ALTER / DROP / RENAME / COMMENT / GRANT / REVOKE
    pub ddl: bool,
    /// 首个有效关键词（大写）
    pub verb: String,
    /// UPDATE / DELETE 且顶层缺少 WHERE
    pub missing_where: bool,
}

/// 按分号拆分多语句；跳过单双引号字符串、反引号标识符、行注释、
/// 块注释与 PG 美元引用（$$…$$ / $tag$…$tag$）。返回去除首尾空白的非空语句。
pub fn split_statements(sql: &str, dialect: SqlDialect) -> Vec<String> {
    let mut out = Vec::new();
    let mut start = 0usize;
    let bytes = sql.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        let rest = &sql[i..];
        if let Some(skip) = skip_non_code(rest, dialect) {
            i += skip;
            continue;
        }
        let ch = rest.chars().next().unwrap();
        if ch == ';' {
            let stmt = sql[start..i].trim();
            if !is_blank_statement(stmt, dialect) {
                out.push(stmt.to_string());
            }
            i += 1;
            start = i;
            continue;
        }
        i += ch.len_utf8();
    }
    let tail = sql[start..].trim();
    if !is_blank_statement(tail, dialect) {
        out.push(tail.to_string());
    }
    out
}

/// 若当前位置是字符串/注释/美元引用的开头，返回整段字节长度；否则 None。
fn skip_non_code(rest: &str, dialect: SqlDialect) -> Option<usize> {
    let first = rest.chars().next()?;
    match first {
        '\'' | '"' | '`' => Some(skip_quoted(rest, first)),
        '-' => {
            if rest.starts_with("--") {
                Some(rest.find('\n').map(|p| p + 1).unwrap_or(rest.len()))
            } else {
                None
            }
        }
        '#' if dialect == SqlDialect::MySql => {
            Some(rest.find('\n').map(|p| p + 1).unwrap_or(rest.len()))
        }
        '/' => {
            if rest.starts_with("/*") {
                Some(rest.find("*/").map(|p| p + 2).unwrap_or(rest.len()))
            } else {
                None
            }
        }
        '$' if dialect == SqlDialect::Pg => skip_dollar_quoted(rest),
        _ => None,
    }
}

/// 跳过引号包裹段。支持成对引号转义（'' / "" / ``）；MySQL 方言下字符串
/// 还支持反斜杠转义（对反引号标识符不适用）。PG 标准模式反斜杠为字面量，
/// 但为兼容 KB 的 escape 字符串这里对两种方言都按转义处理（误差仅出现在
/// 以反斜杠结尾的字符串这一罕见形态）。
fn skip_quoted(rest: &str, quote: char) -> usize {
    let mut iter = rest.char_indices();
    iter.next(); // 开头引号
    while let Some((idx, ch)) = iter.next() {
        if ch == '\\' && quote != '`' {
            iter.next();
            continue;
        }
        if ch == quote {
            // 成对引号转义
            if rest[idx + ch.len_utf8()..].starts_with(quote) {
                iter.next();
                continue;
            }
            return idx + ch.len_utf8();
        }
    }
    rest.len()
}

/// 跳过 PG 美元引用：$$…$$ 或 $tag$…$tag$。非美元引用开头（如 $1 参数）返回 None。
fn skip_dollar_quoted(rest: &str) -> Option<usize> {
    let after = &rest[1..];
    let tag_end = after.find('$')?;
    let tag = &after[..tag_end];
    if !tag.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    let opener_len = 1 + tag_end + 1; // $tag$
    let closer = format!("${tag}$");
    match rest[opener_len..].find(&closer) {
        Some(p) => Some(opener_len + p + closer.len()),
        None => Some(rest.len()),
    }
}

/// 语句是否为空（仅空白或注释）。
fn is_blank_statement(stmt: &str, dialect: SqlDialect) -> bool {
    words_with_depth(stmt, dialect).is_empty()
}

/// 提取字符串与注释之外的单词序列（统一大写），并记录每个词所在括号深度。
fn words_with_depth(stmt: &str, dialect: SqlDialect) -> Vec<(String, i32)> {
    let mut out = Vec::new();
    let bytes = stmt.as_bytes();
    let mut i = 0usize;
    let mut depth = 0i32;
    let mut word = String::new();
    let mut word_depth = 0i32;
    fn flush(word: &mut String, word_depth: i32, out: &mut Vec<(String, i32)>) {
        if !word.is_empty() {
            out.push((word.to_ascii_uppercase(), word_depth));
            word.clear();
        }
    }
    while i < bytes.len() {
        let rest = &stmt[i..];
        if let Some(skip) = skip_non_code(rest, dialect) {
            flush(&mut word, word_depth, &mut out);
            i += skip;
            continue;
        }
        let ch = rest.chars().next().unwrap();
        match ch {
            '(' => {
                flush(&mut word, word_depth, &mut out);
                depth += 1;
            }
            ')' => {
                flush(&mut word, word_depth, &mut out);
                depth -= 1;
            }
            c if c.is_ascii_alphanumeric() || c == '_' => {
                if word.is_empty() {
                    word_depth = depth;
                }
                word.push(c);
            }
            _ => flush(&mut word, word_depth, &mut out),
        }
        i += ch.len_utf8();
    }
    flush(&mut word, word_depth, &mut out);
    out
}

const READONLY_VERBS: &[&str] = &["SELECT", "SHOW", "EXPLAIN", "DESC", "DESCRIBE", "VALUES", "TABLE"];
const DML_VERBS: &[&str] = &["INSERT", "UPDATE", "DELETE", "REPLACE", "MERGE", "TRUNCATE"];
const DDL_VERBS: &[&str] = &["CREATE", "ALTER", "DROP", "RENAME", "COMMENT", "GRANT", "REVOKE"];

/// 分类单条语句。`WITH` 开头的 CTE 按顶层是否出现写动词判定（PG 支持写 CTE）。
/// 括号包裹的 SELECT（如 `(SELECT 1)`）没有顶层词，退化为按全部词序判定。
pub fn classify_statement(stmt: &str, dialect: SqlDialect) -> StatementInfo {
    let all = words_with_depth(stmt, dialect);
    let top: Vec<String> = all
        .iter()
        .filter(|(_, d)| *d <= 0)
        .map(|(w, _)| w.clone())
        .collect();
    let words: Vec<String> = if top.is_empty() {
        all.into_iter().map(|(w, _)| w).collect()
    } else {
        top
    };
    let verb = words.first().cloned().unwrap_or_default();

    let mut dml = DML_VERBS.contains(&verb.as_str());
    let mut ddl = DDL_VERBS.contains(&verb.as_str());
    let mut readonly = READONLY_VERBS.contains(&verb.as_str());

    if verb == "WITH" {
        // 顶层出现写动词则整条按写处理；否则视为只读 CTE 查询
        dml = words.iter().any(|w| DML_VERBS.contains(&w.as_str()));
        ddl = words.iter().any(|w| DDL_VERBS.contains(&w.as_str()));
        readonly = !dml && !ddl;
    }

    let missing_where =
        matches!(verb.as_str(), "UPDATE" | "DELETE") && !words.iter().any(|w| w == "WHERE");

    StatementInfo {
        readonly,
        dml,
        ddl,
        verb,
        missing_where,
    }
}

/// MySQL 标识符：反引号包裹，内部反引号翻倍。
pub fn quote_ident_mysql(name: &str) -> String {
    format!("`{}`", name.replace('`', "``"))
}

/// PG / KingbaseES 标识符：双引号包裹，内部双引号翻倍。
/// 带 schema 限定（a.b）时分段处理。
pub fn quote_ident_pg(name: &str) -> String {
    name.split('.')
        .map(|part| format!("\"{}\"", part.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(".")
}

#[cfg(test)]
mod tests {
    use super::*;

    const MY: SqlDialect = SqlDialect::MySql;
    const PG: SqlDialect = SqlDialect::Pg;

    #[test]
    fn split_basic() {
        assert_eq!(split_statements("SELECT 1; SELECT 2", MY), vec!["SELECT 1", "SELECT 2"]);
        assert_eq!(split_statements("SELECT 1", MY), vec!["SELECT 1"]);
        assert_eq!(split_statements("  ;;  ", MY), Vec::<String>::new());
    }

    #[test]
    fn split_respects_strings_and_comments() {
        assert_eq!(
            split_statements("SELECT ';' AS a; SELECT 2", MY),
            vec!["SELECT ';' AS a", "SELECT 2"]
        );
        assert_eq!(split_statements("-- x; y\nSELECT 1", MY), vec!["-- x; y\nSELECT 1"]);
        assert_eq!(split_statements("/* ; */ SELECT 1", MY), vec!["/* ; */ SELECT 1"]);
        assert_eq!(split_statements("SELECT `a;b` FROM t", MY), vec!["SELECT `a;b` FROM t"]);
        assert_eq!(
            split_statements("SELECT \"a;b\" FROM t; SELECT 2", PG),
            vec!["SELECT \"a;b\" FROM t", "SELECT 2"]
        );
        assert_eq!(
            split_statements("SELECT 'it\\'s; fine'; SELECT 2", MY),
            vec!["SELECT 'it\\'s; fine'", "SELECT 2"]
        );
        assert_eq!(
            split_statements("SELECT 'a''b;c'; SELECT 2", MY),
            vec!["SELECT 'a''b;c'", "SELECT 2"]
        );
    }

    #[test]
    fn split_hash_comment_is_mysql_only() {
        // MySQL：# 是行注释
        assert_eq!(split_statements("SELECT 1 # c;\n; SELECT 2", MY).len(), 2);
        // PG：#> 是 JSONB 操作符，不能当注释吞掉分号
        assert_eq!(
            split_statements("SELECT data #> '{a}' FROM t; SELECT 2", PG),
            vec!["SELECT data #> '{a}' FROM t", "SELECT 2"]
        );
    }

    #[test]
    fn split_dollar_quoted() {
        let sql = "CREATE FUNCTION f() RETURNS void AS $$ BEGIN PERFORM 1; END $$ LANGUAGE plpgsql; SELECT 1";
        let parts = split_statements(sql, PG);
        assert_eq!(parts.len(), 2);
        assert!(parts[0].contains("PERFORM 1;"));
        assert_eq!(split_statements("SELECT $tag$a;b$tag$; SELECT 2", PG).len(), 2);
        // $1 参数占位不触发美元引用
        assert_eq!(split_statements("SELECT $1; SELECT 2", PG).len(), 2);
    }

    #[test]
    fn split_comment_only_dropped() {
        assert_eq!(split_statements("-- hello\n; SELECT 1", MY), vec!["SELECT 1"]);
    }

    #[test]
    fn classify_readonly_forms() {
        for sql in [
            "SELECT * FROM t",
            "  select 1",
            "(SELECT 1)",
            "SHOW TABLES",
            "EXPLAIN SELECT 1",
            "DESC t",
            "DESCRIBE t",
            "VALUES (1)",
            "WITH a AS (SELECT 1) SELECT * FROM a",
            "/* note */ SELECT 1",
        ] {
            let info = classify_statement(sql, MY);
            assert!(info.readonly, "should be readonly: {sql}");
            assert!(!info.dml && !info.ddl, "should not be write: {sql}");
        }
    }

    #[test]
    fn classify_writes() {
        let info = classify_statement("INSERT INTO t VALUES (1)", MY);
        assert!(info.dml && !info.readonly);
        let info = classify_statement("WITH a AS (SELECT 1) INSERT INTO t SELECT * FROM a", PG);
        assert!(info.dml && !info.readonly, "写 CTE 必须判为写");
        let info = classify_statement("TRUNCATE TABLE t", MY);
        assert!(info.dml);
        let info = classify_statement("CREATE TABLE t (id INT)", MY);
        assert!(info.ddl && !info.readonly);
        // 字符串里的写动词不误伤只读判断
        let info = classify_statement("SELECT 'INSERT INTO x' AS s", MY);
        assert!(info.readonly);
    }

    #[test]
    fn classify_missing_where() {
        assert!(classify_statement("UPDATE t SET a=1", MY).missing_where);
        assert!(classify_statement("DELETE FROM t", MY).missing_where);
        assert!(!classify_statement("UPDATE t SET a=1 WHERE id=1", MY).missing_where);
        assert!(
            !classify_statement("DELETE FROM t WHERE id IN (SELECT id FROM x WHERE y=1)", MY)
                .missing_where
        );
        // 仅子查询内有 WHERE，顶层没有 → 仍算缺失
        assert!(
            classify_statement("UPDATE t SET a=(SELECT max(w) FROM x WHERE q=1)", MY).missing_where
        );
        // 字符串里的 WHERE 不算
        assert!(classify_statement("UPDATE t SET a='WHERE'", MY).missing_where);
        assert!(!classify_statement("INSERT INTO t VALUES (1)", MY).missing_where);
    }

    #[test]
    fn quote_idents() {
        assert_eq!(quote_ident_mysql("user"), "`user`");
        assert_eq!(quote_ident_mysql("a`b"), "`a``b`");
        assert_eq!(quote_ident_pg("user"), "\"user\"");
        assert_eq!(quote_ident_pg("a\"b"), "\"a\"\"b\"");
        assert_eq!(quote_ident_pg("public.users"), "\"public\".\"users\"");
    }
}
