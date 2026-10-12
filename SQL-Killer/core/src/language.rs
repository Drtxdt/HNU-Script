use crate::{
    model::*,
    script::{split_script, tokenize, Token},
};
use sqlparser::{
    dialect::{MySqlDialect, SQLiteDialect},
    parser::Parser,
};
use std::collections::{BTreeMap, BTreeSet};

const KEYWORDS: &[&str] = &[
    "SELECT",
    "FROM",
    "WHERE",
    "JOIN",
    "LEFT",
    "RIGHT",
    "FULL",
    "INNER",
    "OUTER",
    "CROSS",
    "NATURAL",
    "ON",
    "USING",
    "GROUP",
    "BY",
    "HAVING",
    "ORDER",
    "ASC",
    "DESC",
    "LIMIT",
    "OFFSET",
    "AS",
    "DISTINCT",
    "ALL",
    "UNION",
    "INTERSECT",
    "EXCEPT",
    "WITH",
    "RECURSIVE",
    "AND",
    "OR",
    "NOT",
    "NULL",
    "IS",
    "IN",
    "EXISTS",
    "BETWEEN",
    "LIKE",
    "CASE",
    "WHEN",
    "THEN",
    "ELSE",
    "END",
    "TRUE",
    "FALSE",
    "INSERT",
    "INTO",
    "UPDATE",
    "SET",
    "DELETE",
    "CREATE",
    "TABLE",
    "VIEW",
    "INDEX",
    "DROP",
    "ALTER",
    "VALUES",
    "PRIMARY",
    "KEY",
    "FOREIGN",
    "REFERENCES",
    "CHECK",
    "DEFAULT",
    "UNIQUE",
    "BEGIN",
    "COMMIT",
    "ROLLBACK",
    "OVER",
    "PARTITION",
    "WINDOW",
    "COLLATE",
];
fn ident(t: &Token) -> String {
    t.text.trim_matches(['"', '`', '[', ']']).to_string()
}
fn is_name(t: &Token) -> bool {
    !t.text.starts_with('\'')
        && t.text
            .chars()
            .next()
            .is_some_and(|c| c.is_alphabetic() || matches!(c, '_' | '"' | '`' | '['))
}
fn keyword(s: &str) -> bool {
    KEYWORDS.contains(&s.to_ascii_uppercase().as_str())
}
fn fields_for(name: &str, schema: &Schema) -> Vec<String> {
    schema
        .relations
        .iter()
        .find(|r| r.name.eq_ignore_ascii_case(name))
        .map(|r| r.columns.iter().map(|c| c.name.clone()).collect())
        .unwrap_or_default()
}

#[derive(Clone, Debug)]
struct Scope {
    start: usize,
    end: usize,
    depth: usize,
    tables: BTreeMap<String, Vec<String>>,
    outputs: Vec<String>,
}
fn scopes(sql: &str, schema: &Schema) -> Vec<Scope> {
    let t = tokenize(sql);
    let mut ctes = BTreeMap::<String, Vec<String>>::new();
    for i in 0..t.len() {
        if t[i].text.eq_ignore_ascii_case("AS")
            && i > 0
            && i + 1 < t.len()
            && t[i + 1].text == "("
            && is_name(&t[i - 1])
        {
            let end = (i + 2..t.len())
                .find(|&j| t[j].text == ")" && t[j].depth == t[i + 1].depth)
                .unwrap_or(t.len());
            let fields = projection(&t[i + 2..end]);
            ctes.insert(ident(&t[i - 1]), fields);
        }
    }
    let mut result = Vec::new();
    for (i, tok) in t
        .iter()
        .enumerate()
        .filter(|(_, t)| t.text.eq_ignore_ascii_case("SELECT"))
    {
        let end = (i + 1..t.len())
            .find(|&j| {
                t[j].depth < tok.depth
                    || (t[j].depth == tok.depth
                        && matches!(
                            t[j].text.to_ascii_uppercase().as_str(),
                            "UNION" | "EXCEPT" | "INTERSECT" | ";"
                        ))
            })
            .unwrap_or(t.len());
        let mut scope = Scope {
            start: tok.start,
            end: t.get(end).map(|t| t.start).unwrap_or(sql.len()),
            depth: tok.depth,
            tables: BTreeMap::new(),
            outputs: projection(&t[i..end]),
        };
        let mut in_from = false;
        let mut need_table = false;
        let mut j = i + 1;
        while j < end {
            let cur = &t[j];
            if cur.depth != tok.depth {
                j += 1;
                continue;
            }
            let word = cur.text.to_ascii_uppercase();
            if word == "FROM" || word == "JOIN" {
                in_from = true;
                need_table = true;
                j += 1;
                continue;
            }
            if matches!(
                word.as_str(),
                "WHERE" | "GROUP" | "HAVING" | "ORDER" | "LIMIT"
            ) {
                in_from = false;
                need_table = false;
            }
            if in_from && word == "," {
                need_table = true;
                j += 1;
                continue;
            }
            if need_table && cur.text == "(" {
                let close = (j + 1..end).find(|&k| t[k].text == ")" && t[k].depth == cur.depth);
                if let Some(k) = close {
                    let fields = projection(&t[j + 1..k]);
                    let mut a = k + 1;
                    if a < end && t[a].text.eq_ignore_ascii_case("AS") {
                        a += 1
                    }
                    if a < end && is_name(&t[a]) && !keyword(&t[a].text) {
                        scope.tables.insert(ident(&t[a]), fields);
                    }
                    j = k + 1;
                    need_table = false;
                    continue;
                }
            }
            if need_table && is_name(cur) {
                let name = ident(cur);
                let fields = ctes
                    .get(&name)
                    .cloned()
                    .unwrap_or_else(|| fields_for(&name, schema));
                let mut a = j + 1;
                if a < end && t[a].text.eq_ignore_ascii_case("AS") {
                    a += 1
                }
                let alias = if a < end && is_name(&t[a]) && !keyword(&t[a].text) {
                    ident(&t[a])
                } else {
                    name
                };
                scope.tables.insert(alias, fields);
                need_table = false;
            }
            j += 1;
        }
        result.push(scope);
    }
    result
}
fn projection(t: &[Token]) -> Vec<String> {
    let Some(i) = t.iter().position(|t| t.text.eq_ignore_ascii_case("SELECT")) else {
        return vec![];
    };
    let d = t[i].depth;
    let end = t[i + 1..]
        .iter()
        .position(|t| t.depth == d && t.text.eq_ignore_ascii_case("FROM"))
        .map(|p| i + 1 + p)
        .unwrap_or(t.len());
    let mut out = Vec::new();
    let mut part = Vec::new();
    for tok in &t[i + 1..end] {
        if tok.text == "," && tok.depth == d {
            if let Some(n) = projection_name(&part) {
                out.push(n)
            }
            part.clear();
        } else {
            part.push(tok)
        }
    }
    if let Some(n) = projection_name(&part) {
        out.push(n)
    }
    out
}
fn projection_name(t: &[&Token]) -> Option<String> {
    if let Some(i) = t.iter().position(|t| t.text.eq_ignore_ascii_case("AS")) {
        return t.get(i + 1).map(|t| ident(t));
    }
    t.last().filter(|t| is_name(t)).map(|t| ident(t))
}
fn visible(
    sql: &str,
    schema: &Schema,
    cursor: usize,
) -> (BTreeMap<String, Vec<String>>, Vec<String>) {
    let mut scopes = scopes(sql, schema);
    scopes.retain(|s| s.start <= cursor && s.end >= cursor);
    scopes.sort_by_key(|s| s.depth);
    let mut tables = BTreeMap::new();
    let mut outputs = vec![];
    for s in scopes {
        tables.extend(s.tables);
        outputs = s.outputs;
    }
    (tables, outputs)
}
pub fn complete_sql(sql: &str, cursor: usize, schema: &Schema) -> Vec<Completion> {
    let cursor = cursor.min(sql.len());
    let cursor = (0..=cursor)
        .rev()
        .find(|&n| sql.is_char_boundary(n))
        .unwrap_or(0);
    let before = &sql[..cursor];
    let tokens = tokenize(before);
    if tokens.last().is_some_and(|t| t.text.starts_with('\'')) {
        return vec![];
    }
    let (tables, outputs) = visible(sql, schema, cursor);
    let mut items = Vec::new();
    let mut add = |label: String, kind: &str, detail: String| {
        items.push(Completion {
            insert_text: label.clone(),
            label,
            kind: kind.into(),
            detail,
        });
    };
    let dot = tokens
        .iter()
        .rposition(|t| t.text == ".")
        .filter(|&i| i + 2 >= tokens.len());
    if let Some(i) = dot {
        if i > 0 {
            let qualifier = ident(&tokens[i - 1]);
            let cols = tables
                .get(&qualifier)
                .cloned()
                .unwrap_or_else(|| fields_for(&qualifier, schema));
            for c in cols {
                add(c, "column", format!("{qualifier} 的列"));
            }
        }
        return items;
    }
    let context = tokens
        .iter()
        .rev()
        .find(|t| keyword(&t.text))
        .map(|t| t.text.to_ascii_uppercase())
        .unwrap_or_default();
    if matches!(context.as_str(), "FROM" | "JOIN" | "INTO" | "UPDATE") {
        for r in &schema.relations {
            add(
                r.name.clone(),
                "table",
                format!("{} · {} 列", r.kind, r.columns.len()),
            );
        }
    } else {
        let mut names = BTreeSet::new();
        for (alias, cols) in &tables {
            for c in cols {
                names.insert(c.clone());
                add(format!("{alias}.{c}"), "column", format!("{alias} · {c}"));
            }
        }
        if tables.is_empty() {
            for r in &schema.relations {
                for c in &r.columns {
                    names.insert(c.name.clone());
                }
            }
        }
        for c in names {
            add(c, "column", "字段".into());
        }
        if context == "BY" {
            for o in outputs {
                add(o, "column", "输出别名".into());
            }
        }
        for f in [
            "COUNT(*)",
            "SUM()",
            "AVG()",
            "MIN()",
            "MAX()",
            "COALESCE()",
            "NULLIF()",
            "ROUND()",
        ] {
            add(f.into(), "function", "SQL 函数".into());
        }
        if context == "ON" {
            for r in &schema.relations {
                for fk in &r.foreign_keys {
                    let left = tables
                        .iter()
                        .find(|(_, c)| r.columns.iter().all(|x| c.contains(&x.name)));
                    let right = tables.iter().find(|(alias, c)| {
                        left.is_none_or(|(a, _)| a != *alias)
                            && schema
                                .relations
                                .iter()
                                .find(|r| r.name == fk.target)
                                .is_some_and(|r| {
                                    r.columns.len() == c.len()
                                        && r.columns.iter().all(|x| c.contains(&x.name))
                                })
                    });
                    if let (Some((a, _)), Some((b, _))) = (left, right) {
                        let expression = fk
                            .columns
                            .iter()
                            .zip(&fk.target_columns)
                            .map(|(l, r)| format!("{a}.{l} = {b}.{r}"))
                            .collect::<Vec<_>>()
                            .join(" AND ");
                        add(expression, "snippet", format!("外键 → {}", fk.target));
                    }
                }
            }
        }
    }
    for k in [
        "SELECT",
        "FROM",
        "WHERE",
        "JOIN",
        "LEFT JOIN",
        "GROUP BY",
        "HAVING",
        "ORDER BY",
        "IS NULL",
        "IS NOT NULL",
        "EXISTS",
        "DISTINCT",
        "LIMIT",
    ] {
        add(k.into(), "keyword", "SQL 关键字".into());
    }
    items.sort_by(|a, b| a.label.cmp(&b.label));
    items.dedup_by(|a, b| a.label == b.label);
    items
}
pub fn analyze_sql(sql: &str, engine: &str, schema: &Schema) -> Analysis {
    let mut result = Analysis::default();
    let dialect: Box<dyn sqlparser::dialect::Dialect> = if engine == "mysql" {
        Box::new(MySqlDialect {})
    } else {
        Box::new(SQLiteDialect {})
    };
    for stmt in split_script(sql) {
        result.statements.push(stmt.range.clone());
        match Parser::parse_sql(dialect.as_ref(), &stmt.sql) {
            Ok(ast) => result.ast.extend(ast.iter().map(ToString::to_string)),
            Err(e) => result.diagnostics.push(Diagnostic {
                severity: "warning".into(),
                category: "parser".into(),
                message: format!("分析器：{e}。执行时仍由真实数据库验证。"),
                range: stmt.range,
                replacement: None,
            }),
        }
    }
    let t = tokenize(sql);
    let scope_list = scopes(sql, schema);
    for (i, tok) in t.iter().enumerate() {
        if !is_name(tok)
            || keyword(&tok.text)
            || t.get(i + 1)
                .is_some_and(|t| matches!(t.text.as_str(), "." | "("))
            || i > 0
                && (t[i - 1].text == "."
                    || matches!(
                        t[i - 1].text.to_uppercase().as_str(),
                        "AS" | "FROM" | "JOIN" | "COLLATE"
                    ))
        {
            continue;
        }
        let mut containing = scope_list
            .iter()
            .filter(|s| s.start <= tok.start && s.end >= tok.end)
            .collect::<Vec<_>>();
        containing.sort_by_key(|s| std::cmp::Reverse(s.depth));
        if containing.is_empty() {
            continue;
        }
        let name = ident(tok);
        if containing
            .iter()
            .any(|s| s.tables.keys().any(|a| a.eq_ignore_ascii_case(&name)))
        {
            continue;
        }
        let Some(inner) = containing.first() else {
            continue;
        };
        let clause = t[..i].iter().rev().find(|x| {
            x.depth == tok.depth
                && matches!(
                    x.text.to_uppercase().as_str(),
                    "SELECT" | "FROM" | "JOIN" | "WHERE" | "ON" | "ORDER" | "GROUP" | "HAVING"
                )
        });
        if clause.is_some_and(|c| matches!(c.text.to_uppercase().as_str(), "FROM" | "JOIN")) {
            continue;
        }
        if clause
            .is_some_and(|c| matches!(c.text.to_uppercase().as_str(), "ORDER" | "GROUP" | "HAVING"))
            && inner.outputs.iter().any(|c| c.eq_ignore_ascii_case(&name))
        {
            continue;
        }
        let mut matches = 0;
        let mut known = true;
        for scope in &containing {
            if scope.tables.values().any(|c| c.is_empty()) {
                known = false;
            }
            matches = scope
                .tables
                .values()
                .filter(|cols| cols.iter().any(|c| c.eq_ignore_ascii_case(&name)))
                .count();
            if matches > 0 {
                break;
            }
        }
        // Natural/USING joins coalesce their common columns; leave that binding to
        // the native engine rather than reporting an incorrect ambiguity.
        let coalescing = t
            .iter()
            .any(|t| matches!(t.text.to_uppercase().as_str(), "NATURAL" | "USING"));
        if known && (!inner.tables.is_empty()) && (matches == 0 || matches > 1 && !coalescing) {
            result.diagnostics.push(Diagnostic {
                severity: "warning".into(),
                category: "binding".into(),
                message: if matches == 0 {
                    format!("当前作用域找不到列 {name}；请检查拼写或声明输出别名")
                } else {
                    format!("列 {name} 来自多个关系，请加表别名限定")
                },
                range: source_range(sql, tok.start, tok.end),
                replacement: None,
            });
        }
    }
    let cte_names = ctes_names(&t);
    for (i, tok) in t.iter().enumerate() {
        if matches!(tok.text.as_str(), "=" | "!=" | "<>")
            && t.get(i + 1)
                .is_some_and(|t| t.text.eq_ignore_ascii_case("NULL"))
        {
            let end = t[i + 1].end;
            result.diagnostics.push(Diagnostic {
                severity: "warning".into(),
                category: "semantics".into(),
                message: "与 NULL 比较的结果是 UNKNOWN；请使用 IS NULL／IS NOT NULL。".into(),
                range: source_range(sql, tok.start, end),
                replacement: Some(
                    if tok.text == "=" {
                        "IS NULL"
                    } else {
                        "IS NOT NULL"
                    }
                    .into(),
                ),
            });
        }
        if matches!(tok.text.to_ascii_uppercase().as_str(), "FROM" | "JOIN") {
            if let Some(name) = t.get(i + 1).filter(|t| is_name(t)) {
                let n = ident(name);
                if !schema
                    .relations
                    .iter()
                    .any(|r| r.name.eq_ignore_ascii_case(&n))
                    && !cte_names.contains(&n)
                {
                    let suggestion = schema
                        .relations
                        .iter()
                        .min_by_key(|r| distance(&r.name.to_lowercase(), &n.to_lowercase()));
                    result.diagnostics.push(Diagnostic {
                        severity: "error".into(),
                        category: "schema".into(),
                        message: format!(
                            "找不到表或视图 {n}{}",
                            suggestion
                                .map(|r| format!("；是否指 {}？", r.name))
                                .unwrap_or_default()
                        ),
                        range: source_range(sql, name.start, name.end),
                        replacement: suggestion.map(|r| r.name.clone()),
                    });
                }
            }
        }
        if tok.text == "." && i > 0 {
            if let Some(col) = t.get(i + 1).filter(|c| is_name(c)) {
                let qualifier = ident(&t[i - 1]);
                let (tables, _) = visible(sql, schema, tok.start);
                if let Some(fields) = tables.get(&qualifier) {
                    if !fields.is_empty()
                        && !fields.iter().any(|f| f.eq_ignore_ascii_case(&ident(col)))
                    {
                        result.diagnostics.push(Diagnostic {
                            severity: "error".into(),
                            category: "schema".into(),
                            message: format!("{qualifier} 没有列 {}", col.text),
                            range: source_range(sql, col.start, col.end),
                            replacement: None,
                        });
                    }
                }
            }
        }
    }
    // Course grouping rule is a portability warning, not an engine syntax error.
    for scope in scopes(sql, schema) {
        let local = t
            .iter()
            .filter(|t| t.start >= scope.start && t.end <= scope.end && t.depth == scope.depth)
            .collect::<Vec<_>>();
        if let Some(g) = local.windows(2).position(|w| {
            w[0].text.eq_ignore_ascii_case("GROUP") && w[1].text.eq_ignore_ascii_case("BY")
        }) {
            let group = local[g + 2..]
                .iter()
                .take_while(|t| {
                    !matches!(
                        t.text.to_ascii_uppercase().as_str(),
                        "HAVING" | "ORDER" | "LIMIT"
                    )
                })
                .map(|t| ident(t).to_lowercase())
                .collect::<BTreeSet<_>>();
            for output in scope.outputs {
                if !group.contains(&output.to_lowercase())
                    && scope.tables.values().any(|cols| cols.contains(&output))
                {
                    if let Some(token) = local.iter().find(|t| ident(t) == output) {
                        result.diagnostics.push(Diagnostic{severity:"warning".into(),category:"course".into(),message:format!("课程规范：{output} 既未分组也未聚合；实际合法性还取决于引擎及函数依赖。"),range:source_range(sql,token.start,token.end),replacement:None});
                    }
                }
            }
        }
    }
    result
}
fn ctes_names(t: &[Token]) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for w in t.windows(3) {
        if is_name(&w[0]) && w[1].text.eq_ignore_ascii_case("AS") && w[2].text == "(" {
            names.insert(ident(&w[0]));
        }
    }
    names
}
fn distance(a: &str, b: &str) -> usize {
    let b = b.chars().collect::<Vec<_>>();
    let mut d = (0..=b.len()).collect::<Vec<_>>();
    for (i, x) in a.chars().enumerate() {
        let mut prev = d[0];
        d[0] = i + 1;
        for (j, y) in b.iter().enumerate() {
            let old = d[j + 1];
            d[j + 1] = (d[j] + 1)
                .min(d[j + 1] + 1)
                .min(prev + usize::from(x != *y));
            prev = old;
        }
    }
    d[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database;
    #[test]
    fn alias_and_cte() {
        let s = database::schema(&database::memory("student").unwrap()).unwrap();
        for sql in [
            "SELECT S. FROM Student S",
            "WITH x AS (SELECT Sname AS name FROM Student) SELECT x. FROM x",
        ] {
            let pos = sql.find('.').unwrap() + 1;
            let v = complete_sql(sql, pos, &s);
            assert!(!v.is_empty());
        }
        let d = analyze_sql("SELECT Sname FROM Students WHERE Sage = NULL", "sqlite", &s);
        assert!(d.diagnostics.iter().any(|d| d.category == "semantics"));
        assert!(d.diagnostics.iter().any(|d| d.category == "schema"));
    }
    #[test]
    fn unicode_positions() {
        let s = "SELECT '😀中';\nSELECT x";
        let p = s.find('x').unwrap();
        let r = source_range(s, p, p + 1);
        assert_eq!((r.line, r.column), (2, 8));
    }
}
